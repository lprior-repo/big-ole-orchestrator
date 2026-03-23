//! HTTP handlers for wtf-api v3 endpoints.

pub mod definitions;
pub mod events;
pub mod journal;
pub mod signal;
pub mod validate;
pub mod workflow;
pub mod workflow_mappers;
pub mod workflow_replay;

pub use definitions::*;
pub use events::*;
pub use journal::*;
pub use signal::*;
pub use validate::*;
pub use workflow::*;
pub use workflow_replay::replay_to;

use ractor::{rpc::CallResult, ActorRef};
use std::time::Duration;
use wtf_actor::OrchestratorMsg;
use wtf_common::{InstanceId, WorkflowParadigm};

/// Timeout for all actor RPC calls from HTTP handlers.
pub const ACTOR_CALL_TIMEOUT: Duration = Duration::from_secs(5);

use std::sync::Arc;
use wtf_common::{EventStore, StateStore};

pub async fn get_event_store(master: &ActorRef<OrchestratorMsg>) -> Option<Arc<dyn EventStore>> {
    master
        .call(
            |tx| OrchestratorMsg::GetEventStore { reply: tx },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await
        .ok()
        .and_then(|r| match r {
            CallResult::Success(s) => s,
            _ => None,
        })
}

pub async fn get_state_store(master: &ActorRef<OrchestratorMsg>) -> Option<Arc<dyn StateStore>> {
    master
        .call(
            |tx| OrchestratorMsg::GetStateStore { reply: tx },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await
        .ok()
        .and_then(|r| match r {
            CallResult::Success(s) => s,
            _ => None,
        })
}

pub async fn get_db(master: &ActorRef<OrchestratorMsg>) -> Option<sled::Db> {
    master
        .call(
            |tx| OrchestratorMsg::GetSnapshotDb { reply: tx },
            Some(ACTOR_CALL_TIMEOUT),
        )
        .await
        .ok()
        .and_then(|r| match r {
            CallResult::Success(s) => s,
            _ => None,
        })
}

/// Split a path `<namespace>/<instance_id>` into the two parts.
pub(crate) fn split_path_id(path: &str) -> Option<(String, InstanceId)> {
    let slash = path.find('/')?;
    let (ns, id) = path.split_at(slash);
    Some((ns.to_owned(), InstanceId::new(id[1..].to_owned())))
}

pub(crate) fn parse_paradigm(s: &str) -> Option<WorkflowParadigm> {
    match s {
        "fsm" => Some(WorkflowParadigm::Fsm),
        "dag" => Some(WorkflowParadigm::Dag),
        "procedural" => Some(WorkflowParadigm::Procedural),
        _ => None,
    }
}

pub(crate) fn paradigm_to_str(p: WorkflowParadigm) -> &'static str {
    match p {
        WorkflowParadigm::Fsm => "fsm",
        WorkflowParadigm::Dag => "dag",
        WorkflowParadigm::Procedural => "procedural",
    }
}

pub(crate) fn phase_to_str(p: wtf_actor::InstancePhaseView) -> &'static str {
    match p {
        wtf_actor::InstancePhaseView::Replay => "replay",
        wtf_actor::InstancePhaseView::Live => "live",
    }
}
