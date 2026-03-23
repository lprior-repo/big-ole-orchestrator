//! WorkflowInstance message types and phase management (ADR-016).

use crate::procedural::WorkflowFn;
use bytes::Bytes;
use ractor::RpcReplyPort;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wtf_common::{
    EventStore, InstanceId, NamespaceId, StateStore, TaskQueue, WorkflowDefinition, WorkflowEvent,
    WorkflowParadigm, WtfError,
};

/// Arguments passed to a WorkflowInstance actor when it is spawned.
#[derive(Debug, Clone)]
pub struct InstanceArguments {
    pub namespace: NamespaceId,
    pub instance_id: InstanceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub input: Bytes,
    pub engine_node_id: String,
    /// Abstract event store for writing events.
    pub event_store: Option<Arc<dyn EventStore>>,
    /// Abstract state store for heartbeats and metadata.
    pub state_store: Option<Arc<dyn StateStore>>,
    /// Abstract task queue for dispatching activities.
    pub task_queue: Option<Arc<dyn TaskQueue>>,
    /// Sled database handle for snapshot storage.
    pub snapshot_db: Option<sled::Db>,
    /// Procedural workflow function.
    pub procedural_workflow: Option<Arc<dyn WorkflowFn>>,
    /// FSM or DAG definition.
    pub workflow_definition: Option<WorkflowDefinition>,
}

/// Per-instance identity fields shared between fresh spawn and crash recovery.
pub struct InstanceSeed {
    pub namespace: NamespaceId,
    pub instance_id: InstanceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub input: Bytes,
}

/// Execution phase of a WorkflowInstance (ADR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstancePhase {
    Replay,
    Live,
}

/// Messages that can be sent to a `WorkflowInstance` actor.
#[derive(Debug)]
pub enum InstanceMsg {
    InjectEvent {
        seq: u64,
        event: WorkflowEvent,
    },
    InjectSignal {
        signal_name: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<(), WtfError>>,
    },
    Heartbeat,
    Cancel {
        reason: String,
        reply: RpcReplyPort<Result<(), WtfError>>,
    },
    GetStatus(RpcReplyPort<InstanceStatusSnapshot>),
    GetProceduralCheckpoint {
        operation_id: u32,
        reply: RpcReplyPort<Option<crate::procedural::Checkpoint>>,
    },
    ProceduralDispatch {
        activity_type: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<Bytes, WtfError>>,
    },
    ProceduralSleep {
        operation_id: u32,
        duration: std::time::Duration,
        reply: RpcReplyPort<Result<(), WtfError>>,
    },
    ProceduralNow {
        operation_id: u32,
        reply: RpcReplyPort<chrono::DateTime<chrono::Utc>>,
    },
    ProceduralRandom {
        operation_id: u32,
        reply: RpcReplyPort<u64>,
    },
    ProceduralWorkflowCompleted,
    ProceduralWorkflowFailed(String),
}

/// A point-in-time snapshot of an instance's status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStatusSnapshot {
    pub instance_id: InstanceId,
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstancePhaseView {
    Replay,
    Live,
}

impl From<InstancePhase> for InstancePhaseView {
    fn from(p: InstancePhase) -> Self {
        match p {
            InstancePhase::Replay => InstancePhaseView::Replay,
            InstancePhase::Live => InstancePhaseView::Live,
        }
    }
}
