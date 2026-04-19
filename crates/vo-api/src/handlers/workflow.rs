use std::time::Duration;
use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use vo_actor::{CompensateError, InstancePhaseView, OrchestratorMsg, StartError, TerminateError};
use vo_common::{InstanceId, NamespaceId};
use vo_core::circuit_breaker::{unquarantine, CircuitBreakerConfig, CircuitBreakerState};
use vo_types::{BinaryHash, WorkflowName};

use crate::types::{ApiError, V3StartRequest, V3StartResponse, V3StatusResponse, WorkloadRejectionError};
use crate::handlers::helpers::{parse_paradigm, split_path_id, paradigm_to_str, phase_to_str};

/// Request body for unquarantine API.
#[derive(Debug, Deserialize)]
pub struct UnquarantineRequest {
    /// The operator performing the unquarantine.
    pub operator: String,
}

/// Response for unquarantine API.
#[derive(Debug, Serialize)]
pub struct UnquarantineResponse {
    pub workflow_name: String,
    pub previous_status: String,
    pub new_status: String,
    pub failures_cleared: usize,
}

/// Response for workflow status API (includes quarantine info).
#[derive(Debug, Serialize)]
pub struct WorkflowStatusResponse {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: String,
    pub phase: String,
    pub events_applied: u64,
    pub registration_status: Option<String>,
    pub is_quarantined: bool,
}

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// POST /api/v1/workflows — start a new workflow instance (bead vo-7mif).
#[tracing::instrument(skip_all)]
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    // Validate dedupe key is present for exact workflow ingress (ADR-028).
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

    // Validate namespace.
    let namespace = NamespaceId::from(req.namespace);

    // Parse paradigm.
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

    // Generate or validate instance_id.
    let instance_id_str = match req.instance_id {
        Some(ref id) => id.clone(),
        None => Ulid::new().to_string(),
    };
    let instance_id = vo_types::InstanceId::parse(&instance_id_str).expect("generated ULID should be valid");

    // Serialize input to msgpack bytes.
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

/// GET /api/v1/workflows/:id — get instance status (bead vo-016l).
#[tracing::instrument(skip_all)]
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

/// GET /api/v1/workflows — list all active workflow instances.
#[tracing::instrument(skip_all)]
pub async fn list_workflows(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
) -> impl IntoResponse {
    let call_result = master
        .call(
            |tx| OrchestratorMsg::ListActive { reply: tx },
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
        Ok(CallResult::Success(snapshots)) => {
            let views: Vec<V3StatusResponse> = snapshots
                .into_iter()
                .map(|s| V3StatusResponse {
                    instance_id: s.instance_id.to_string(),
                    namespace: s.namespace.to_string(),
                    workflow_type: s.workflow_type,
                    paradigm: paradigm_to_str(s.paradigm).to_owned(),
                    phase: phase_to_str(s.phase).to_owned(),
                    events_applied: s.events_applied,
                })
                .collect();
            (StatusCode::OK, Json(views)).into_response()
        }
    }
}

/// POST /api/v1/workflows/:id/unquarantine — manually unquarantine a workflow (ADR-026).
#[tracing::instrument(skip_all)]
pub async fn unquarantine_workflow(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(req): Json<UnquarantineRequest>,
) -> impl IntoResponse {
    // Parse workflow name from path
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

    // Parse workflow name
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

    // Get circuit breaker state from extension (would be injected in production)
    // For now, return not implemented - this requires circuit breaker state injection
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new(
            "not_implemented",
            "circuit breaker state injection required (see bead ve-jfj5)",
        )),
    )
        .into_response()
}

/// GET /api/v1/workflows/:id/status — get workflow status including quarantine info (ADR-026).
#[tracing::instrument(skip_all)]
pub async fn get_workflow_status(
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
        Ok(CallResult::Success(Some(snapshot))) => {
            // TODO: Add quarantine status from circuit breaker
            let status_response = WorkflowStatusResponse {
                instance_id: snapshot.instance_id.to_string(),
                namespace: snapshot.namespace.to_string(),
                workflow_type: snapshot.workflow_type,
                paradigm: paradigm_to_str(snapshot.paradigm).to_owned(),
                phase: phase_to_str(snapshot.phase).to_owned(),
                events_applied: snapshot.events_applied,
                registration_status: None,
                is_quarantined: false,
            };
            (StatusCode::OK, Json(status_response)).into_response()
        }
    }
}
