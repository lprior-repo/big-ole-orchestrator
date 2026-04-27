use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::ActorRef;
use serde::Serialize;
use std::sync::Arc;
use vo_actor::OrchestratorMsg;
use vo_storage::event_log::replay_events_in_namespace;
use vo_types::{EventEnvelope, InstanceId};

use crate::types::ApiError;

#[derive(Debug, Serialize)]
struct EventHistoryResponse {
    instance_id: String,
    total_replayed: usize,
    events: Vec<EventEnvelope>,
}

/// GET /api/v1/workflows/:id/events where `id` is `namespace/instance_id`.
pub async fn get_events(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(db): Extension<Arc<fjall::Database>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some((namespace, instance_id)) = id.split_once('/') else {
        return invalid_id_response("id must be namespace/instance_id");
    };
    replay_response(&db, namespace, instance_id)
}

/// GET /api/v1/workflows/:namespace/:id/events.
pub async fn get_events_namespaced(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(db): Extension<Arc<fjall::Database>>,
    Path((namespace, instance_id)): Path<(String, String)>,
) -> impl IntoResponse {
    replay_response(&db, &namespace, &instance_id)
}

fn replay_response(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &str,
) -> axum::response::Response {
    let instance = match InstanceId::parse(instance_id) {
        Ok(instance) => instance,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_id", error.to_string())),
            )
                .into_response();
        }
    };

    let events =
        match replay_events_in_namespace(db, namespace, &instance).collect::<Result<Vec<_>, _>>() {
            Ok(events) => events,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new("event_replay_failed", error.to_string())),
                )
                    .into_response();
            }
        };

    let total_replayed = events.len();
    Json(EventHistoryResponse {
        instance_id: format!("{namespace}/{instance_id}"),
        total_replayed,
        events,
    })
    .into_response()
}

fn invalid_id_response(message: &str) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError::new("invalid_id", message)),
    )
        .into_response()
}
