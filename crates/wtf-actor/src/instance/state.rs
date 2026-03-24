//! In-memory state of a running `WorkflowInstance`.

use super::lifecycle::ParadigmState;
use crate::messages::{InstanceArguments, InstancePhase};
use bytes::Bytes;
use ractor::RpcReplyPort;
use std::collections::HashMap;
use wtf_common::WorkflowParadigm;
use wtf_common::{ActivityId, WorkflowEvent, WtfError};

/// In-memory state of a running `WorkflowInstance`.
#[derive(Debug)]
pub struct InstanceState {
    /// Immutable spawn arguments.
    pub args: InstanceArguments,
    /// Current execution phase (Replay or Live).
    pub phase: InstancePhase,
    /// Total events applied (monotonically increasing).
    pub total_events_applied: u64,
    /// Events since last snapshot (reset at `SNAPSHOT_INTERVAL`).
    pub events_since_snapshot: u32,
    /// Current state of the execution paradigm.
    pub paradigm_state: ParadigmState,

    /// Outbox for saga compensation — events that failed to publish go here.
    /// Drained on next successful publish or during recovery.
    pub outbox: Vec<WorkflowEvent>,

    /// Pending RPC calls from procedural workflows waiting for activity results.
    /// Keyed by `ActivityId`. Not persisted in snapshots.
    pub pending_activity_calls: HashMap<ActivityId, RpcReplyPort<Result<Bytes, WtfError>>>,

    /// Pending RPC calls from procedural workflows waiting for timers.
    /// Keyed by `TimerId`. Not persisted in snapshots.
    pub pending_timer_calls: HashMap<wtf_common::TimerId, RpcReplyPort<Result<(), WtfError>>>,

    /// Pending RPC calls from procedural workflows waiting for signals.
    /// Keyed by signal name (String). Not persisted in snapshots.
    pub pending_signal_calls: HashMap<String, RpcReplyPort<Result<Bytes, WtfError>>>,

    /// Join handle for the procedural workflow task.
    pub procedural_task: Option<tokio::task::JoinHandle<()>>,

    /// Join handle for the live subscription task.
    pub live_subscription_task: Option<tokio::task::JoinHandle<()>>,
}

impl InstanceState {
    /// Create the initial state for a new workflow instance.
    #[must_use]
    pub fn initial(args: InstanceArguments) -> Self {
        let paradigm_state = initialize_paradigm_state(&args);
        Self {
            args,
            phase: InstancePhase::Replay,
            total_events_applied: 0,
            events_since_snapshot: 0,
            paradigm_state,
            outbox: Vec::new(),
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            pending_signal_calls: HashMap::new(),
            procedural_task: None,
            live_subscription_task: None,
        }
    }
}

pub fn initialize_paradigm_state(args: &InstanceArguments) -> ParadigmState {
    match args.paradigm {
        WorkflowParadigm::Fsm => ParadigmState::Fsm(crate::fsm::FsmActorState::new("Initial")),
        WorkflowParadigm::Dag => {
            let nodes = args
                .workflow_definition
                .as_ref()
                .and_then(|def| crate::dag::parse::parse_dag_graph(&def.graph_raw).ok())
                .unwrap_or_default();
            ParadigmState::Dag(crate::dag::DagActorState::new(nodes))
        }
        WorkflowParadigm::Procedural => {
            ParadigmState::Procedural(crate::procedural::ProceduralActorState::new())
        }
    }
}
