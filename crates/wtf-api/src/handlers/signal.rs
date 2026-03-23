use super::{split_path_id, ACTOR_CALL_TIMEOUT};
use crate::types::{ApiError, SignalResponse, V3SignalRequest};
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::{ActorRef, MessagingErr};
use wtf_actor::OrchestratorMsg;

/// POST /api/v1/workflows/:id/signals — send a signal to a running instance.
pub async fn send_signal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(req): Json<V3SignalRequest>,
) -> impl IntoResponse {
    let (_, instance_id) = match split_path_id(&id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_id", "malformed path id")),
            )
                .into_response()
        }
    };

    let payload = match serde_json::to_vec(&req.payload) {
        Ok(v) => Bytes::from(v),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_payload", e.to_string())),
            )
                .into_response()
        }
    };

    let res = master
        .call(
            |tx| OrchestratorMsg::Signal {
                instance_id,
                signal_name: req.signal_name.clone(),
                payload,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    map_signal_result(res).into_response()
}

fn map_signal_result(
    res: Result<CallResult<Result<(), wtf_common::WtfError>>, MessagingErr<OrchestratorMsg>>,
) -> impl IntoResponse {
    match res {
        Ok(CallResult::Success(Ok(()))) => (
            StatusCode::ACCEPTED,
            Json(SignalResponse { acknowledged: true }),
        )
            .into_response(),
        Ok(CallResult::Success(Err(wtf_common::WtfError::InstanceNotFound { .. }))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("instance_not_found", "instance not found")),
        )
            .into_response(),
        Ok(CallResult::Success(Err(e))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("actor_error", e.to_string())),
        )
            .into_response(),
        Ok(CallResult::Timeout) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("actor_timeout", "actor call timed out")),
        )
            .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("actor_error", "actor communication failure")),
        )
            .into_response(),
    }
}
