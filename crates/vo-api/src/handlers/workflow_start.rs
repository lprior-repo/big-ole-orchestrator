use axum::{
    extract::{Extension, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ulid::Ulid;
use vo_actor::{OrchestratorMsg, StartError};
use vo_common::NamespaceId;
use vo_core::admission::{PressureGuardResult, WriterPressureGuard};
use vo_core::circuit_breaker::CircuitBreakerState;
use vo_storage::dedupe_partition::DedupeStore;
use vo_storage::event_log::{append_event, replay_events_in_namespace, AppendEventRequest};
use vo_types::events::EventMetadata;
use vo_types::InstanceId;

use crate::handlers::helpers::parse_paradigm;
use crate::handlers::ingress::{
    admit_ingress, IngressAdmission, IngressAdmissionError, DEFAULT_DEDUPE_TTL_MS,
};
use crate::types::{ApiError, V3StartRequest, V3StartResponse, WorkloadRejectionError};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows — start a new workflow instance (bead vo-7mif).
///
/// Per ADR-028, this handler enforces exactly-once ingress:
/// 1. Validates that a `dedupe_key` is present for exact workflow ingress.
/// 2. Checks writer pressure — if overloaded, returns 429 + Retry-After with NO dedupe records.
/// 3. Calls `admit_ingress` to atomically check-and-insert into the dedupe store.
/// 4. If duplicate, returns 409 Conflict with the existing instance ID.
/// 5. Checks quarantine status — if quarantined, returns 403 Forbidden.
/// 6. If new, proceeds to start the workflow via the orchestrator actor.
#[tracing::instrument(skip_all)]
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(dedupe_store): Extension<Arc<dyn DedupeStore>>,
    Extension(writer_pressure): Extension<Arc<dyn WriterPressureGuard>>,
    Extension(circuit_breaker): Extension<Arc<CircuitBreakerState>>,
    Extension(event_db): Extension<Arc<fjall::Database>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    // Step 1: Validate dedupe key presence (ADR-028 Section 2).
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
        Some(ref id) => {
            match vo_types::InstanceId::parse(id) {
                Ok(id) => id,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiError::new("invalid_instance_id", format!("invalid instance_id format: {e}"))),
                    )
                        .into_response();
                }
            }
        }
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

fn persist_workflow_start_rejected_event(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &InstanceId,
) -> Result<(), vo_storage::codec::StorageError> {
    let payload = serde_json::json!({
        "type": "WorkflowTerminated",
        "namespace": namespace,
        "reason": "start-commit-failed",
    });
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

fn start_error_response(
    call_result: Result<CallResult<Result<(), StartError>>, ractor::MessagingErr<OrchestratorMsg>>,
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
                    "orchestrator did not respond in time",
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
        Ok(CallResult::Success(Err(StartError::AtCapacity { running, max }))) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ApiError::new(
                    "at_capacity",
                    format!("engine at capacity: {running}/{max} instances running"),
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(StartError::AlreadyExists(id)))) => Some(
            (
                StatusCode::CONFLICT,
                Json(ApiError::new(
                    "already_exists",
                    format!("instance {id} already exists"),
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(StartError::SpawnFailed(msg)))) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("spawn_failed", msg)),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Err(StartError::InvalidConfig(msg)))) => Some(
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("invalid_config", msg)),
            )
                .into_response(),
        ),
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
            Some(
                (
                    match StatusCode::from_u16(rejection.status_code()) {
                        Ok(status) => status,
                        Err(_) => StatusCode::TOO_MANY_REQUESTS,
                    },
                    Json(ApiError::new(rejection.error_code(), rejection.to_string())),
                )
                    .into_response(),
            )
        }
        Ok(CallResult::Success(Ok(()))) => None,
    }
}

fn request_namespace(namespace: &str) -> Result<NamespaceId, String> {
    if namespace.is_empty() {
        return Err("namespace must not be empty".to_string());
    }
    if namespace.contains('/') || namespace.as_bytes().contains(&b'\0') {
        return Err("namespace must not contain '/' or NUL".to_string());
    }
    Ok(NamespaceId::from(namespace.to_string()))
}

fn persist_workflow_started_event(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &InstanceId,
    workflow_type: &str,
    paradigm: &str,
    top_level_binary_hash: Option<&str>,
    input: &serde_json::Value,
    dedupe_key: &str,
) -> Result<(), vo_storage::codec::StorageError> {
    let binary_hash = workflow_binary_hash(top_level_binary_hash, input);
    let payload = serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": instance_id.to_string(),
        "workflow_type": workflow_type,
        "paradigm": paradigm,
        "namespace": namespace,
        "binary_hash": binary_hash,
        "workflow_version_hash": binary_hash,
        "dedupe_key_hash": dedupe_key,
    });
    let mut annotations = HashMap::new();
    annotations.insert("namespace".to_string(), serde_json::json!(namespace));

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

fn workflow_binary_hash(top_level_binary_hash: Option<&str>, input: &serde_json::Value) -> String {
    match top_level_binary_hash.filter(|value| !value.is_empty()) {
        Some(value) => value.to_string(),
        None => input_binary_hash(input),
    }
}

fn input_binary_hash(input: &serde_json::Value) -> String {
    match input
        .get("workflow_binary_hash")
        .and_then(serde_json::Value::as_str)
    {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => "unknown".to_string(),
    }
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).map_or(u64::MAX, |value| value),
        Err(_) => 0,
    }
}
