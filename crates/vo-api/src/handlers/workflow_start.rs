use axum::{
    extract::{Extension, Json},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use std::sync::Arc;
use std::time::Duration;
use ulid::Ulid;
use vo_actor::{OrchestratorMsg, StartError};
use vo_common::NamespaceId;
use vo_core::admission::{PressureGuardResult, WriterPressureGuard};
use vo_storage::dedupe_partition::DedupeStore;

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
/// 5. If new, proceeds to start the workflow via the orchestrator actor.
#[tracing::instrument(skip_all)]
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Extension(dedupe_store): Extension<Arc<dyn DedupeStore>>,
    Extension(writer_pressure): Extension<Arc<dyn WriterPressureGuard>>,
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
    let instance_id =
        vo_types::InstanceId::parse(&instance_id_str).expect("generated ULID should be valid");

    // Step 3: Check writer pressure BEFORE dedupe admission (ADR-006, ADR-015).
    // When DbWriter mailbox is at 80% capacity, shed ingress with 429 + Retry-After.
    // MUST happen before dedupe admission so no records are written when shed.
    match writer_pressure.check() {
        PressureGuardResult::Admitted => {}
        PressureGuardResult::Shed {
            retry_after_secs,
            reason,
        } => {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::RETRY_AFTER,
                retry_after_secs.to_string().parse().expect("valid header value"),
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                headers,
                Json(ApiError::new("writer_pressure_shed", reason)),
            )
                .into_response();
        }
    }

    // Step 4: Atomic admission check against dedupe store (ADR-028 Section 3).
    let admission = match admit_ingress(
        dedupe_store.as_ref(),
        &dedupe_key,
        &instance_id,
        DEFAULT_DEDUPE_TTL_MS,
    ) {
        Ok(a) => a,
        Err(IngressAdmissionError::Storage { reason }) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new("dedupe_storage_error", reason)),
            )
                .into_response();
        }
        Err(IngressAdmissionError::InvalidDedupeKey { reason }) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_dedupe_key", reason)),
            )
                .into_response();
        }
    };

    // Step 5: If duplicate, return 409 Conflict with existing instance (ADR-028).
    if let IngressAdmission::Duplicate {
        existing_instance_id,
    } = admission
    {
        return (
            StatusCode::CONFLICT,
            Json(ApiError::new(
                "duplicate_ingress",
                format!(
                    "dedupe_key '{dedupe_key}' already admitted as instance {existing_instance_id}"
                ),
            )),
        )
            .into_response();
    }

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

    // Step 5: Proceed to start workflow via actor (ADR-028 atomic write).
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
                StatusCode::from_u16(rejection.status_code())
                    .unwrap_or(StatusCode::TOO_MANY_REQUESTS),
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
