//! Scenario families 1-5: SSE event handling and client behavior.
//!
//! 1. SSE — step completed event pushed with type, node_name, sequence
//! 2. SSE keepalive — `:keepalive` comment sent every 15s idle
//! 3. SSE — instance failure event pushed with type and error
//! 4. SSE — client lagged >1000 events triggers connection drop
//! 5. SSE — completed instance sends final events then closes connection

use super::*;
use tokio::sync::broadcast;

mod sse_step_completed_event {
    use super::*;

    #[test]
    fn given_step_completes_when_event_serialized_then_has_type_node_name_sequence() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build-step".to_string(),
            sequence: 42,
        };

        let json = event.to_json_value();

        assert_eq!(json["type"], "step_completed");
        assert_eq!(json["node_name"], "build-step");
        assert_eq!(json["sequence"], 42);
    }

    #[test]
    fn given_multiple_steps_complete_when_events_serialized_then_each_has_correct_sequence() {
        for seq in 0..5u64 {
            let event = WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", seq),
                sequence: seq,
            };

            let json = event.to_json_value();

            assert_eq!(json["type"], "step_completed");
            assert_eq!(json["sequence"], seq);
        }
    }
}

mod sse_keepalive {
    use super::*;

    #[test]
    fn given_sse_connection_when_15s_idle_then_keepalive_interval_is_15s() {
        assert_eq!(SSE_KEEPALIVE_INTERVAL_SECS, 15);
    }
}

mod sse_instance_failure {
    use super::*;

    #[test]
    fn given_instance_fails_when_event_pushed_then_type_and_error_present() {
        let event = WorkflowSseEvent::InstanceFailed {
            error: "timeout after 30s".to_string(),
        };

        let json = event.to_json_value();

        assert_eq!(json["type"], "instance_failed");
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

mod sse_client_lag_drops_connection {
    use super::*;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn given_sse_client_lags_over_1000_events_when_detected_then_connection_dropped() {
        let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(10);

        let slow_consumer = tokio::spawn(async move {
            let mut stream = tokio_stream::wrappers::BroadcastStream::new(rx);
            let mut count = 0u64;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(_) => {
                        count += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
                        return (count, true);
                    }
                }
            }
            (count, false)
        });

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

        assert!(lagged, "Slow client should be dropped via Lagged error");
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
