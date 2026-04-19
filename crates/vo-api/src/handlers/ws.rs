use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tokio::sync::broadcast;

use super::split_path_id;
use crate::types::ApiError;
pub use crate::types::events::WorkflowEvent;

const WS_BROADCAST_CAPACITY: usize = 1000;

#[derive(Clone)]
pub struct WsBroadcaster {
    tx: broadcast::Sender<WorkflowEvent>,
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(WS_BROADCAST_CAPACITY);
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

impl Default for WsBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct WsState {
    pub broadcaster: WsBroadcaster,
}

impl WsState {
    pub fn new() -> Self {
        Self {
            broadcaster: WsBroadcaster::new(),
        }
    }
}

impl Default for WsState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WsConnectionCount {
    pub active_connections: std::sync::atomic::AtomicUsize,
}

impl WsConnectionCount {
    pub fn new() -> Self {
        Self {
            active_connections: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl Default for WsConnectionCount {
    fn default() -> Self {
        Self::new()
    }
}

impl WsConnectionCount {
    pub fn increment(&self) -> usize {
        self.active_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub fn decrement(&self) -> usize {
        self.active_connections
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst)
    }
}

/// GET /api/v1/ws/:instance_id — WebSocket stream for workflow live updates.
///
/// WebSocket endpoint for real-time workflow event streaming.
/// Supports bidirectional communication but primarily pushes events to clients.
///
/// Connection is maintained until client disconnects or instance completes/fails.
#[tracing::instrument(skip_all)]
pub async fn ws_workflow(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    State(state): State<WsState>,
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

    let broadcaster = state.broadcaster.clone();

    ws.on_upgrade(move |socket| async move {
        let mut ws = socket;
        let mut receiver = broadcaster.subscribe();

        loop {
            tokio::select! {
                recv_result = receiver.recv() => {
                    match recv_result {
                        Ok(event) => {
                            let msg = axum::extract::ws::Message::Text(event.to_json_string().into());
                            if ws.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
                msg = ws.recv() => {
                    match msg {
                        Some(Ok(axum::extract::ws::Message::Close(_))) => break,
                        Some(Ok(axum::extract::ws::Message::Ping(data))) => {
                            let _ = ws.send(axum::extract::ws::Message::Pong(data)).await;
                        }
                        Some(Ok(axum::extract::ws::Message::Text(text))) => {
                            tracing::debug!(msg = %text, "Received WebSocket message");
                        }
                        Some(Ok(axum::extract::ws::Message::Binary(_))) => {}
                        Some(Ok(axum::extract::ws::Message::Pong(_))) => {}
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "WebSocket receive error");
                            break;
                        }
                        None => break,
                    }
                }
            }
        }
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_event_step_completed_serializes_correctly() {
        let event = WorkflowEvent::StepCompleted {
            node_name: "build-step".to_string(),
            sequence: 42,
        };
        let json = event.to_json_string();
        assert!(json.contains("\"type\":\"step_completed\""));
        assert!(json.contains("\"node_name\":\"build-step\""));
        assert!(json.contains("\"sequence\":42"));
    }

    #[test]
    fn ws_event_timer_fired_serializes_correctly() {
        let event = WorkflowEvent::TimerFired {
            timer_id: "timer-123".to_string(),
        };
        let json = event.to_json_string();
        assert!(json.contains("\"type\":\"timer_fired\""));
        assert!(json.contains("\"timer_id\":\"timer-123\""));
    }

    #[test]
    fn ws_broadcaster_creates_with_capacity() {
        let broadcaster = WsBroadcaster::new();
        let receiver = broadcaster.subscribe();
        assert!(receiver.is_empty());
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
}
