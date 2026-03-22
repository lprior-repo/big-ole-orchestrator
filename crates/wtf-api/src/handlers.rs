//! HTTP handlers for wtf-api v3 endpoints (beads wtf-7mif, wtf-016l, wtf-meua, wtf-k0ck).
//!
//! Each handler extracts the `OrchestratorMsg` actor ref from the axum `Extension`
//! and calls the MasterOrchestrator via ractor RPC (`actor_ref.call()`).
//!
//! # Namespace / ID routing
//! The `:id` path parameter is `<namespace>/<instance_id>` (URL-encoded slash or
//! a literal `/`). Handlers split on the first `/` to separate the two components.
//! Example: `/api/v1/workflows/payments/01ARZ3NDEKTSV4RRFFQ69G5FAV`.
//!
//! # Error mapping
//! - `OrchestratorMsg::StartWorkflow` → `StartError::AtCapacity` → 503
//! - `StartError::AlreadyExists` → 409
//! - `StartError::SpawnFailed` → 500
//! - `WtfError::InstanceNotFound` → 404
//! - Actor timeout → 503

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::time::Duration;

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::ActorRef;
use ulid::Ulid;
use wtf_actor::{messages::WorkflowParadigm, OrchestratorMsg, StartError};
use wtf_common::{InstanceId, NamespaceId};

use crate::types::{ApiError, DefinitionRequest, DefinitionResponse, DiagnosticDto, V3SignalRequest, V3StartRequest, V3StartResponse, V3StatusResponse};

/// Timeout for all actor RPC calls from HTTP handlers.
const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

// ── POST /api/v1/workflows ───────────────────────────────────────────────────

/// POST /api/v1/workflows — start a new workflow instance (bead wtf-7mif).
///
/// Request body: [`V3StartRequest`] (JSON).
/// Response: 201 with [`V3StartResponse`] or 4xx/5xx with [`ApiError`].
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    // Validate namespace.
    let namespace = match NamespaceId::try_new(&req.namespace) {
        Ok(ns) => ns,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_namespace",
                    format!("namespace contains illegal characters: {:?}", req.namespace),
                )),
            )
                .into_response();
        }
    };

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
    let instance_id = match req.instance_id {
        Some(ref id) => match InstanceId::try_new(id) {
            Ok(id) => id,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiError::new(
                        "invalid_instance_id",
                        "instance_id contains NATS-illegal characters",
                    )),
                )
                    .into_response();
            }
        },
        None => InstanceId::new(Ulid::new().to_string()),
    };

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

// ── GET /api/v1/workflows/:id ────────────────────────────────────────────────

/// GET /api/v1/workflows/:id — get instance status (bead wtf-016l).
///
/// Path: `:id` = `<namespace>/<instance_id>`.
/// Response: 200 with [`V3StatusResponse`] or 404 / 503.
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

// ── DELETE /api/v1/workflows/:id ─────────────────────────────────────────────

/// DELETE /api/v1/workflows/:id — terminate a running instance (bead wtf-016l).
///
/// Path: `:id` = `<namespace>/<instance_id>`.
/// Response: 204 on success, 404 not found, 503 actor unavailable.
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
        Ok(CallResult::Success(Err(wtf_actor::messages::TerminateError::NotFound(id)))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                "not_found",
                format!("instance {id} not found"),
            )),
        )
            .into_response(),
        Ok(CallResult::Success(Err(wtf_actor::messages::TerminateError::Failed(msg)))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("terminate_failed", msg)),
        )
            .into_response(),
        Ok(CallResult::Success(Ok(()))) => StatusCode::NO_CONTENT.into_response(),
    }
}

// ── GET /api/v1/workflows (list) ─────────────────────────────────────────────

/// GET /api/v1/workflows — list all active workflow instances.
///
/// Response: 200 with JSON array of [`V3StatusResponse`].
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

// ── POST /api/v1/workflows/:id/signals ───────────────────────────────────────

/// POST /api/v1/workflows/:id/signals — send a signal to a running instance (bead wtf-meua).
///
/// Path: `:id` = `<namespace>/<instance_id>`.
/// Request body: [`V3SignalRequest`] (JSON).
/// Response: 202 on success, 404 not found, 503 actor unavailable.
pub async fn send_signal(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
    Json(req): Json<V3SignalRequest>,
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

    // Serialize signal payload to bytes.
    let payload = match serde_json::to_vec(&req.payload) {
        Ok(v) => Bytes::from(v),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "invalid_payload",
                    format!("failed to encode payload: {e}"),
                )),
            )
                .into_response();
        }
    };

    let call_result = master
        .call(
            |tx| OrchestratorMsg::Signal {
                instance_id,
                signal_name: req.signal_name.clone(),
                payload,
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
        Ok(CallResult::Success(Err(e))) => (
            StatusCode::NOT_FOUND,
            Json(ApiError::new("signal_failed", e.to_string())),
        )
            .into_response(),
        Ok(CallResult::Success(Ok(()))) => StatusCode::ACCEPTED.into_response(),
    }
}

// ── GET /api/v1/workflows/:id/events ─────────────────────────────────────────

/// GET /api/v1/workflows/:id/events — fetch the JetStream event log (bead wtf-k0ck).
///
/// Returns events as a JSON array. Full SSE streaming is in bead wtf-wdxg.
/// This stub returns NOT_IMPLEMENTED — a NATS connection is needed (see wtf-k0ck).
pub async fn get_events(
    Extension(_master): Extension<ActorRef<OrchestratorMsg>>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // Full implementation requires JetStream access injected via Extension<Context>.
    // That's wired up in bead wtf-k0ck alongside the NATS connection setup.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new(
            "not_implemented",
            "event log streaming: see bead wtf-k0ck",
        )),
    )
        .into_response()
}

// ── POST /api/v1/definitions/:type ──────────────────────────────────────────

/// POST /api/v1/definitions/:type — ingest and lint a workflow definition (bead wtf-qyxl).
///
/// Accepts Rust source code for a procedural workflow, runs the wtf-linter,
/// and returns lint diagnostics. Returns 422 if violations are found.
///
/// Path: `:type` = definition type identifier (e.g., "workflow", "activity").
/// Request body: [`DefinitionRequest`] with `source` field containing Rust source.
/// Response: 200 with [`DefinitionResponse`] or 400 with [`ApiError`] on parse failure.
pub async fn ingest_definition(
    Path(_definition_type): Path<String>,
    Json(req): Json<DefinitionRequest>,
) -> impl IntoResponse {
    match wtf_linter::lint_workflow_code(&req.source) {
        Ok(diagnostics) => {
            let dtos: Vec<DiagnosticDto> = diagnostics
                .into_iter()
                .map(|d| DiagnosticDto {
                    code: d.code.as_str().to_owned(),
                    severity: d.severity.to_string(),
                    message: d.message,
                    suggestion: d.suggestion,
                    span: d.span,
                })
                .collect();
            let valid = dtos.iter().all(|d| d.severity != "error");
            (StatusCode::OK, Json(DefinitionResponse { valid, diagnostics: dtos }))
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiError::new("parse_error", e.to_string())),
        )
            .into_response(),
    }
}

// ── Pure helper functions ─────────────────────────────────────────────────────

/// Split a path `<namespace>/<instance_id>` into the two parts.
///
/// Returns `None` if the path has no `/` separator.
fn split_path_id(path: &str) -> Option<(String, InstanceId)> {
    let slash = path.find('/')?;
    let namespace = path[..slash].to_owned();
    let instance_id = InstanceId::new(path[slash + 1..].to_owned());
    Some((namespace, instance_id))
}

fn parse_paradigm(s: &str) -> Option<WorkflowParadigm> {
    match s {
        "fsm" => Some(WorkflowParadigm::Fsm),
        "dag" => Some(WorkflowParadigm::Dag),
        "procedural" => Some(WorkflowParadigm::Procedural),
        _ => None,
    }
}

fn paradigm_to_str(p: WorkflowParadigm) -> &'static str {
    match p {
        WorkflowParadigm::Fsm => "fsm",
        WorkflowParadigm::Dag => "dag",
        WorkflowParadigm::Procedural => "procedural",
    }
}

fn phase_to_str(p: wtf_actor::messages::InstancePhaseView) -> &'static str {
    match p {
        wtf_actor::messages::InstancePhaseView::Replay => "replay",
        wtf_actor::messages::InstancePhaseView::Live => "live",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_id_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, id) = result.map(|(n, i)| (n, i)).expect("some");
        assert_eq!(ns, "payments");
        assert_eq!(id.as_str(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn split_path_id_missing_slash_returns_none() {
        let result = split_path_id("no-slash-here");
        assert!(result.is_none());
    }

    #[test]
    fn split_path_id_multiple_slashes_splits_on_first() {
        let result = split_path_id("ns/id/extra");
        let (ns, id) = result.expect("some");
        assert_eq!(ns, "ns");
        assert_eq!(id.as_str(), "id/extra");
    }

    #[test]
    fn parse_paradigm_fsm() {
        assert_eq!(parse_paradigm("fsm"), Some(WorkflowParadigm::Fsm));
    }

    #[test]
    fn parse_paradigm_dag() {
        assert_eq!(parse_paradigm("dag"), Some(WorkflowParadigm::Dag));
    }

    #[test]
    fn parse_paradigm_procedural() {
        assert_eq!(
            parse_paradigm("procedural"),
            Some(WorkflowParadigm::Procedural)
        );
    }

    #[test]
    fn parse_paradigm_invalid_returns_none() {
        assert!(parse_paradigm("").is_none());
        assert!(parse_paradigm("FSM").is_none());
        assert!(parse_paradigm("state_machine").is_none());
    }

    #[test]
    fn paradigm_to_str_roundtrip() {
        for p in [
            WorkflowParadigm::Fsm,
            WorkflowParadigm::Dag,
            WorkflowParadigm::Procedural,
        ] {
            let s = paradigm_to_str(p);
            assert_eq!(parse_paradigm(s), Some(p));
        }
    }
}
