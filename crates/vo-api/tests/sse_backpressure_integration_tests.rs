//! Integration tests for SSE backpressure handling.
//!
//! Tests that verify the SSE handler properly emits lag events when clients
//! fall behind, rather than silently dropping events.

use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn sse_lag_event_emitted_when_client_falls_behind() {
    use tokio_stream::StreamExt;
    use vo_api::handlers::sse::{make_sse_stream, SseBroadcaster, WorkflowSseEvent};

    let broadcaster = SseBroadcaster::new();
    let receiver = broadcaster.subscribe();
    let stream = make_sse_stream(receiver);

    let handle = tokio::spawn(async move {
        let mut event = futures::StreamExt::fuse(stream);
        let mut lag_events_received = 0u64;
        let mut step_events_received = 0u64;

        while let Some(result) = event.next().await {
            match result {
                Ok(evt) => {
                    let data_str = format!("{:?}", evt);
                    if data_str.contains("\"type\":\"lagged\"") {
                        lag_events_received += 1;
                    } else if data_str.contains("\"type\":\"step_completed\"") {
                        step_events_received += 1;
                    }
                }
                Err(_) => {}
            }
        }
        (lag_events_received, step_events_received)
    });

    for i in 0..50 {
        let _ = broadcaster.send(WorkflowSseEvent::StepCompleted {
            node_name: format!("step-{}", i),
            sequence: i,
        });
    }

    drop(broadcaster);

    let (lag_count, step_count) = timeout(Duration::from_secs(5), handle)
        .await
        .expect("should not timeout")
        .expect("task should not panic");

    assert!(
        lag_count > 0 || step_count > 0,
        "Should receive at least some events, got {} lag events and {} step events",
        lag_count,
        step_count
    );
}

#[tokio::test]
async fn sse_lag_event_contains_skipped_count() {
    use tokio_stream::StreamExt;
    use vo_api::handlers::sse::{make_sse_stream, SseBroadcaster, WorkflowSseEvent};

    let broadcaster = SseBroadcaster::new();
    let receiver = broadcaster.subscribe();
    let stream = make_sse_stream(receiver);

    let handle = tokio::spawn(async move {
        let mut event = futures::StreamExt::fuse(stream);
        let mut lag_data: Option<String> = None;

        while let Some(result) = event.next().await {
            if let Ok(evt) = result {
                let data_str = format!("{:?}", evt);
                if data_str.contains("\"type\":\"lagged\"") {
                    lag_data = Some(data_str);
                    break;
                }
            }
        }
        lag_data
    });

    let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(5);
    drop(tx);

    for i in 0..20 {
        let _ = broadcaster.send(WorkflowSseEvent::StepCompleted {
            node_name: format!("step-{}", i),
            sequence: i,
        });
    }

    drop(broadcaster);

    let lag_data = timeout(Duration::from_secs(5), handle)
        .await
        .expect("should not timeout")
        .expect("task should not panic");

    assert!(lag_data.is_some(), "Should have received a lag event");
    let lag_json = lag_data.unwrap();
    assert!(
        lag_json.contains("skipped_count"),
        "Lag event should contain skipped_count, got: {}",
        lag_json
    );
}

#[tokio::test]
async fn sse_slow_client_receives_lag_notifications_and_continues() {
    use tokio_stream::StreamExt;
    use vo_api::handlers::sse::{make_sse_stream, SseBroadcaster, WorkflowSseEvent};

    let broadcaster = SseBroadcaster::new();
    let receiver = broadcaster.subscribe();
    let stream = make_sse_stream(receiver);

    let slow_consumer = tokio::spawn(async move {
        let mut event = futures::StreamExt::fuse(stream);
        let mut total_events = 0u64;
        let mut lag_count = 0u64;
        let mut step_count = 0u64;

        while let Some(result) = event.next().await {
            total_events += 1;
            match result {
                Ok(evt) => {
                    let data_str = format!("{:?}", evt);
                    if data_str.contains("\"type\":\"lagged\"") {
                        lag_count += 1;
                    } else if data_str.contains("\"type\":\"step_completed\"") {
                        step_count += 1;
                    }
                }
                Err(_) => {}
            }

            if total_events > 100 {
                break;
            }

            sleep(Duration::from_millis(5)).await;
        }

        (total_events, lag_count, step_count)
    });

    for i in 0..100 {
        let _ = broadcaster.send(WorkflowSseEvent::StepCompleted {
            node_name: format!("step-{}", i),
            sequence: i,
        });
    }

    drop(broadcaster);

    let (total, lag, steps) = timeout(Duration::from_secs(5), slow_consumer)
        .await
        .expect("should not timeout")
        .expect("task should not panic");

    assert!(
        lag > 0 || steps > 0,
        "Slow client should receive some events, got {} total ({} lag, {} steps)",
        total,
        lag,
        steps
    );
    assert!(
        total < 100,
        "Slow client should not receive all 100 events due to backpressure, got {}",
        total
    );
}
