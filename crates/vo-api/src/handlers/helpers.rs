use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse, Json};
use ractor::rpc::CallResult;
use vo_actor::{InstancePhaseView, OrchestratorMsg, StartError, WorkflowParadigm};

use crate::types::{ApiError, V3StatusResponse};

/// Split a path `<namespace>/<instance_id>` into the two parts.
///
/// Returns `None` if the path has no `/` separator.
#[must_use]
pub fn split_path_id(path: &str) -> Option<(String, vo_types::InstanceId)> {
    let slash = path.find("/")?;
    let namespace = path[..slash].to_owned();
    let instance_id = vo_types::InstanceId::parse(&path[slash + 1..]).ok()?;
    Some((namespace, instance_id))
}

#[must_use]
pub fn parse_paradigm(s: &str) -> Option<WorkflowParadigm> {
    match s {
        "fsm" => Some(WorkflowParadigm::Fsm),
        "dag" => Some(WorkflowParadigm::Dag),
        "procedural" => Some(WorkflowParadigm::Procedural),
        _ => None,
    }
}

#[must_use]
pub fn paradigm_to_str(p: WorkflowParadigm) -> &'static str {
    match p {
        WorkflowParadigm::Fsm => "fsm",
        WorkflowParadigm::Dag => "dag",
        WorkflowParadigm::Procedural => "procedural",
    }
}

#[must_use]
pub fn phase_to_str(p: InstancePhaseView) -> &'static str {
    match p {
        InstancePhaseView::Replay => "replay",
        InstancePhaseView::Live => "live",
    }
}

/// Validates and returns the namespace string.
///
/// Returns `Ok(namespace)` if valid, `Err(message)` if invalid.
#[must_use]
pub fn request_namespace(namespace: &str) -> Result<String, String> {
    if namespace.is_empty() {
        Err("namespace cannot be empty".to_string())
    } else if namespace.len() > 256 {
        Err("namespace exceeds 256 characters".to_string())
    } else {
        Ok(namespace.to_string())
    }
}

/// Handle errors from workflow start operations.
#[must_use]
pub fn start_error_response(
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
        }))) => Some(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ApiError::new(
                    "budget_exhausted",
                    format!(
                        "budget exhausted for {:?}: requested {requested}, available {available}",
                        class
                    ),
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Ok(()))) => None,
    }
}

/// Persist a workflow started event to the event database.
#[must_use]
pub fn persist_workflow_started_event(
    _event_db: &Arc<fjall::Database>,
    _namespace: &str,
    _instance_id: &vo_types::InstanceId,
    _workflow_type: &str,
    _paradigm: &str,
    _workflow_binary_hash: Option<&str>,
    _input: &serde_json::Value,
    _dedupe_key: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Stub implementation - event persistence not yet implemented
    Ok(())
}

/// Persist a workflow start rejected event to the event database.
#[must_use]
pub fn persist_workflow_start_rejected_event(
    _event_db: &Arc<fjall::Database>,
    _namespace: &str,
    _instance_id: &vo_types::InstanceId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Stub implementation - event persistence not yet implemented
    Ok(())
}

/// Persist a lifecycle event to the event database.
#[must_use]
pub fn persist_lifecycle_event(
    _event_db: &Arc<fjall::Database>,
    _namespace: &str,
    _instance_id: &vo_types::InstanceId,
    _event: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Stub implementation - event persistence not yet implemented
    Ok(())
}

/// Handle errors from compensation operations.
#[must_use]
pub fn compensate_rejection(
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
        Ok(CallResult::Success(Err(_))) => Some(
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "compensation_failed",
                    "workflow compensation could not be initiated",
                )),
            )
                .into_response(),
        ),
        Ok(CallResult::Success(Ok(()))) => None,
    }
}

/// Look up terminal status from the event database.
#[must_use]
pub fn terminal_status_response(
    _event_db: &Arc<fjall::Database>,
    _namespace: &str,
    _instance_id: &vo_types::InstanceId,
) -> Option<vo_actor::InstanceSnapshot> {
    // Stub implementation - terminal status lookup not yet implemented
    None
}

/// Convert an instance snapshot to a V3 status response.
#[must_use]
pub fn status_response(snapshot: vo_actor::InstanceSnapshot) -> V3StatusResponse {
    V3StatusResponse {
        instance_id: snapshot.instance_id.to_string(),
        namespace: snapshot.namespace.to_string(),
        workflow_type: snapshot.workflow_type,
        paradigm: paradigm_to_str(snapshot.paradigm).to_owned(),
        phase: phase_to_str(snapshot.phase).to_owned(),
        events_applied: snapshot.events_applied,
    }
}

/// Response type for workflow status endpoint.
#[derive(Debug, serde::Serialize)]
pub struct WorkflowStatusResponseInner {
    pub instance_id: String,
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: String,
    pub phase: String,
    pub events_applied: u64,
    pub registration_status: Option<String>,
    pub is_quarantined: bool,
}

/// Convert an instance snapshot to a workflow status response.
#[must_use]
pub fn workflow_status_response(
    snapshot: vo_actor::InstanceSnapshot,
) -> WorkflowStatusResponseInner {
    WorkflowStatusResponseInner {
        instance_id: snapshot.instance_id.to_string(),
        namespace: snapshot.namespace.to_string(),
        workflow_type: snapshot.workflow_type,
        paradigm: paradigm_to_str(snapshot.paradigm).to_owned(),
        phase: phase_to_str(snapshot.phase).to_owned(),
        events_applied: snapshot.events_applied,
        registration_status: None,
        is_quarantined: false,
    }
}
