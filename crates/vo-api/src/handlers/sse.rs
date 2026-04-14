use std::time::Duration;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
};
use futures::Stream;
use ractor::ActorRef;
use tokio::sync::broadcast;
use vo_actor::OrchestratorMsg;

use crate::types::ApiError;

const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSE_BROADCAST_CAPACITY: usize = 1000;

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

impl WorkflowSseEvent {
    fn to_sse_event(&self) -> Event {
        let data = match self {
            WorkflowSseEvent::StepCompleted { node_name, sequence } => {
                serde_json::json!({
                    "type": "step_completed",
                    "node_name": node_name,
                    "sequence": sequence,
                })
            }
            WorkflowSseEvent::StepFailed { node_name, sequence, error } => {
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

    pub fn send(&self, event: WorkflowSseEvent) -> Result<(), broadcast::error::SendError> {
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

struct SseStream {
    receiver: broadcast::Receiver<WorkflowSseEvent>,
    lag_notified: bool,
    _phantom: std::marker::PhantomData<()>,
}

impl Stream for SseStream {
    type Item = Result<Event, axum::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.lag_notified {
            return std::task::Poll::Ready(None);
        }

        match std::pin::Pin::new(&mut self.receiver).poll_recv(cx) {
            std::task::Poll::Ready(Ok(event)) => {
                std::task::Poll::Ready(Some(Ok(event.to_sse_event())))
            }
            std::task::Poll::Ready(Err(broadcast::error::RecvError::Closed)) => {
                std::task::Poll::Ready(None)
            }
            std::task::Poll::Ready(Err(broadcast::error::RecvError::Lagged(_))) => {
                self.lag_notified = true;
                let lag_event = Event::default().comment("lagged");
                std::task::Poll::Ready(Some(Ok(lag_event)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// GET /api/v1/watch/:instance_id — SSE stream for workflow live updates (ADR-007/024).
///
/// Best-effort live tail of workflow events. Does not block the write path.
/// Keeps connection alive with 15-second keepalive pings.
/// If client falls behind by more than 1000 events, connection is dropped.
#[tracing::instrument(skip_all)]
pub async fn watch_workflow(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    State(state): State<SseState>,
) -> impl IntoResponse {
    let (_, _instance_id) = match split_path_id(&id) {
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

    let stream = SseStream {
        receiver,
        lag_notified: false,
        _phantom: std::marker::PhantomData,
    };

    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(SSE_KEEPALIVE_INTERVAL))
        .into_response()
}

fn split_path_id(path: &str) -> Option<(String, String)> {
    let slash = path.find('/')?;
    let namespace = path[..slash].to_owned();
    let instance_id = path[slash + 1..].to_owned();
    Some((namespace, instance_id))
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
        let sse_event = event.to_sse_event();
        let data = sse_event.data().to_string();
        assert!(data.contains("\"type\":\"step_completed\""));
        assert!(data.contains("\"node_name\":\"build-step\""));
        assert!(data.contains("\"sequence\":42"));
    }

    #[test]
    fn sse_event_timer_fired_serializes_correctly() {
        let event = WorkflowSseEvent::TimerFired {
            timer_id: "timer-123".to_string(),
        };
        let sse_event = event.to_sse_event();
        let data = sse_event.data().to_string();
        assert!(data.contains("\"type\":\"timer_fired\""));
        assert!(data.contains("\"timer_id\":\"timer-123\""));
    }

    #[test]
    fn sse_broadcaster_creates_with_capacity() {
        let broadcaster = SseBroadcaster::new();
        let receiver = broadcaster.subscribe();
        assert!(receiver.is_empty());
    }

    #[test]
    fn split_path_id_returns_namespace_and_id_when_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        if let Some((ns, id)) = result {
            assert_eq!(ns, "payments");
            assert_eq!(id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        }
    }

    #[test]
    fn split_path_id_returns_none_when_missing_slash() {
        let result = split_path_id("no-slash-here");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn sse_lagged_event_emitted_before_stream_closes() {
        use tokio::sync::broadcast;

        let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(10);

        let stream = SseStream {
            receiver: rx,
            lag_notified: false,
            _phantom: std::marker::PhantomData,
        };

        let event = futures::stream::StreamExt::into_async_iter(stream);
        let mut event = Box::pin(event);

        for i in 0..15 {
            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let first = event.next().await;
        assert!(first.is_some(), "Should receive at least one event before lag");

        let mut lag_received = false;
        let mut empty_received = false;
        while let Some(result) = event.next().await {
            match result {
                Ok(event) => {
                    let data_str = event.data().to_string();
                    if data_str.contains(":lagged") {
                        lag_received = true;
                    }
                }
                Err(_) => {}
            }
        }

        assert!(lag_received, "Should emit :lagged comment before closing");
    }

    #[tokio::test]
    async fn sse_stream_closes_after_lag_event() {
        use tokio::sync::broadcast;

        let (tx, rx) = broadcast::channel::<WorkflowSseEvent>(5);

        let stream = SseStream {
            receiver: rx,
            lag_notified: false,
            _phantom: std::marker::PhantomData,
        };

        let event = futures::stream::StreamExt::into_async_iter(stream);
        let mut event = Box::pin(event);

        for i in 0..20 {
            let _ = tx.send(WorkflowSseEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let mut count = 0u64;
        while let Some(_result) = event.next().await {
            count += 1;
        }

        assert!(
            count <= 6,
            "Should receive lag notification and then close, not all 20 events"
        );
    }

    #[test]
    fn keepalive_interval_is_15_seconds() {
        assert_eq!(SSE_KEEPALIVE_INTERVAL, Duration::from_secs(15));
    }

    #[test]
    fn broadcast_capacity_is_1000() {
        assert_eq!(SSE_BROADCAST_CAPACITY, 1000);
    }
}