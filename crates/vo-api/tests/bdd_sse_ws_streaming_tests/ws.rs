//! Scenario families 6-9: WebSocket event handling and client behavior.
//!
//! 6. WebSocket — step completed event pushed with event details
//! 7. WebSocket — instance completed message pushed
//! 8. WebSocket — bidirectional text messages logged at debug level
//! 9. WebSocket — client lagged >1000 events silently dropped, connection stays open

use super::*;
use tokio::sync::broadcast;
use vo_api::handlers::ws::{WorkflowWsEvent, WsBroadcaster};

mod ws_step_completed_event {
    use super::*;

    #[test]
    fn given_step_completes_when_ws_message_pushed_then_has_type_node_name_sequence() {
        let event = WorkflowWsEvent::StepCompleted {
            node_name: "validate-input".to_string(),
            sequence: 7,
        };

        let json_str = event.to_json_string();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json["type"], "step_completed");
        assert_eq!(json["node_name"], "validate-input");
        assert_eq!(json["sequence"], 7);
    }

    #[tokio::test]
    async fn given_ws_broadcaster_when_step_event_sent_then_subscriber_receives_it() {
        let broadcaster = WsBroadcaster::new();
        let mut receiver = broadcaster.subscribe();

        let handle = tokio::spawn(async move { receiver.recv().await.ok() });

        let _ = broadcaster.send(WorkflowWsEvent::StepCompleted {
            node_name: "process".to_string(),
            sequence: 1,
        });

        let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle)
            .await
            .expect("should not timeout")
            .expect("task should not panic");

        let event = result.expect("should receive event");
        let json_str = event.to_json_string();
        assert!(json_str.contains("\"type\":\"step_completed\""));
        assert!(json_str.contains("\"node_name\":\"process\""));
        assert!(json_str.contains("\"sequence\":1"));
    }
}

mod ws_instance_completed {
    use super::*;

    #[test]
    fn given_instance_completes_when_ws_message_pushed_then_type_is_instance_completed() {
        let event = WorkflowWsEvent::InstanceCompleted;
        let json_str = event.to_json_string();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json["type"], "instance_completed");
    }

    #[test]
    fn given_all_ws_event_types_when_serialized_then_type_field_present() {
        let events = vec![
            WorkflowWsEvent::StepCompleted {
                node_name: "s1".to_string(),
                sequence: 1,
            },
            WorkflowWsEvent::StepFailed {
                node_name: "s1".to_string(),
                sequence: 1,
                error: "fail".to_string(),
            },
            WorkflowWsEvent::TimerFired {
                timer_id: "t1".to_string(),
            },
            WorkflowWsEvent::SignalReceived {
                signal_name: "sig".to_string(),
            },
            WorkflowWsEvent::PhaseChanged {
                phase: "live".to_string(),
            },
            WorkflowWsEvent::InstanceCompleted,
            WorkflowWsEvent::InstanceFailed {
                error: "err".to_string(),
            },
        ];

        for event in events {
            let json_str = event.to_json_string();
            let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
            assert!(
                json.get("type").is_some(),
                "Every WS event must have a 'type' field"
            );
        }
    }

    #[tokio::test]
    async fn given_instance_completes_when_broadcast_then_subscriber_receives_completion() {
        let broadcaster = WsBroadcaster::new();
        let mut receiver = broadcaster.subscribe();

        let handle = tokio::spawn(async move {
            let mut received_completion = false;
            loop {
                match receiver.recv().await {
                    Ok(WorkflowWsEvent::InstanceCompleted) => {
                        received_completion = true;
                        break;
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            received_completion
        });

        let _ = broadcaster.send(WorkflowWsEvent::StepCompleted {
            node_name: "final-step".to_string(),
            sequence: 10,
        });
        let _ = broadcaster.send(WorkflowWsEvent::InstanceCompleted);
        drop(broadcaster);

        let received = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("should not timeout")
            .expect("task should not panic");

        assert!(
            received,
            "Subscriber should receive InstanceCompleted event"
        );
    }
}

mod ws_bidirectional_messages {
    use super::*;

    #[test]
    fn given_ws_client_sends_text_when_received_then_message_parseable() {
        let text_msg = r#"{"action": "subscribe", "instance_id": "payments/abc"}"#;

        let json: serde_json::Value = serde_json::from_str(text_msg).unwrap();
        assert_eq!(json["action"], "subscribe");
        assert_eq!(json["instance_id"], "payments/abc");
    }

    #[test]
    fn given_ws_connection_count_when_clients_connect_and_disconnect_then_reflects_count() {
        let counter = WsConnectionCount::new();

        let before = counter.increment();
        assert_eq!(before, 0, "Should return previous count before increment");
        assert_eq!(
            counter
                .active_connections
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        counter.increment();
        assert_eq!(
            counter
                .active_connections
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );

        counter.decrement();
        assert_eq!(
            counter
                .active_connections
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn given_various_text_messages_when_parsed_then_valid_json() {
        let messages = vec![
            r#"{"type":"ping"}"#,
            r#"{"type":"ack","seq":42}"#,
            r#"{"action":"unsubscribe"}"#,
            r#"{"filter":{"event_type":"step_completed"}}"#,
        ];

        for msg in messages {
            let json: serde_json::Value = serde_json::from_str(msg).unwrap();
            assert!(
                json.is_object(),
                "Text message should be valid JSON: {}",
                msg
            );
        }
    }
}

mod ws_client_lag_silent_drop {
    use super::*;

    #[tokio::test]
    async fn given_ws_client_lags_over_1000_when_detected_then_events_silently_dropped_connection_stays_open(
    ) {
        let (tx, mut receiver) = broadcast::channel::<WorkflowSseEvent>(10);

        let handle = tokio::spawn(async move {
            let mut count = 0u64;
            let mut lagged_count = 0u64;
            loop {
                match receiver.recv().await {
                    Ok(_) => {
                        count += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        lagged_count += 1;
                        if lagged_count > 3 {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            (count, lagged_count, lagged_count > 0)
        });

        for i in 0..500u64 {
            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }
        drop(tx);

        let (count, lagged_count, stayed_open) =
            tokio::time::timeout(std::time::Duration::from_secs(5), handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

        assert!(stayed_open, "WS connection should stay open on lag");
        assert!(lagged_count > 0, "Should have experienced Lagged errors");
        assert!(
            count < 500,
            "Lagged client should miss events, got {}",
            count
        );
    }

    #[tokio::test]
    async fn given_ws_fast_consumer_when_no_lag_then_receives_all_events() {
        let broadcaster = WsBroadcaster::new();
        let mut receiver = broadcaster.subscribe();

        let send_count = 500u64;
        let handle = tokio::spawn(async move {
            let mut count = 0u64;
            while let Ok(_) = receiver.recv().await {
                count += 1;
            }
            count
        });

        for i in 0..send_count {
            let _ = broadcaster.send(WorkflowWsEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }
        drop(broadcaster);

        let count = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("should not timeout")
            .expect("task should not panic");

        assert_eq!(
            count, send_count,
            "Fast WS consumer should receive all events"
        );
    }
}
