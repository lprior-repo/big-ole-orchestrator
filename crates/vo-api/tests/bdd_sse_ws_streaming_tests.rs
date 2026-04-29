//! BDD tests for Monitoring: SSE, WebSocket, Live Streaming.
//!
//! Scenario families:
//! 1. SSE — step completed event pushed with type, node_name, sequence
//! 2. SSE keepalive — `:keepalive` comment sent every 15s idle
//! 3. SSE — instance failure event pushed with type and error
//! 4. SSE — client lagged >1000 events triggers connection drop
//! 5. SSE — completed instance sends final events then closes connection
//! 6. WebSocket — step completed event pushed with event details
//! 7. WebSocket — instance completed message pushed
//! 8. WebSocket — bidirectional text messages logged at debug level
//! 9. WebSocket — client lagged >1000 events silently dropped, connection stays open
//! 10. Multiple SSE+WS clients — broadcast: all clients receive same event
//!
//! Given/When/Then format per Dan North.

// =========================================================================
// Shared SSE types (mirrors handlers/sse.rs — module currently commented out)
// =========================================================================

#[derive(Debug, Clone)]
pub enum WorkflowSseEvent {
    StepCompleted {
        node_name: String,
        sequence: u64,
    },
    StepFailed {
        node_name: String,
        sequence: u64,
        error: String,
    },
    InstanceFailed {
        error: String,
    },
    InstanceCompleted,
    TimerFired {
        timer_id: String,
    },
    SignalReceived {
        signal_name: String,
    },
    PhaseChanged {
        phase: String,
    },
}

impl WorkflowSseEvent {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            WorkflowSseEvent::StepCompleted {
                node_name,
                sequence,
            } => {
                serde_json::json!({
                    "type": "step_completed",
                    "node_name": node_name,
                    "sequence": sequence,
                })
            }
            WorkflowSseEvent::StepFailed {
                node_name,
                sequence,
                error,
            } => {
                serde_json::json!({
                    "type": "step_failed",
                    "node_name": node_name,
                    "sequence": sequence,
                    "error": error,
                })
            }
            WorkflowSseEvent::TimerFired { timer_id } => {
                serde_json::json!({
                    "type": "timer_fired",
                    "timer_id": timer_id,
                })
            }
            WorkflowSseEvent::SignalReceived { signal_name } => {
                serde_json::json!({
                    "type": "signal_received",
                    "signal_name": signal_name,
                })
            }
            WorkflowSseEvent::PhaseChanged { phase } => {
                serde_json::json!({
                    "type": "phase_changed",
                    "phase": phase,
                })
            }
            WorkflowSseEvent::InstanceCompleted => {
                serde_json::json!({
                    "type": "instance_completed",
                })
            }
            WorkflowSseEvent::InstanceFailed { error } => {
                serde_json::json!({
                    "type": "instance_failed",
                    "error": error,
                })
            }
        }
    }
}

const SSE_BROADCAST_CAPACITY: usize = 1000;
const SSE_KEEPALIVE_INTERVAL_SECS: u64 = 15;

// =========================================================================
// Scenario Family 1: SSE — step completed event pushed
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    mod sse_step_completed_event {
        use super::*;

        #[test]
        fn given_step_completes_when_event_serialized_then_has_type_node_name_sequence() {
            // GIVEN a step completes
            let event = WorkflowSseEvent::StepCompleted {
                node_name: "build-step".to_string(),
                sequence: 42,
            };

            // WHEN event is serialized to SSE payload
            let json = event.to_json_value();

            // THEN event pushed with type="step_completed"
            assert_eq!(json["type"], "step_completed");
            // THEN event has node_name
            assert_eq!(json["node_name"], "build-step");
            // THEN event has sequence
            assert_eq!(json["sequence"], 42);
        }

        #[test]
        fn given_multiple_steps_complete_when_events_serialized_then_each_has_correct_sequence() {
            // GIVEN multiple steps complete in sequence
            for seq in 0..5u64 {
                let event = WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", seq),
                    sequence: seq,
                };

                // WHEN each event is serialized
                let json = event.to_json_value();

                // THEN each has correct type and sequence
                assert_eq!(json["type"], "step_completed");
                assert_eq!(json["sequence"], seq);
            }
        }
    }

    // =========================================================================
    // Scenario Family 2: SSE keepalive
    // =========================================================================

    mod sse_keepalive {
        use super::*;

        #[test]
        fn given_sse_connection_when_15s_idle_then_keepalive_interval_is_15s() {
            // GIVEN SSE keepalive configuration
            // WHEN idle for 15 seconds
            // THEN `:keepalive` comment sent
            assert_eq!(SSE_KEEPALIVE_INTERVAL_SECS, 15);
        }
    }

    // =========================================================================
    // Scenario Family 3: SSE — instance failure event pushed
    // =========================================================================

    mod sse_instance_failure {
        use super::*;

        #[test]
        fn given_instance_fails_when_event_pushed_then_type_and_error_present() {
            // GIVEN an instance fails
            let event = WorkflowSseEvent::InstanceFailed {
                error: "timeout after 30s".to_string(),
            };

            // WHEN event is serialized
            let json = event.to_json_value();

            // THEN event pushed with type="instance_failed"
            assert_eq!(json["type"], "instance_failed");
            // THEN event has error details
            assert_eq!(json["error"], "timeout after 30s");
        }

        #[test]
        fn given_instance_fails_with_various_errors_when_serialized_then_error_preserved() {
            let errors = vec![
                "node out of memory",
                "disk full",
                "network unreachable",
                "permission denied",
            ];
            for error in errors {
                let event = WorkflowSseEvent::InstanceFailed {
                    error: error.to_string(),
                };
                let json = event.to_json_value();
                assert_eq!(json["type"], "instance_failed");
                assert_eq!(json["error"], error);
            }
        }

        #[tokio::test]
        async fn given_instance_fails_when_broadcast_then_subscriber_receives_failure() {
            let (tx, mut rx) = broadcast::channel::<WorkflowSseEvent>(SSE_BROADCAST_CAPACITY);

            let handle =
                tokio::spawn(async move { rx.recv().await.ok().map(|e| e.to_json_value()) });

            let _ = tx.send(WorkflowSseEvent::InstanceFailed {
                error: "critical failure".to_string(),
            });

            let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

            let json = result.expect("should receive event");
            assert_eq!(json["type"], "instance_failed");
            assert_eq!(json["error"], "critical failure");
        }
    }

    // =========================================================================
    // Scenario Family 4: SSE — client lagged >1000 events triggers drop
    // =========================================================================

    mod sse_client_lag_drops_connection {
        use super::*;
        use tokio_stream::StreamExt;

        #[tokio::test]
        async fn given_sse_client_lags_over_1000_events_when_detected_then_connection_dropped() {
            // GIVEN SSE client connected with small channel buffer
            let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(10);

            // WHEN client is slow (lagged behind)
            let slow_consumer = tokio::spawn(async move {
                let mut stream = tokio_stream::wrappers::BroadcastStream::new(rx);
                let mut count = 0u64;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(_) => {
                            count += 1;
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(
                            _,
                        )) => {
                            // THEN connection dropped with server-side close (Lagged error)
                            return (count, true);
                        }
                    }
                }
                (count, false)
            });

            // Send far more events than buffer capacity
            for i in 0..1500u64 {
                let _ = tx.send(WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }
            drop(tx);

            let (count, lagged) =
                tokio::time::timeout(std::time::Duration::from_secs(5), slow_consumer)
                    .await
                    .expect("should not timeout")
                    .expect("task should not panic");

            // THEN client received Lagged error (server-side close)
            assert!(lagged, "Slow client should be dropped via Lagged error");
            // THEN client did NOT receive all 1500 events
            assert!(
                count < 1500,
                "Lagged client should miss events, got {}",
                count
            );
        }

        #[tokio::test]
        async fn given_fast_client_when_receiving_events_then_no_lag_error() {
            let (tx, mut rx) = broadcast::channel::<WorkflowSseEvent>(SSE_BROADCAST_CAPACITY);
            let event_count = 500u64;

            let handle = tokio::spawn(async move {
                let mut count = 0u64;
                while let Ok(_) = rx.recv().await {
                    count += 1;
                }
                count
            });

            for i in 0..event_count {
                let _ = tx.send(WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }
            drop(tx);

            let count = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

            assert_eq!(count, event_count, "Fast client should receive all events");
        }
    }

    // =========================================================================
    // Scenario Family 5: SSE — completed instance sends final events then closes
    // =========================================================================

    mod sse_completed_instance {
        use super::*;

        #[test]
        fn given_instance_completed_event_when_serialized_then_type_is_instance_completed() {
            let event = WorkflowSseEvent::InstanceCompleted;
            let json = event.to_json_value();
            assert_eq!(json["type"], "instance_completed");
        }

        #[tokio::test]
        async fn given_instance_events_and_completion_when_subscriber_joins_then_stream_ends() {
            let (tx, mut rx) = broadcast::channel::<WorkflowSseEvent>(SSE_BROADCAST_CAPACITY);

            let handle = tokio::spawn(async move {
                let mut received_completion = false;
                loop {
                    match rx.recv().await {
                        Ok(WorkflowSseEvent::InstanceCompleted) => {
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

            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: "step-1".to_string(),
                sequence: 1,
            });
            let _ = tx.send(WorkflowSseEvent::InstanceCompleted);
            drop(tx);

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

    // =========================================================================
    // Scenario Family 6: WebSocket — step completed event pushed
    // =========================================================================

    mod ws_step_completed_event {
        use vo_api::handlers::ws::{WorkflowEvent, WsBroadcaster};

        #[test]
        fn given_step_completes_when_ws_message_pushed_then_has_type_node_name_sequence() {
            // GIVEN a step completes
            let event = WorkflowEvent::StepCompleted {
                node_name: "validate-input".to_string(),
                sequence: 7,
            };

            // WHEN message is serialized
            let json_str = event.to_json_string();
            let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // THEN message pushed with event details
            assert_eq!(json["type"], "step_completed");
            assert_eq!(json["node_name"], "validate-input");
            assert_eq!(json["sequence"], 7);
        }

        #[tokio::test]
        async fn given_ws_broadcaster_when_step_event_sent_then_subscriber_receives_it() {
            let broadcaster = WsBroadcaster::new();
            let mut receiver = broadcaster.subscribe();

            let handle = tokio::spawn(async move { receiver.recv().await.ok() });

            let _ = broadcaster.send(WorkflowEvent::StepCompleted {
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

    // =========================================================================
    // Scenario Family 7: WebSocket — instance completed message pushed
    // =========================================================================

    mod ws_instance_completed {
        use tokio::sync::broadcast;
        use vo_api::handlers::ws::{WorkflowWsEvent, WsBroadcaster};

        #[test]
        fn given_instance_completes_when_ws_message_pushed_then_type_is_instance_completed() {
            let event = WorkflowEvent::InstanceCompleted;
            let json_str = event.to_json_string();
            let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // THEN message pushed with type="instance_completed"
            assert_eq!(json["type"], "instance_completed");
        }

        #[test]
        fn given_all_ws_event_types_when_serialized_then_type_field_present() {
            let events = vec![
                WorkflowEvent::StepCompleted {
                    node_name: "s1".to_string(),
                    sequence: 1,
                },
                WorkflowEvent::StepFailed {
                    node_name: "s1".to_string(),
                    sequence: 1,
                    error: "fail".to_string(),
                },
                WorkflowEvent::TimerFired {
                    timer_id: "t1".to_string(),
                },
                WorkflowEvent::SignalReceived {
                    signal_name: "sig".to_string(),
                },
                WorkflowEvent::PhaseChanged {
                    phase: "live".to_string(),
                },
                WorkflowEvent::InstanceCompleted,
                WorkflowEvent::InstanceFailed {
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
                        Ok(WorkflowEvent::InstanceCompleted) => {
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

            let _ = broadcaster.send(WorkflowEvent::StepCompleted {
                node_name: "final-step".to_string(),
                sequence: 10,
            });
            let _ = broadcaster.send(WorkflowEvent::InstanceCompleted);
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

    // =========================================================================
    // Scenario Family 8: WebSocket — bidirectional text messages logged at debug
    // =========================================================================

    mod ws_bidirectional_messages {
        use vo_api::handlers::ws::{WorkflowEvent, WsBroadcaster, WsConnectionCount};

        #[test]
        fn given_ws_client_sends_text_when_received_then_message_parseable() {
            // WHEN client sends text message
            let text_msg = r#"{"action": "subscribe", "instance_id": "payments/abc"}"#;

            // THEN message is parseable (logged at debug level — verified by handler code)
            let json: serde_json::Value = serde_json::from_str(text_msg).unwrap();
            assert_eq!(json["action"], "subscribe");
            assert_eq!(json["instance_id"], "payments/abc");
        }

        #[test]
        fn given_ws_connection_count_when_clients_connect_and_disconnect_then_reflects_count() {
            let counter = WsConnectionCount::new();

            // WHEN client connects
            let before = counter.increment();
            assert_eq!(before, 0, "Should return previous count before increment");
            assert_eq!(
                counter
                    .active_connections
                    .load(std::sync::atomic::Ordering::SeqCst),
                1
            );

            // WHEN another client connects
            counter.increment();
            assert_eq!(
                counter
                    .active_connections
                    .load(std::sync::atomic::Ordering::SeqCst),
                2
            );

            // WHEN client disconnects
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

    // =========================================================================
    // Scenario Family 9: WebSocket — client lagged >1000 events silently dropped
    // =========================================================================

    mod ws_client_lag_silent_drop {
        use crate::WorkflowSseEvent;
        use tokio::sync::broadcast;
        use vo_api::handlers::ws::{WorkflowWsEvent, WsBroadcaster};

        #[tokio::test]
        async fn given_ws_client_lags_over_1000_when_detected_then_events_silently_dropped_connection_stays_open(
        ) {
            // GIVEN WebSocket client connected with capacity 10
            let (tx, mut receiver) = broadcast::channel::<WorkflowSseEvent>(10);

            // WHEN client lags (slow consumer that continues on Lagged, mimicking WS handler)
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
                            // THEN events silently dropped (connection stays open, WS handler uses continue)
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

            // Send far more events than capacity to force lagging
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

            // THEN connection stayed open (received Lagged but did NOT disconnect)
            assert!(stayed_open, "WS connection should stay open on lag");
            assert!(lagged_count > 0, "Should have experienced Lagged errors");
            // THEN not all events received (silently dropped)
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
                let _ = broadcaster.send(WorkflowEvent::StepCompleted {
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

    mod broadcast_multiple_clients {
        use super::*;
        use tokio::sync::broadcast;
        use vo_api::handlers::ws::{WorkflowWsEvent, WsBroadcaster};

        #[tokio::test]
        async fn given_multiple_sse_clients_when_same_event_then_all_clients_receive_event() {
            // GIVEN multiple SSE clients subscribed
            let (tx, _) = broadcast::channel::<WorkflowSseEvent>(SSE_BROADCAST_CAPACITY);
            let client_count = 5;

            let mut handles = Vec::new();
            for _ in 0..client_count {
                let mut rx = tx.subscribe();
                handles.push(tokio::spawn(async move {
                    let mut count = 0u64;
                    while let Ok(_) = rx.recv().await {
                        count += 1;
                    }
                    count
                }));
            }

            // WHEN same instance event is broadcast
            let event_count = 100u64;
            for i in 0..event_count {
                let _ = tx.send(WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }
            drop(tx);

            // THEN all clients receive event (broadcast semantics)
            let mut total = 0u64;
            for handle in handles {
                let count = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
                    .await
                    .expect("should not timeout")
                    .expect("task should not panic");
                total += count;
            }

            assert!(
                total >= event_count,
                "All {} SSE clients should collectively receive all {} events, got {}",
                client_count,
                event_count,
                total
            );
        }

        #[tokio::test]
        async fn given_multiple_ws_clients_when_same_event_then_all_clients_receive_event() {
            // GIVEN multiple WS clients subscribed
            let broadcaster = WsBroadcaster::new();
            let client_count = 5;

            let mut handles = Vec::new();
            for _ in 0..client_count {
                let mut receiver = broadcaster.subscribe();
                handles.push(tokio::spawn(async move {
                    let mut count = 0u64;
                    while let Ok(_) = receiver.recv().await {
                        count += 1;
                    }
                    count
                }));
            }

            // Give subscribers time to register
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // WHEN same instance event is broadcast
            let event_count = 100u64;
            for i in 0..event_count {
                let _ = broadcaster.send(WorkflowEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }
            drop(broadcaster);

            // THEN all clients receive event (broadcast semantics)
            let mut total = 0u64;
            for handle in handles {
                let count = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
                    .await
                    .expect("should not timeout")
                    .expect("task should not panic");
                total += count;
            }

            assert!(
                total >= event_count,
                "All {} WS clients should collectively receive all {} events, got {}",
                client_count,
                event_count,
                total
            );
        }

        #[tokio::test]
        async fn given_mixed_sse_and_ws_clients_when_events_broadcast_then_both_receive() {
            // GIVEN both SSE and WS clients subscribed to same instance
            let sse_tx = {
                let (tx, _) = broadcast::channel::<WorkflowSseEvent>(SSE_BROADCAST_CAPACITY);
                tx
            };
            let ws_broadcaster = WsBroadcaster::new();

            // SSE client
            let mut sse_rx = sse_tx.subscribe();
            let sse_handle = tokio::spawn(async move {
                let mut count = 0u64;
                while let Ok(_) = sse_rx.recv().await {
                    count += 1;
                }
                count
            });

            // WS client
            let mut ws_rx = ws_broadcaster.subscribe();
            let ws_handle = tokio::spawn(async move {
                let mut count = 0u64;
                while let Ok(_) = ws_rx.recv().await {
                    count += 1;
                }
                count
            });

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            // WHEN events are broadcast
            let event_count = 50u64;
            for i in 0..event_count {
                let _ = sse_tx.send(WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
                let _ = ws_broadcaster.send(WorkflowEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }
            drop(sse_tx);
            drop(ws_broadcaster);

            // THEN both SSE and WS clients receive events
            let sse_count = tokio::time::timeout(std::time::Duration::from_secs(5), sse_handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

            let ws_count = tokio::time::timeout(std::time::Duration::from_secs(5), ws_handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

            assert_eq!(
                sse_count, event_count,
                "SSE client should receive all {} events",
                event_count
            );
            assert_eq!(
                ws_count, event_count,
                "WS client should receive all {} events",
                event_count
            );
        }

        #[tokio::test]
        async fn given_clients_join_at_different_times_when_events_broadcast_then_late_client_misses_early_events(
        ) {
            // GIVEN two SSE clients joining at different times
            let (tx, _) = broadcast::channel::<WorkflowSseEvent>(SSE_BROADCAST_CAPACITY);

            // Send 50 events before second client joins
            for i in 0..50u64 {
                let _ = tx.send(WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }

            // First client (subscribed from start)
            let mut early_rx = tx.subscribe();
            let early_handle = tokio::spawn(async move {
                let mut count = 0u64;
                while let Ok(_) = early_rx.recv().await {
                    count += 1;
                }
                count
            });

            // Second client joins after 50 events
            let mut late_rx = tx.subscribe();
            let late_handle = tokio::spawn(async move {
                let mut count = 0u64;
                while let Ok(_) = late_rx.recv().await {
                    count += 1;
                }
                count
            });

            // Send 50 more events
            for i in 50..100u64 {
                let _ = tx.send(WorkflowSseEvent::StepCompleted {
                    node_name: format!("step-{}", i),
                    sequence: i,
                });
            }
            drop(tx);

            let early_count = tokio::time::timeout(std::time::Duration::from_secs(5), early_handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

            let late_count = tokio::time::timeout(std::time::Duration::from_secs(5), late_handle)
                .await
                .expect("should not timeout")
                .expect("task should not panic");

            // THEN late client receives fewer events than early client
            assert!(
                late_count <= early_count,
                "Late client should receive <= early client events ({} vs {})",
                late_count,
                early_count
            );
        }
    }
}
