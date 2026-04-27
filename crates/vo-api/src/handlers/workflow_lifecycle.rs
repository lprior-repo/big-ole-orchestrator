use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vo_actor::{OrchestratorMsg, TerminateError};
use vo_core::circuit_breaker::{CircuitBreakerState, UnquarantineResult};
use vo_storage::event_log::{append_event, AppendEventRequest};
use vo_types::events::EventMetadata;
use vo_types::WorkflowName;

use crate::handlers::helpers::split_path_id;
use crate::types::ApiError;

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
    Extension(event_db): Extension<Arc<fjall::Database>>,
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

    let reason = "api-terminate".to_owned();
    let preflight_result = master
        .call(
            |tx| OrchestratorMsg::ReserveTerminate {
                namespace: namespace.clone(),
                instance_id: instance_id.clone(),
                reason: reason.clone(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    if let Some(response) = terminate_rejection(preflight_result) {
        return response;
    }

    if let Err(error) = persist_lifecycle_event(
        &event_db,
        &namespace,
        &instance_id,
        serde_json::json!({
            "type": "WorkflowTerminated",
            "namespace": namespace.clone(),
            "reason": reason.clone(),
        }),
    ) {
        abort_reserved_transition(&master, namespace.clone(), instance_id.clone()).await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("event_persist_failed", error.to_string())),
        )
            .into_response();
    }

    let call_result = master
        .call(
            |tx| OrchestratorMsg::CommitTerminate {
                namespace,
                instance_id,
                reason,
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

async fn abort_reserved_transition(
    master: &ActorRef<OrchestratorMsg>,
    namespace: String,
    instance_id: vo_types::InstanceId,
) {
    match master
        .call(
            |tx| OrchestratorMsg::AbortWorkflowTransition {
                namespace,
                instance_id,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await
    {
        Ok(CallResult::Success(())) => {}
        Ok(other) => tracing::warn!(?other, "failed to abort reserved workflow transition"),
        Err(error) => tracing::warn!(?error, "failed to abort reserved workflow transition"),
    }
}

fn terminate_rejection(
    call_result: Result<
        CallResult<Result<(), TerminateError>>,
        ractor::MessagingErr<OrchestratorMsg>,
    >,
) -> Option<axum::response::Response> {
    match call_result {
        Err(e) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new("actor_unavailable", e.to_string())),
            )
                .into_response(),
        ),
        Ok(CallResult::Timeout) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new(
                    "actor_timeout",
                    "orchestrator did not respond",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::SenderError) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "actor_error",
                    "orchestrator dropped the reply",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(TerminateError::NotFound(id)))) => Some(
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(
                    "not_found",
                    format!("instance {id} not found"),
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(TerminateError::Failed(msg)))) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("terminate_failed", msg)),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Ok(()))) => None,
    }
}

/// POST /api/v1/workflows/:id/unquarantine — manually unquarantine a workflow (ADR-026).
#[tracing::instrument(skip_all)]
pub async fn unquarantine_workflow(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(circuit_breaker): Extension<Arc<CircuitBreakerState>>,
    Path(id): Path<String>,
    Json(req): Json<UnquarantineRequest>,
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

    let workflow_name = match WorkflowName::parse(&instance_id.to_string()) {
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

    let operator = req.operator;

    match vo_core::circuit_breaker::unquarantine(
        &workflow_name,
        &operator,
        circuit_breaker.as_ref(),
    ) {
        Ok(UnquarantineResult {
            workflow_name,
            previous_status,
            new_status,
            failures_cleared,
        }) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "workflow_name": workflow_name.to_string(),
                "previous_status": format!("{:?}", previous_status),
                "new_status": format!("{:?}", new_status),
                "failures_cleared": failures_cleared,
            })),
        )
            .into_response(),
        Err(e) => {
            let (status, error_code, message) = match e {
                vo_core::circuit_breaker::CircuitBreakerError::WorkflowNotFound {
                    workflow_name,
                } => (
                    StatusCode::NOT_FOUND,
                    "workflow_not_found",
                    format!("workflow '{}' not found", workflow_name),
                ),
                vo_core::circuit_breaker::CircuitBreakerError::NotQuarantined {
                    workflow_name,
                    current_status,
                } => (
                    StatusCode::CONFLICT,
                    "not_quarantined",
                    format!(
                        "workflow '{}' is not quarantined (current status: {:?})",
                        workflow_name, current_status
                    ),
                ),
                vo_core::circuit_breaker::CircuitBreakerError::StorageError { reason } => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "storage_error",
                    format!("storage error: {}", reason),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "unquarantine_failed",
                    e.to_string(),
                ),
            };
            (status, Json(ApiError::new(error_code, message))).into_response()
        }
    }
}

/// POST /api/v1/workflows/:id/compensate — trigger manual compensation for a workflow instance.
#[tracing::instrument(skip_all)]
pub async fn compensate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(event_db): Extension<Arc<fjall::Database>>,
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

    let preflight_result = master
        .call(
            |tx| OrchestratorMsg::ReserveCompensate {
                namespace: namespace.clone(),
                instance_id: instance_id.clone(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    if let Some(response) = compensate_rejection(preflight_result) {
        return response;
    }

    if let Err(error) = persist_lifecycle_event(
        &event_db,
        &namespace,
        &instance_id,
        serde_json::json!({
            "type": "WorkflowCompensationInitiated",
            "namespace": namespace.clone(),
        }),
    ) {
        abort_reserved_transition(&master, namespace.clone(), instance_id.clone()).await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("event_persist_failed", error.to_string())),
        )
            .into_response();
    }

    let call_result = master
        .call(
            |tx| OrchestratorMsg::CommitCompensate {
                namespace,
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
        Ok(CallResult::Success(Ok(_))) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "instance_id": id,
                "status": "compensation_initiated",
            })),
        )
            .into_response(),
    }
}

fn compensate_rejection(
    call_result: Result<
        CallResult<Result<(), vo_actor::CompensateError>>,
        ractor::MessagingErr<OrchestratorMsg>,
    >,
) -> Option<axum::response::Response> {
    match call_result {
        Err(e) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new("actor_unavailable", e.to_string())),
            )
                .into_response(),
        ),
        Ok(CallResult::Timeout) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new(
                    "actor_timeout",
                    "orchestrator did not respond",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::SenderError) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    "actor_error",
                    "orchestrator dropped the reply",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(vo_actor::CompensateError::NotFound(id)))) => Some(
            (
                StatusCode::NOT_FOUND,
                Json(ApiError::new(
                    "not_found",
                    format!("instance {id} not found"),
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(vo_actor::CompensateError::Failed(msg)))) => Some(
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("compensation_failed", msg)),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Ok(()))) => None,
    }
}

fn persist_lifecycle_event(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &vo_types::InstanceId,
    payload: serde_json::Value,
) -> Result<(), vo_storage::codec::StorageError> {
    let annotations = HashMap::from([("namespace".to_string(), serde_json::json!(namespace))]);
    append_event(
        db,
        AppendEventRequest {
            namespace: namespace.to_string(),
            instance_id: instance_id.clone(),
            timestamp_ms: now_ms(),
            payload,
            metadata: EventMetadata {
                command_metadata: None,
                annotations,
            },
        },
    )
    .map(|_| ())
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).map_or(u64::MAX, |value| value),
        Err(_) => 0,
    }
}
