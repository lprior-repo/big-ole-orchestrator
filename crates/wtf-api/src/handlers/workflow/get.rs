use std::time::Duration;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use wtf_actor::OrchestratorMsg;

use crate::types::{ApiError, V3StatusResponse};
use crate::handlers::helpers::{split_path_id, paradigm_to_str, phase_to_str};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// GET /api/v1/workflows/:id — get instance status (bead wtf-016l).
pub async fn get_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (namespace, instance_id) = match split_path_id(&id) {
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

    let call_result = master
        .call(
            |tx| OrchestratorMsg::GetStatus {
                instance_id,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    match call_result {
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new("actor_unavailable", e.to_string())),
        )
            .into_response(),
        Ok(CallResult::Timeout) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "actor_timeout",
                "orchestrator did not respond",
            )),
        )
            .into_response(),
        Ok(CallResult::SenderError) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "actor_error",
                "orchestrator dropped the reply",
            )),
        )
            .into_response(),
        Ok(CallResult::Success(None)) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "not_found",
                format!(
                    "instance {namespace}/{instance_id_str} not found",
                    instance_id_str = id
                ),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Some(snapshot))) => (
            StatusCode::OK,
            Json(V3StatusResponse {
                instance_id: snapshot.instance_id.to_string(),
                namespace: snapshot.namespace.to_string(),
                workflow_type: snapshot.workflow_type,
                paradigm: paradigm_to_str(snapshot.paradigm).to_owned(),
                phase: phase_to_str(snapshot.phase).to_owned(),
                events_applied: snapshot.events_applied,
            }),
        )
            .into_response(),
    }
}
