use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
};
use ractor::rpc::CallResult;
use ractor::ActorRef;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use vo_actor::{InstancePhaseView, InstanceSnapshot, OrchestratorMsg};
use vo_storage::event_log::replay_events_in_namespace;
use vo_types::InstanceId;

use crate::handlers::helpers::{paradigm_to_str, phase_to_str, split_path_id};
use crate::types::{ApiError, V3StatusResponse};

const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

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

#[cfg(test)]
mod tests {
    use crate::handlers::helpers::split_path_id;

    #[test]
    fn test_split_path_id_valid_format() {
        let result = split_path_id("test-namespace/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, id) = result.unwrap();
        assert_eq!(ns.as_str(), "test-namespace");
        assert_eq!(id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn test_split_path_id_invalid_format() {
        let result = split_path_id("invalid_id_format");
        assert!(result.is_none());
    }

    #[test]
    fn test_split_path_id_empty_path() {
        let result = split_path_id("");
        assert!(result.is_none());
    }
}

/// GET /api/v1/workflows/:id — get instance status (bead vo-016l).
#[tracing::instrument(skip_all)]
pub async fn get_workflow(
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

    let query_namespace = namespace.clone();
    let event_instance_id = instance_id.clone();
    let call_result = master
        .call(
            |tx| OrchestratorMsg::GetStatus {
                namespace: query_namespace,
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
        Ok(CallResult::Success(None)) => {
            terminal_status_response(&event_db, &namespace, &event_instance_id).map_or_else(
                || {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ApiError::new(
                            "not_found",
                            format!("instance {namespace}/{event_instance_id} not found"),
                        )),
                    )
                        .into_response()
                },
                |snapshot| (StatusCode::OK, Json(status_response(snapshot))).into_response(),
            )
        }
        Ok(CallResult::Success(Some(snapshot))) => {
            (StatusCode::OK, Json(status_response(snapshot))).into_response()
        }
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

/// GET /api/v1/workflows/:id/status — get workflow status including quarantine info (ADR-026).
#[tracing::instrument(skip_all)]
pub async fn get_workflow_status(
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

    let query_namespace = namespace.clone();
    let event_instance_id = instance_id.clone();
    let call_result = master
        .call(
            |tx| OrchestratorMsg::GetStatus {
                namespace: query_namespace,
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
        Ok(CallResult::Success(None)) => {
            terminal_status_response(&event_db, &namespace, &event_instance_id).map_or_else(
                || {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ApiError::new(
                            "not_found",
                            format!("instance {namespace}/{event_instance_id} not found"),
                        )),
                    )
                        .into_response()
                },
                |snapshot| {
                    (StatusCode::OK, Json(workflow_status_response(snapshot))).into_response()
                },
            )
        }
        Ok(CallResult::Success(Some(snapshot))) => {
            (StatusCode::OK, Json(workflow_status_response(snapshot))).into_response()
        }
    }
}

fn status_response(snapshot: InstanceSnapshot) -> V3StatusResponse {
    V3StatusResponse {
        instance_id: snapshot.instance_id.to_string(),
        namespace: snapshot.namespace.to_string(),
        workflow_type: snapshot.workflow_type,
        paradigm: paradigm_to_str(snapshot.paradigm).to_owned(),
        phase: phase_to_str(snapshot.phase).to_owned(),
        events_applied: snapshot.events_applied,
    }
}

fn workflow_status_response(snapshot: InstanceSnapshot) -> WorkflowStatusResponse {
    WorkflowStatusResponse {
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

fn terminal_status_response(
    db: &fjall::Database,
    namespace: &str,
    instance_id: &InstanceId,
) -> Option<InstanceSnapshot> {
    replay_events_in_namespace(db, namespace, instance_id)
        .filter_map(Result::ok)
        .fold(None, terminal_snapshot_step)
        .filter(|snapshot| snapshot.phase == InstancePhaseView::Terminated)
}

fn terminal_snapshot_step(
    current: Option<InstanceSnapshot>,
    envelope: vo_types::EventEnvelope,
) -> Option<InstanceSnapshot> {
    match envelope
        .payload
        .get("type")
        .and_then(serde_json::Value::as_str)
    {
        Some("WorkflowStarted") => started_snapshot(envelope),
        Some("WorkflowTerminated") => current.map(|snapshot| InstanceSnapshot {
            phase: InstancePhaseView::Terminated,
            events_applied: envelope.sequence,
            ..snapshot
        }),
        Some("SignalAccepted") | Some("WorkflowCompensationInitiated") => {
            current.map(|snapshot| InstanceSnapshot {
                events_applied: envelope.sequence,
                ..snapshot
            })
        }
        _ => current,
    }
}

fn started_snapshot(envelope: vo_types::EventEnvelope) -> Option<InstanceSnapshot> {
    let instance_id = InstanceId::parse(&envelope.instance_id).ok()?;
    let workflow_type = envelope
        .payload
        .get("workflow_type")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| "unknown".to_string(), ToString::to_string);
    let paradigm = envelope
        .payload
        .get("paradigm")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_paradigm_from_event)
        .map_or(vo_actor::WorkflowParadigm::Procedural, |value| value);
    payload_namespace(&envelope).map(|namespace| InstanceSnapshot {
        instance_id,
        namespace,
        workflow_type,
        paradigm,
        phase: InstancePhaseView::Live,
        events_applied: envelope.sequence,
    })
}

fn payload_namespace(envelope: &vo_types::EventEnvelope) -> Option<String> {
    envelope
        .payload
        .get("namespace")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            envelope
                .metadata
                .annotations
                .get("namespace")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
}

fn parse_paradigm_from_event(value: &str) -> Option<vo_actor::WorkflowParadigm> {
    match value {
        "fsm" => Some(vo_actor::WorkflowParadigm::Fsm),
        "dag" => Some(vo_actor::WorkflowParadigm::Dag),
        "procedural" => Some(vo_actor::WorkflowParadigm::Procedural),
        _ => None,
    }
}
