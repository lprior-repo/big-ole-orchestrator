use std::time::Duration;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use vo_actor::OrchestratorMsg;

use crate::types::{ApiError, V3SignalRequest};
use crate::handlers::helpers::split_path_id;

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows/:id/signals — send a signal to a running instance (bead vo-meua).
#[tracing::instrument(skip_all)]
pub async fn send_signal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(req): Json<V3SignalRequest>,
) -> impl IntoResponse {
    let (_, instance_id) = match split_path_id(&id) {
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

    // Serialize signal payload to bytes.
    let payload = match serde_json::to_vec(&req.payload) {
        Ok(v) => Bytes::from(v),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_payload",
                    format!("failed to encode payload: {e}"),
                )),
            )
                .into_response();
        }
    };

    // Signal handling requires OrchestratorMsg::Signal variant which is not yet implemented.
    // Return NOT_IMPLEMENTED until the vo-actor signal handling is added.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new(
            "not_implemented",
            "signal handling: OrchestratorMsg::Signal variant not yet implemented",
        )),
    )
        .into_response()
}
