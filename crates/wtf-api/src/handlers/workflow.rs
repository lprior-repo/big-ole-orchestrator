use axum::{extract::{Extension, Path}, http::StatusCode, response::{IntoResponse, Response}, Json};
use bytes::Bytes;
use ractor::rpc::CallResult;
use ractor::{ActorRef, MessagingErr};
use wtf_actor::{OrchestratorMsg, StartError, InstanceStatusSnapshot, TerminateError};
use std::sync::Arc;
use wtf_common::{InstanceId, NamespaceId, WorkflowParadigm, EventStore};
use wtf_common::storage::ReplayBatch;
use crate::types::{ApiError, V3StartRequest, V3StartResponse, V3StatusResponse};
use super::{ACTOR_CALL_TIMEOUT, split_path_id, parse_paradigm, paradigm_to_str, phase_to_str, get_event_store, get_state_store, get_db};

/// POST /api/v1/workflows — start a new workflow instance.
pub async fn start_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Json(req): Json<V3StartRequest>,
) -> impl IntoResponse {
    let (ns, id, p, input) = match validate_start_req(&req) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let res = master.call(|tx| OrchestratorMsg::StartWorkflow {
        namespace: ns, instance_id: id, workflow_type: req.workflow_type.clone(), paradigm: p, input, reply: tx,
    }, Some(ACTOR_CALL_TIMEOUT)).await;
    map_start_result(res, req.workflow_type)
}

/// GET /api/v1/workflows/:id — get instance status.
pub async fn get_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (_, inst_id) = match split_path_id(&id) {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_id", "bad id"))).into_response(),
    };
    let res = master.call(|tx| OrchestratorMsg::GetStatus { instance_id: inst_id, reply: tx }, Some(ACTOR_CALL_TIMEOUT)).await;
    map_status_result(res, id)
}

/// DELETE /api/v1/workflows/:id — terminate a running instance.
pub async fn terminate_workflow(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let (_, inst_id) = match split_path_id(&id) {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_id", "bad id"))).into_response(),
    };
    let res = master.call(|tx| OrchestratorMsg::Terminate {
        instance_id: inst_id, reason: "api-terminate".to_owned(), reply: tx,
    }, Some(ACTOR_CALL_TIMEOUT)).await;
    map_terminate_result(res)
}

/// GET /api/v1/workflows — list all active workflow instances.
pub async fn list_workflows(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
) -> impl IntoResponse {
    let res = master.call(|tx| OrchestratorMsg::ListActive { reply: tx }, Some(ACTOR_CALL_TIMEOUT)).await;
    match res {
        Ok(CallResult::Success(snapshots)) => (StatusCode::OK, Json(snapshots.into_iter().map(V3StatusResponse::from).collect::<Vec<_>>())).into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

/// GET /api/v1/instances/:id/replay-to/:seq — replay instance to a specific sequence.
pub async fn replay_to(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path((id, seq)): Path<(String, u64)>,
) -> impl IntoResponse {
    let (ns_str, inst_id) = match split_path_id(&id) {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_id", "bad id"))).into_response(),
    };
    let ns = NamespaceId::new(ns_str);
    let paradigm = match get_instance_paradigm(&master, &ns, &inst_id).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::NOT_FOUND, Json(ApiError::new("not_found", e.to_string()))).into_response(),
    };
    let store = match get_event_store(&master).await { Some(s) => s, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError::new("no_store", "event store unavailable"))).into_response() };
    let db = match get_db(&master).await { Some(d) => d, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError::new("no_db", "db unavailable"))).into_response() };
    match do_replay_to(store, db, ns, inst_id, seq, paradigm).await {
        Ok(state) => (StatusCode::OK, Json(state)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new("replay_error", e.to_string()))).into_response(),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn get_instance_paradigm(master: &ActorRef<OrchestratorMsg>, _ns: &NamespaceId, id: &InstanceId) -> Result<WorkflowParadigm, anyhow::Error> {
    let res = master.call(|tx| OrchestratorMsg::GetStatus { instance_id: id.clone(), reply: tx }, Some(ACTOR_CALL_TIMEOUT)).await;
    if let Ok(CallResult::Success(Some(snap))) = res {
        return Ok(snap.paradigm);
    }
    let store = get_state_store(master).await.ok_or_else(|| anyhow::anyhow!("no_store"))?;
    let metadata = store.get_instance_metadata(id).await?.ok_or_else(|| anyhow::anyhow!("instance metadata not found: {}", id))?;
    Ok(metadata.paradigm)
}

fn validate_start_req(req: &V3StartRequest) -> Result<(NamespaceId, InstanceId, WorkflowParadigm, Bytes), (StatusCode, Json<ApiError>)> {
    let ns = NamespaceId::try_new(&req.namespace).map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_namespace", "bad namespace"))))?;
    let p = parse_paradigm(&req.paradigm).ok_or_else(|| (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_paradigm", "bad paradigm"))))?;
    let id = req.instance_id.as_ref().map_or(Ok(InstanceId::new(ulid::Ulid::new().to_string())), |s| InstanceId::try_new(s)).map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_instance_id", "bad instance_id"))))?;
    let input = serde_json::to_vec(&req.input).map_err(|e| (StatusCode::BAD_REQUEST, Json(ApiError::new("invalid_input", e.to_string()))))?;
    Ok((ns, id, p, Bytes::from(input)))
}

fn map_start_result(res: Result<CallResult<Result<InstanceId, StartError>>, MessagingErr<OrchestratorMsg>>, wf_type: String) -> Response {
    match res {
        Ok(CallResult::Success(Ok(id))) => (StatusCode::CREATED, Json(V3StartResponse { instance_id: id.to_string(), namespace: "".to_owned(), workflow_type: wf_type })).into_response(),
        Ok(CallResult::Success(Err(e))) => map_start_error(e).into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

fn map_start_error(err: StartError) -> impl IntoResponse {
    match err {
        StartError::AtCapacity { running, max } => (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError::new("at_capacity", format!("{running}/{max}")))),
        StartError::AlreadyExists(id) => (StatusCode::CONFLICT, Json(ApiError::new("already_exists", id.to_string()))),
        StartError::SpawnFailed(msg) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new("spawn_failed", msg))),
    }
}

fn map_status_result(res: Result<CallResult<Option<InstanceStatusSnapshot>>, MessagingErr<OrchestratorMsg>>, id: String) -> Response {
    match res {
        Ok(CallResult::Success(Some(s))) => (StatusCode::OK, Json(V3StatusResponse::from(s))).into_response(),
        Ok(CallResult::Success(None)) => (StatusCode::NOT_FOUND, Json(ApiError::new("not_found", id))).into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

fn map_terminate_result(res: Result<CallResult<Result<(), TerminateError>>, MessagingErr<OrchestratorMsg>>) -> Response {
    match res {
        Ok(CallResult::Success(Ok(()))) => StatusCode::NO_CONTENT.into_response(),
        Ok(CallResult::Success(Err(TerminateError::NotFound(id)))) => (StatusCode::NOT_FOUND, Json(ApiError::new("not_found", id.to_string()))).into_response(),
        _ => map_actor_error(res).into_response(),
    }
}

fn map_actor_error<T>(res: Result<CallResult<T>, MessagingErr<OrchestratorMsg>>) -> impl IntoResponse {
    match res {
        Ok(CallResult::Timeout) => (StatusCode::SERVICE_UNAVAILABLE, Json(ApiError::new("actor_timeout", "timeout"))),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new("actor_error", "actor failed"))),
    }
}

impl From<InstanceStatusSnapshot> for V3StatusResponse {
    fn from(s: InstanceStatusSnapshot) -> Self {
        Self {
            instance_id: s.instance_id.to_string(), namespace: s.namespace.to_string(), workflow_type: s.workflow_type,
            paradigm: paradigm_to_str(s.paradigm).to_owned(), phase: phase_to_str(s.phase).to_owned(), events_applied: s.events_applied,
        }
    }
}

async fn do_replay_to(store: Arc<dyn EventStore>, db: sled::Db, ns: NamespaceId, id: InstanceId, target_seq: u64, paradigm: WorkflowParadigm) -> Result<wtf_actor::instance::lifecycle::ParadigmState, anyhow::Error> {
    let (mut p_state, from_seq) = load_snapshot(&db, &id, target_seq, paradigm).await?;
    let mut stream = store.open_replay_stream(&ns, &id, from_seq).await?;
    loop {
        match stream.next_event().await {
            Ok(ReplayBatch::Event(replayed)) => {
                if replayed.seq > target_seq { break; }
                p_state = p_state.apply_event(&replayed.event, replayed.seq, wtf_actor::InstancePhase::Replay).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
            Ok(ReplayBatch::TailReached) => break,
            Err(e) => return Err(anyhow::anyhow!(e.to_string())),
        }
    }
    Ok(p_state)
}

async fn load_snapshot(db: &sled::Db, id: &InstanceId, target_seq: u64, paradigm: WorkflowParadigm) -> Result<(wtf_actor::instance::lifecycle::ParadigmState, u64), anyhow::Error> {
    if let Ok(Some(snap)) = wtf_storage::read_snapshot(db, id) {
        if snap.seq <= target_seq {
            let state = wtf_actor::instance::lifecycle::deserialize_paradigm_state(paradigm, &snap.state_bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            return Ok((state, snap.seq + 1));
        }
    }
    Ok((wtf_actor::instance::state::initialize_paradigm_state(&wtf_actor::InstanceArguments {
        namespace: NamespaceId::new(""), instance_id: id.clone(), workflow_type: "".to_owned(), paradigm,
        input: Bytes::new(), engine_node_id: "".to_owned(), snapshot_db: None,
        procedural_workflow: None, workflow_definition: None,
        event_store: None, state_store: None, task_queue: None,
    }), 1))
}

#[cfg(test)]
mod tests {
    use super::*;

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
