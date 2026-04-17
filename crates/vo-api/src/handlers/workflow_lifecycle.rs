use std::time::Duration;
use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use serde::{Deserialize, Serialize};
use vo_actor::{OrchestratorMsg, TerminateError};
use vo_types::WorkflowName;

use crate::types::ApiError;
use crate::handlers::helpers::split_path_id;

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub struct UnquarantineRequest {
    pub operator: String,
}

#[derive(Debug, Serialize)]
pub struct UnquarantineResponse {
    pub workflow_name: String,
    pub previous_status: String,
    pub new_status: String,
    pub failures_cleared: usize,
}

/// DELETE /api/v1/workflows/:id — terminate a running instance (bead vo-016l).
#[tracing::instrument(skip_all)]
pub async fn terminate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
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

    let call_result = master
        .call(
            |tx| OrchestratorMsg::Terminate {
                instance_id,
                reason: "api-terminate".to_owned(),
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
        Ok(CallResult::Success(Err(TerminateError::NotFound(id)))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "not_found",
                format!("instance {id} not found"),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(TerminateError::Failed(msg)))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("terminate_failed", msg)),
        )
            .into_response(),
        Ok(CallResult::Success(Ok(()))) => StatusCode::NO_CONTENT.into_response(),
    }
}

/// POST /api/v1/workflows/:id/unquarantine — manually unquarantine a workflow (ADR-026).
#[tracing::instrument(skip_all)]
pub async fn unquarantine_workflow(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(_req): Json<UnquarantineRequest>,
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

    let _workflow_name = match WorkflowName::parse(&instance_id.to_string()) {
        Ok(name) => name,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_workflow_name",
                    format!("invalid workflow name: {}", instance_id),
                )),
            )
                .into_response();
        }
    };

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new(
            "not_implemented",
            "circuit breaker state injection required (see bead ve-jfj5)",
        )),
    )
        .into_response()
}

/// POST /api/v1/workflows/:id/compensate — trigger manual compensation for a workflow instance.
#[tracing::instrument(skip_all)]
pub async fn compensate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
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

    let call_result = master
        .call(
            |tx| OrchestratorMsg::Compensate {
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
        Ok(CallResult::Success(Err(_))) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                "compensation_failed",
                "workflow compensation could not be initiated",
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Ok(_))) => (StatusCode::ACCEPTED, Json(serde_json::json!({
            "instance_id": id,
            "status": "compensation_initiated",
        }))).into_response(),
    }
}