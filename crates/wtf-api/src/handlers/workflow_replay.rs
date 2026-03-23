//! Replay-to-sequence handler and supporting logic.

use super::{get_db, get_event_store, get_state_store, split_path_id, ACTOR_CALL_TIMEOUT};
use crate::types::ApiError;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use ractor::{rpc::CallResult, ActorRef};
use std::sync::Arc;
use wtf_actor::OrchestratorMsg;
use wtf_common::storage::ReplayBatch;
use wtf_common::{EventStore, InstanceId, NamespaceId, WorkflowParadigm};

pub async fn replay_to(
    Extension(master): Extension<ActorRef<OrchestratorMsg>>,
    Path((id, seq)): Path<(String, u64)>,
) -> impl IntoResponse {
    let (ns_str, inst_id) = match split_path_id(&id) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("invalid_id", "bad id")),
            )
                .into_response()
        }
    };
    let ns = NamespaceId::new(ns_str);
    let paradigm = match get_instance_paradigm(&master, &ns, &inst_id).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiError::new("not_found", e.to_string())),
            )
                .into_response()
        }
    };
    let store = match get_event_store(&master).await {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("Retry-After", "5")],
                Json(ApiError::new("no_store", "event store unavailable")),
            )
                .into_response()
        }
    };
    let db = match get_db(&master).await {
        Some(d) => d,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("Retry-After", "5")],
                Json(ApiError::new("no_db", "db unavailable")),
            )
                .into_response()
        }
    };
    match do_replay_to(store, db, ns, inst_id, seq, paradigm).await {
        Ok(state) => (StatusCode::OK, Json(state)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError::new("replay_error", e.to_string())),
        )
            .into_response(),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) async fn get_instance_paradigm(
    master: &ActorRef<OrchestratorMsg>,
    _ns: &NamespaceId,
    id: &InstanceId,
) -> Result<WorkflowParadigm, anyhow::Error> {
    let res = master
        .call(
            |tx| OrchestratorMsg::GetStatus {
                instance_id: id.clone(),
                reply: tx,
            },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await;
    if let Ok(CallResult::Success(Ok(Some(snap)))) = res {
        return Ok(snap.paradigm);
    }
    let store = get_state_store(master)
        .await
        .ok_or_else(|| anyhow::anyhow!("no_store"))?;
    let metadata = store
        .get_instance_metadata(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("instance metadata not found: {}", id))?;
    Ok(metadata.paradigm)
}

pub(crate) async fn do_replay_to(
    store: Arc<dyn EventStore>,
    db: sled::Db,
    ns: NamespaceId,
    id: InstanceId,
    target_seq: u64,
    paradigm: WorkflowParadigm,
) -> Result<wtf_actor::instance::lifecycle::ParadigmState, anyhow::Error> {
    let (mut p_state, from_seq) = load_snapshot(&db, &id, target_seq, paradigm).await?;
    let mut stream = store.open_replay_stream(&ns, &id, from_seq).await?;
    loop {
        match stream.next_event().await {
            Ok(ReplayBatch::Event(replayed)) => {
                if replayed.seq > target_seq {
                    break;
                }
                p_state = p_state
                    .apply_event(
                        &replayed.event,
                        replayed.seq,
                        wtf_actor::InstancePhase::Replay,
                    )
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
            Ok(ReplayBatch::TailReached) => break,
            Err(e) => return Err(anyhow::anyhow!(e.to_string())),
        }
    }
    Ok(p_state)
}

pub(crate) async fn load_snapshot(
    db: &sled::Db,
    id: &InstanceId,
    target_seq: u64,
    paradigm: WorkflowParadigm,
) -> Result<(wtf_actor::instance::lifecycle::ParadigmState, u64), anyhow::Error> {
    if let Ok(Some(snap)) = wtf_storage::read_snapshot(db, id) {
        if snap.seq <= target_seq {
            let state = wtf_actor::instance::lifecycle::deserialize_paradigm_state(
                paradigm,
                &snap.state_bytes,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            return Ok((state, snap.seq + 1));
        }
    }
    Ok((
        wtf_actor::instance::state::initialize_paradigm_state(&wtf_actor::InstanceArguments {
            namespace: NamespaceId::new(""),
            instance_id: id.clone(),
            workflow_type: "".to_owned(),
            paradigm,
            input: Bytes::new(),
            engine_node_id: "".to_owned(),
            snapshot_db: None,
            procedural_workflow: None,
            workflow_definition: None,
            event_store: None,
            state_store: None,
            task_queue: None,
        }),
        1,
    ))
}
