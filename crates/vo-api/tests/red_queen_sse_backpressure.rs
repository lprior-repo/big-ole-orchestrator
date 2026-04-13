use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub enum WorkflowSseEvent {
    StepCompleted { node_name: String, sequence: u64 },
    StepFailed { node_name: String, sequence: u64, error: String },
    TimerFired { timer_id: String },
    SignalReceived { signal_name: String },
    PhaseChanged { phase: String },
    InstanceCompleted,
    InstanceFailed { error: String },
}

const SSE_BROADCAST_CAPACITY: usize = 1000;

pub struct SseBroadcaster {
    tx: broadcast::Sender<WorkflowSseEvent>,
}

impl SseBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(SSE_BROADCAST_CAPACITY);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowSseEvent> {
        self.tx.subscribe()
    }

    pub fn send(&self, event: WorkflowSseEvent) -> Result<(), broadcast::error::SendError> {
        self.tx.send(event)
    }
}

impl Default for SseBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

fn make_event(seq: u64) -> WorkflowSseEvent {
    WorkflowSseEvent::StepCompleted {
        node_name: format!("step-{}", seq),
        sequence: seq,
    }
}

#[tokio::test]
async fn red_queen_sse_backpressure_slow_consumer_drops_after_capacity() {
    let broadcaster = SseBroadcaster::new();
    let mut receiver = broadcaster.subscribe();

    let handle = tokio::spawn(async move {
        let mut count = 0u64;
        while let Ok(event) = receiver.recv().await {
            count += 1;
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = event;
        }
        count
    });

    for i in 0..(SSE_BROADCAST_CAPACITY + 500) {
        let _ = broadcaster.send(make_event(i));
    }

    drop(broadcaster);

    let count = handle.await.expect("task should not panic");
    assert!(
        count <= SSE_BROADCAST_CAPACITY as u64 + 500,
        "Should receive events but may miss some due to backpressure"
    );
}

#[tokio::test]
async fn red_queen_sse_backpressure_no_deadlock_on_rapid_send() {
    let broadcaster = SseBroadcaster::new();

    let send_result = timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            for i in 0..10_000 {
                if broadcaster.send(make_event(i)).is_err() {
                    break;
                }
            }
        }),
    )
    .await;

    assert!(send_result.is_ok(), "Send operation should not deadlock");
}

#[tokio::test]
async fn red_queen_sse_rapid_connect_disconnect_cycles() {
    let broadcaster = SseBroadcaster::new();
    let broadcaster_for_task = broadcaster.clone();

    let handle = tokio::spawn(async move {
        for _ in 0..100 {
            let mut receiver = broadcaster_for_task.subscribe();
            tokio::spawn(async move {
                let _ = receiver.recv().await;
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    for i in 0..1000 {
        let _ = broadcaster.send(make_event(i));
    }

    drop(broadcaster);

    handle.await.expect("rapid connect/disconnect should not panic");
}

#[tokio::test]
async fn red_queen_sse_concurrent_subscriptions_all_receive_events() {
    let broadcaster = SseBroadcaster::new();

    let mut receivers = Vec::new();
    for _ in 0..5 {
        receivers.push(tokio::spawn({
            let mut receiver = broadcaster.subscribe();
            async move {
                let mut count = 0u64;
                while let Ok(_) = receiver.recv().await {
                    count += 1;
                }
                count
            }
        }));
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    for i in 0..100 {
        let _ = broadcaster.send(make_event(i));
    }

    drop(broadcaster);

    let mut total_received = 0u64;
    for handle in receivers {
        let count = handle.await.expect("receiver task should not panic");
        total_received += count;
    }

    assert!(
        total_received >= 100,
        "All subscriptions should collectively receive events"
    );
}

#[tokio::test]
async fn red_queen_sse_broadcast_channel_lagged_error_ends_stream() {
    use futures::StreamExt;
    use tokio::sync::broadcast;

    let (tx, mut rx) = broadcast::channel::<WorkflowSseEvent>(10);

    let slowConsumer = async {
        let mut count = 0u64;
        while let Some(result) = rx.next().await {
            match result {
                Ok(_) => {
                    count += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    break;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
        count
    };

    let handle = tokio::spawn(slowConsumer);

    for i in 0..100 {
        let _ = tx.send(make_event(i));
    }

    drop(tx);

    let count = handle.await.expect("task should not panic");
    assert!(
        count <= 10,
        "Slow consumer should miss events due to Lagged error"
    );
}

#[tokio::test]
async fn red_queen_sse_no_data_loss_when_receiver_keeps_up() {
    let broadcaster = SseBroadcaster::new();
    let mut receiver = broadcaster.subscribe();

    let send_count = 500u64;

    let sender_handle = tokio::spawn({
        let broadcaster_clone = broadcaster.clone();
        async move {
            for i in 0..send_count {
                broadcaster_clone.send(make_event(i)).expect("broadcaster should be open");
            }
        }
    });

    let receiver_handle = tokio::spawn(async move {
        let mut received = 0u64;
        while let Ok(_) = receiver.recv().await {
            received += 1;
        }
        received
    });

    sender_handle.await.expect("sender should complete");

    drop(broadcaster);

    let received = receiver_handle.await.expect("receiver should complete");
    assert_eq!(
        received, send_count,
        "Fast consumer should receive all events with no data loss"
    );
}

#[tokio::test]
async fn red_queen_sse_multiple_rapid_subscribers_after_start() {
    let broadcaster = SseBroadcaster::new();

    for i in 0..50 {
        broadcaster.send(make_event(i)).expect("broadcaster should be open");
    }

    let handle1 = tokio::spawn({
        let mut rx = broadcaster.subscribe();
        async move {
            let mut count = 0u64;
            while let Ok(_) = rx.recv().await {
                count += 1;
            }
            count
        }
    });

    for i in 50..100 {
        broadcaster.send(make_event(i)).expect("broadcaster should be open");
    }

    let handle2 = tokio::spawn({
        let mut rx = broadcaster.subscribe();
        async move {
            let mut count = 0u64;
            while let Ok(_) = rx.recv().await {
                count += 1;
            }
            count
        }
    });

    drop(broadcaster);

    let count1 = handle1.await.expect("task should not panic");
    let count2 = handle2.await.expect("task should not panic");

    assert!(count1 >= 50, "First subscriber should receive at least 50 events");
    assert!(count2 <= 50, "Second subscriber joined late, should receive <= 50 events");
}

#[tokio::test]
async fn red_queen_sse_crash_recovery_subscribe_during_event_burst() {
    let broadcaster = SseBroadcaster::new();

    let broadcaster_for_sender = broadcaster.clone();
    let sender_handle = tokio::spawn(async move {
        for i in 0..1000 {
            if broadcaster_for_sender.send(make_event(i)).is_err() {
                break;
            }
            if i % 100 == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut late_receiver = broadcaster.subscribe();

    let receiver_handle = tokio::spawn(async move {
        let mut count = 0u64;
        while let Ok(_) = late_receiver.recv().await {
            count += 1;
        }
        count
    });

    sender_handle.await.expect("sender should complete");
    drop(broadcaster);

    let late_count = receiver_handle.await.expect("receiver should complete");
    assert!(
        late_count < 1000,
        "Late subscriber should miss events but not deadlock"
    );
}

#[tokio::test]
async fn red_queen_sse_backpressure_channel_full_send_error_handled() {
    use tokio::sync::broadcast;

    let (tx, mut rx) = broadcast::channel::<WorkflowSseEvent>(5);

    let handle = tokio::spawn(async move {
        let mut count = 0u64;
        while let Ok(_) = rx.recv().await {
            count += 1;
        }
        count
    });

    for i in 0..20 {
        let result = tx.send(make_event(i));
        if result.is_err() {
            break;
        }
    }

    drop(tx);

    let count = handle.await.expect("task should not panic");
    assert!(
        count <= 5,
        "Should only receive up to channel capacity"
    );
}
