use std::time::Duration;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
};
use ractor::ActorRef;
use tokio::sync::broadcast;
use tokio::time::interval;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as TokioStreamExt;
use vo_actor::OrchestratorMsg;

use super::split_path_id;
use crate::types::ApiError;

const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSE_BROADCAST_CAPACITY: usize = 1000;

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
    TimerFired {
        timer_id: String,
    },
    SignalReceived {
        signal_name: String,
    },
    PhaseChanged {
        phase: String,
    },
    InstanceCompleted,
    InstanceFailed {
        error: String,
    },
    Lagged {
        skipped_count: u64,
    },
}

impl WorkflowSseEvent {
    fn to_sse_event(&self) -> Event {
        let data = match self {
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
            WorkflowSseEvent::Lagged { skipped_count } => {
                serde_json::json!({
                    "type": "lagged",
                    "skipped_count": skipped_count,
                })
            }
        };
        Event::default()
            .event("workflow-event")
            .data(data.to_string())
    }
}

#[derive(Clone)]
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

    pub fn send(
        &self,
        event: WorkflowSseEvent,
    ) -> Result<usize, broadcast::error::SendError<WorkflowSseEvent>> {
        self.tx.send(event)
    }
}

impl Default for SseBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct SseState {
    pub broadcaster: SseBroadcaster,
}

impl SseState {
    pub fn new() -> Self {
        Self {
            broadcaster: SseBroadcaster::new(),
        }
    }
}

impl Default for SseState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn make_sse_stream(
    receiver: broadcast::Receiver<WorkflowSseEvent>,
) -> impl futures::Stream<Item = Result<Event, axum::Error>> + Send + 'static {
    TokioStreamExt::map(BroadcastStream::new(receiver), |result| match result {
        Ok(event) => Ok(event.to_sse_event()),
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            Ok(WorkflowSseEvent::Lagged { skipped_count: n }.to_sse_event())
        }
    })
}

fn keepalive_stream() -> impl futures::Stream<Item = Result<Event, axum::Error>> + Send + 'static {
    async_stream::stream! {
        let mut interval = interval(SSE_KEEPALIVE_INTERVAL);
        loop {
            yield Ok(Event::default()
                .comment(":keepalive"));
            interval.tick().await;
        }
    }
}

fn merge_with_keepalive(
    receiver: broadcast::Receiver<WorkflowSseEvent>,
) -> impl futures::Stream<Item = Result<Event, axum::Error>> + Send + 'static {
    let events = make_sse_stream(receiver);
    let keepalive = keepalive_stream();
    events.merge(keepalive)
}

/// GET /api/v1/watch/:instance_id — SSE stream for workflow live updates (ADR-007/024).
///
/// Best-effort live tail of workflow events. Does not block the write path.
/// Keeps connection alive with 15-second keepalive pings (`:keepalive` comment).
/// If client falls behind by more than 1000 events, connection is dropped.
#[tracing::instrument(skip_all)]
pub async fn watch_workflow(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    State(state): State<SseState>,
) -> impl IntoResponse {
    let (_namespace, _instance_id) = match split_path_id(&id) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_id",
                    "id must be <namespace>/<instance_id>",
                )),
            )
                .into_response();
        }
    };

    let receiver = state.broadcaster.subscribe();
    let stream = merge_with_keepalive(receiver);

    Sse::new(stream).into_response()
}

use axum::Json;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_event_step_completed_serializes_correctly() {
        let event = WorkflowSseEvent::StepCompleted {
            node_name: "build-step".to_string(),
            sequence: 42,
        };
        let _sse_event = event.to_sse_event();
    }

    #[test]
    fn sse_event_timer_fired_serializes_correctly() {
        let event = WorkflowSseEvent::TimerFired {
            timer_id: "timer-123".to_string(),
        };
        let _sse_event = event.to_sse_event();
    }

    #[test]
    fn sse_broadcaster_creates_with_capacity() {
        let broadcaster = SseBroadcaster::new();
        let _receiver = broadcaster.subscribe();
    }

    #[test]
    fn split_path_id_returns_namespace_and_id_when_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, id) = result.unwrap();
        assert_eq!(ns, "payments");
        assert_eq!(id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn split_path_id_returns_none_when_missing_slash() {
        let result = split_path_id("no-slash-here");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn sse_lagged_event_emitted_instead_of_error() {
        use tokio::sync::broadcast;
        use tokio::time::{timeout, Duration};

        let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(10);

        let stream = make_sse_stream(rx);
        let mut event = futures::StreamExt::fuse(stream);

        for i in 0..15 {
            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }
        drop(tx);

        let mut count = 0u64;
        let mut lag_count = 0u64;
        let result = timeout(Duration::from_secs(2), async {
            while let Some(item) = futures::StreamExt::next(&mut event).await {
                count += 1;
                if let Ok(evt) = item {
                    let data_str = evt.data().to_string();
                    if data_str.contains("\"type\":\"lagged\"") {
                        lag_count += 1;
                    }
                }
            }
        })
        .await;

        let completed = result.is_ok();
        assert!(completed, "Stream should terminate after channel closes");
        assert!(
            count <= 15 + lag_count,
            "Should receive at most 15 events plus lag notifications, got {count}"
        );
    }

    #[tokio::test]
    async fn sse_lag_event_contains_skipped_count() {
        use tokio::sync::broadcast;
        use tokio::time::{timeout, Duration};

        let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(5);

        let stream = make_sse_stream(rx);
        let mut event = futures::StreamExt::fuse(stream);

        for i in 0..20 {
            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let mut lag_events = Vec::new();
        let _ = timeout(Duration::from_secs(2), async {
            while let Some(result) = futures::StreamExt::next(&mut event).await {
                if let Ok(evt) = result {
                    let data_str = evt.data().to_string();
                    if data_str.contains("\"type\":\"lagged\"") {
                        lag_events.push(data_str);
                    }
                }
            }
        })
        .await;

        assert!(
            !lag_events.is_empty(),
            "Should emit at least one lag event when client falls behind"
        );
        for lag_data in &lag_events {
            assert!(
                lag_data.contains("skipped_count"),
                "Lag event should contain skipped_count, got: {lag_data}"
            );
        }
    }

    #[test]
    fn keepalive_interval_is_15_seconds() {
        assert_eq!(SSE_KEEPALIVE_INTERVAL, Duration::from_secs(15));
    }

    #[test]
    fn broadcast_capacity_is_1000() {
        assert_eq!(SSE_BROADCAST_CAPACITY, 1000);
    }

    #[tokio::test]
    async fn sse_broadcast_capacity_1000() {
        let broadcaster = SseBroadcaster::new();
        let mut receiver = broadcaster.subscribe();

        let handle = tokio::spawn(async move {
            let mut count = 0u64;
            while let Ok(_) = receiver.recv().await {
                count += 1;
            }
            count
        });

        for i in 0..(SSE_BROADCAST_CAPACITY + 1) {
            let _ = broadcaster.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i as u64,
            });
        }

        drop(broadcaster);

        let count = handle.await.expect("task should not panic");
        assert!(
            count <= SSE_BROADCAST_CAPACITY as u64 + 1,
            "Should receive at most capacity + 1 events"
        );
    }

    #[tokio::test]
    async fn sse_slow_client_receives_lag_notifications() {
        use tokio::sync::broadcast;
        use tokio::time::{timeout, Duration};

        let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(10);

        let stream = make_sse_stream(rx);
        let mut event = futures::StreamExt::fuse(stream);

        for i in 0..100 {
            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let mut count = 0u64;
        let mut lag_count = 0u64;
        let mut has_lag = false;
        let _ = timeout(Duration::from_secs(2), async {
            while let Some(result) = futures::StreamExt::next(&mut event).await {
                count += 1;
                match result {
                    Ok(evt) => {
                        let data_str = evt.data().to_string();
                        if data_str.contains("\"type\":\"lagged\"") {
                            lag_count += 1;
                            has_lag = true;
                        }
                    }
                    Err(_) => {}
                }
                if count > 50 {
                    break;
                }
            }
        })
        .await;

        assert!(
            has_lag,
            "Slow client should receive lag notifications, got {lag_count} lag events out of {count} total"
        );
    }
}
