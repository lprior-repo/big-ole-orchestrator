//! Core workflow HTTP handlers: start, get, terminate, list.

use super::workflow_mappers::{
    map_actor_error, map_start_result, map_status_result, map_terminate_result, validate_start_req,
};
use super::{split_path_id, ACTOR_CALL_TIMEOUT};
use crate::types::{ApiError, V3StartRequest, V3StatusResponse};
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use ractor::{rpc::CallResult, ActorRef};
use wtf_actor::OrchestratorMsg;

/// POST /api/v1/workflows — start a new workflow instance.
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    let (ns, id, p, input) = match validate_start_req(&req) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let res = master
        .call(
            |tx| OrchestratorMsg::StartWorkflow {
                namespace: ns,
                instance_id: id,
                workflow_type: req.workflow_type.clone(),
                paradigm: p,
                input,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;
    map_start_result(res, req.workflow_type)
}

/// GET /api/v1/workflows/:id — get instance status.
pub async fn get_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (_, inst_id) = match split_path_id(&id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_id", "bad id")),
            )
                .into_response()
        }
    };
    let res = master
        .call(
            |tx| OrchestratorMsg::GetStatus {
                instance_id: inst_id,
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;
    map_status_result(res, id)
}

/// DELETE /api/v1/workflows/:id — terminate a running instance.
pub async fn terminate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (_, inst_id) = match split_path_id(&id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_id", "bad id")),
            )
                .into_response()
        }
    };
    let res = master
        .call(
            |tx| OrchestratorMsg::Terminate {
                instance_id: inst_id,
                reason: "api-terminate".to_owned(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;
    map_terminate_result(res)
}

/// GET /api/v1/workflows — list all active workflow instances.
pub async fn list_workflows(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
) -> impl IntoResponse {
    let res = master
        .call(
            |tx| OrchestratorMsg::ListActive { reply: tx },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;
    match res {
        Ok(CallResult::Success(snapshots)) => (
            StatusCode::OK,
            Json(
                snapshots
                    .into_iter()
                    .map(V3StatusResponse::from)
                    .collect::<Vec<_>>(),
            ),
        )
            .into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{parse_paradigm, paradigm_to_str, split_path_id};
    use wtf_common::WorkflowParadigm;

    #[test]
    fn split_path_id_valid() {
        let result = split_path_id("payments/01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(result.is_some());
        let (ns, id) = result.expect("some");
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
