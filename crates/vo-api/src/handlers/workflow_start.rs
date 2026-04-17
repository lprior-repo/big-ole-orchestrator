use std::time::Duration;
use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    response::IntoResponse,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use ulid::Ulid;
use vo_actor::{OrchestratorMsg, StartError};
use vo_common::NamespaceId;

use crate::types::{ApiError, V3StartRequest, V3StartResponse, WorkloadRejectionError};
use crate::handlers::helpers::parse_paradigm;

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows — start a new workflow instance (bead vo-7mif).
#[tracing::instrument(skip_all)]
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    let _dedupe_key = match req.dedupe_key {
        Some(ref key) if !key.is_empty() => key.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "missing_dedupe_key",
                    "dedupe_key is required for exact workflow ingress (ADR-028)",
                )),
            )
                .into_response();
        }
    };

    let namespace = NamespaceId::from(req.namespace);

    let paradigm = match parse_paradigm(&req.paradigm) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_paradigm",
                    format!(
                        "paradigm must be 'fsm', 'dag', or 'procedural', got: {:?}",
                        req.paradigm
                    ),
                )),
            )
                .into_response();
        }
    };

    let instance_id_str = match req.instance_id {
        Some(ref id) => id.clone(),
        None => Ulid::new().to_string(),
    };
    let instance_id = vo_types::InstanceId::parse(&instance_id_str).expect("generated ULID should be valid");

    let input = match serde_json::to_vec(&req.input) {
        Ok(v) => Bytes::from(v),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_input",
                    format!("failed to encode input: {e}"),
                )),
            )
                .into_response();
        }
    };

    let workflow_type = req.workflow_type.clone();
    let captured_namespace = namespace.clone();
    let captured_id = instance_id.clone();

    let call_result = master
        .call(
            |tx| OrchestratorMsg::StartWorkflow {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
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
                "orchestrator did not respond in time",
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
        Ok(CallResult::Success(Err(StartError::AtCapacity { running, max }))) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError::new(
                "at_capacity",
                format!("engine at capacity: {running}/{max} instances running"),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::AlreadyExists(id)))) => (
            StatusCode::CONFLICT,
            Json(ApiError::new(
                "already_exists",
                format!("instance {id} already exists"),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::SpawnFailed(msg)))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("spawn_failed", msg)),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::InvalidConfig(msg)))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("invalid_config", msg)),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::BudgetExhaustion { class, requested, available }))) => {
            let rejection = WorkloadRejectionError::BudgetExhausted {
                class: class.to_string(),
                requested,
                available,
            };
            (
                StatusCode::from_u16(rejection.status_code()).unwrap_or(StatusCode::TOO_MANY_REQUESTS),
                Json(ApiError::new(rejection.error_code(), rejection.to_string())),
            )
                .into_response()
        }
        Ok(CallResult::Success(Ok(_))) => (
            StatusCode::CREATED,
            Json(V3StartResponse {
                instance_id: captured_id.to_string(),
                namespace: captured_namespace.to_string(),
                workflow_type: req.workflow_type,
            }),
        )
            .into_response(),
    }
}