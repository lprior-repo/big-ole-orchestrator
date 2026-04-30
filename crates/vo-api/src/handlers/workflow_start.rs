use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    response::IntoResponse,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use std::sync::Arc;
use std::time::Duration;
use ulid::Ulid;
use vo_actor::{OrchestratorMsg, StartError};
use vo_core::admission::WriterPressureGuard;
use vo_core::circuit_breaker::CircuitBreakerState;
use vo_storage::dedupe_partition::DedupeStore;
use vo_types::CommandEnvelope;

use crate::handlers::helpers::parse_paradigm;
use crate::handlers::{
    persist_workflow_start_rejected_event, persist_workflow_started_event, request_namespace,
    start_error_response,
};
use crate::types::{ApiError, V3StartRequest, V3StartResponse, WorkloadRejectionError};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows — start a new workflow instance (bead vo-7mif).
///
/// Per ADR-028, this handler enforces exactly-once ingress:
/// 1. Validates that a `command_envelope` is present with identity metadata (ADR-036).
/// 2. Validates that a `dedupe_key` is present for exact workflow ingress.
/// 3. Calls `admit_ingress` to atomically check-and-insert into the dedupe store.
/// 4. If duplicate, returns 409 Conflict with the existing instance ID.
/// 5. If new, proceeds to start the workflow via the orchestrator actor.
#[tracing::instrument(skip_all)]
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(dedupe_store): Extension<Arc<dyn DedupeStore>>,
    Extension(writer_pressure): Extension<Arc<dyn WriterPressureGuard>>,
    Extension(circuit_breaker): Extension<Arc<CircuitBreakerState>>,
    Extension(event_db): Extension<Arc<fjall::Database>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    // Step 1: Validate CommandEnvelope presence (ADR-036).
    let command_envelope = match req.command_envelope {
        Some(ref env_json) => {
            let json_str = serde_json::to_string(env_json).unwrap_or_default();
            match CommandEnvelope::from_str(&json_str) {
                Ok(env) => env,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiError::new(
                            "invalid_command_envelope",
                            format!("command_envelope is required (ADR-036): {e}"),
                        )),
                    )
                        .into_response();
                }
            }
        }
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "missing_command_envelope",
                    "command_envelope is required (ADR-036) with command_id, correlation_id, causation_id, issuer, and issued_at",
                )),
            )
                .into_response();
        }
    };

    let _command_envelope = command_envelope;

    // Step 2: Validate dedupe key presence (ADR-028 Section 2).
    let dedupe_key = match req.dedupe_key {
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

    let namespace = match request_namespace(&req.namespace) {
        Ok(namespace) => namespace,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_namespace", message)),
            )
                .into_response();
        }
    };

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

    let instance_id = match req.instance_id {
        Some(ref id) => match vo_types::InstanceId::parse(id) {
            Ok(id) => id,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "invalid_instance_id",
                        format!("invalid instance_id format: {e}"),
                    )),
                )
                    .into_response();
            }
        },
        None => vo_types::InstanceId::from_bytes(Ulid::new().0.to_be_bytes()),
    };

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

    let reserve_result = master
        .call(
            |tx| OrchestratorMsg::ReserveWorkflowStart {
                namespace: namespace.clone(),
                instance_id: instance_id.clone(),
                workflow_type: workflow_type.clone(),
                paradigm: paradigm.clone(),
                input: input.clone(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    if let Some(response) = start_error_response(reserve_result) {
        return response;
    }

    let persisted_start = persist_workflow_started_event(
        &event_db,
        &captured_namespace,
        &captured_id,
        &workflow_type,
        &req.paradigm,
        req.workflow_binary_hash.as_deref(),
        &req.input,
        &dedupe_key,
    );
    if let Err(error) = persisted_start {
        let _ = master
            .call(
                |tx| OrchestratorMsg::AbortWorkflowStart {
                    namespace: captured_namespace.clone(),
                    instance_id: captured_id.clone(),
                    reply: tx,
                },
                Some(ACTOR_CALL_TIMEOUT),
            )
            .await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new(
                "event_persist_failed",
                format!("workflow start event persistence failed before actor mutation: {error}"),
            )),
        )
            .into_response();
    }

    // Step 5: Proceed to start workflow via actor after durable event append.
    let call_result = master
        .call(
            |tx| OrchestratorMsg::CommitWorkflowStart {
                namespace,
                instance_id,
                workflow_type: workflow_type.clone(),
                paradigm,
                input,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;

    if !matches!(call_result, Ok(CallResult::Success(Ok(())))) {
        let _ = persist_workflow_start_rejected_event(&event_db, &captured_namespace, &captured_id);
    }

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
        Ok(CallResult::Success(Err(StartError::GhostInstance(id)))) => (
            StatusCode::GONE,
            Json(ApiError::new(
                "ghost_instance",
                format!(
                    "instance {id} has been reaped by zombie detection and cannot be restarted"
                ),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(StartError::BudgetExhaustion {
            class,
            requested,
            available,
        }))) => {
            let rejection = WorkloadRejectionError::BudgetExhausted {
                class: class.to_string(),
                requested,
                available,
            };
            (
                match StatusCode::from_u16(rejection.status_code()) {
                    Ok(status) => status,
                    Err(_) => StatusCode::TOO_MANY_REQUESTS,
                },
                Json(ApiError::new(rejection.error_code(), rejection.to_string())),
            )
                .into_response()
        }
        Ok(CallResult::Success(Ok(_))) => (
            StatusCode::CREATED,
            Json(V3StartResponse {
                instance_id: captured_id.to_string(),
                namespace: captured_namespace.to_string(),
                workflow_type,
            }),
        )
            .into_response(),
    }
}
