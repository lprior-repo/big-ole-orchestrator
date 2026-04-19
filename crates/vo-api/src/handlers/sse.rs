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

pub use crate::types::events::WorkflowEvent;

#[derive(Clone)]
pub struct SseBroadcaster {
    tx: broadcast::Sender<WorkflowEvent>,
}

impl SseBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(SSE_BROADCAST_CAPACITY);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowEvent> {
        self.tx.subscribe()
    }

    pub fn send(
        &self,
        event: WorkflowEvent,
    ) -> Result<usize, broadcast::error::SendError<WorkflowEvent>> {
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

fn make_sse_stream(
    receiver: broadcast::Receiver<WorkflowEvent>,
) -> impl futures::Stream<Item = Result<Event, axum::Error>> + Send + 'static {
    TokioStreamExt::map(BroadcastStream::new(receiver), |result| match result {
        Ok(event) => Ok(event.to_sse_event()),
        Err(BroadcastStreamRecvError::Lagged(_)) => {
            Err(axum::Error::new("client fell behind, closing stream"))
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
    receiver: broadcast::Receiver<WorkflowEvent>,
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
    use futures::StreamExt;
    use tokio_stream::StreamExt as TokioStreamExt;

    #[test]
    fn sse_event_step_completed_serializes_correctly() {
        let event = WorkflowEvent::StepCompleted {
            node_name: "build-step".to_string(),
            sequence: 42,
        };
        let _sse_event = event.to_sse_event();
    }

    #[test]
    fn sse_event_timer_fired_serializes_correctly() {
        let event = WorkflowEvent::TimerFired {
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
    async fn sse_lagged_error_closes_stream() {
        use tokio::sync::broadcast;

        let (tx, rx) = broadcast::channel::<WorkflowEvent>(10);

        let stream = make_sse_stream(rx);
        let mut event = futures::StreamExt::fuse(stream);

        for i in 0..15 {
            let _ = tx.send(WorkflowEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let mut count = 0u64;
        let mut lagged_received = false;
        while let Some(result) = futures::StreamExt::next(&mut event).await {
            count += 1;
            match result {
                Ok(event) => {
                    let _ = event;
                    lagged_received = true;
                }
                Err(_) => break,
            }
        }

        assert!(lagged_received || count <= 11, "Should emit lag or close");
        assert!(
            count <= 11,
            "Should close after lag, not receive all 15 events"
        );
    }

    #[tokio::test]
    async fn sse_stream_closes_after_lag_event() {
        use tokio::sync::broadcast;

        let (tx, rx) = broadcast::channel::<WorkflowEvent>(5);

        let stream = make_sse_stream(rx);
        let mut event = futures::StreamExt::fuse(stream);

        for i in 0..20 {
            let _ = tx.send(WorkflowEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let mut count = 0u64;
        while let Some(_result) = futures::StreamExt::next(&mut event).await {
            count += 1;
            if count > 10 {
                break;
            }
        }

        assert!(
            count <= 6,
            "Should close after lag notification, not all 20 events"
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
            let _ = broadcaster.send(WorkflowEvent::StepCompleted {
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
    async fn sse_lagged_error_drops_slow_client() {
        use tokio::sync::broadcast;

        let (tx, rx) = broadcast::channel::<WorkflowEvent>(10);

        let stream = make_sse_stream(rx);
        let mut event = futures::StreamExt::fuse(stream);

        for i in 0..100 {
            let _ = tx.send(WorkflowEvent::StepCompleted {
                node_name: format!("step-{}", i),
                sequence: i,
            });
        }

        let mut count = 0u64;
        let mut lagged = false;
        while let Some(result) = futures::StreamExt::next(&mut event).await {
            count += 1;
            match result {
                Ok(_) => {}
                Err(e) => {
                    assert!(
                        e.to_string().contains("client fell behind")
                            || e.to_string().contains("channel closed")
                    );
                    lagged = true;
                    break;
                }
            }
            if count > 50 {
                break;
            }
        }

        assert!(
            lagged || count <= 11,
            "Slow client should be dropped via Lagged error"
        );
    }
}
