//! Actor message types for wtf-engine v3 — MasterOrchestrator and WorkflowInstance (ADR-006).
//!
//! Two-level hierarchy:
//! - `OrchestratorMsg`: messages to the MasterOrchestrator root supervisor.
//! - `InstanceMsg`: messages to individual WorkflowInstance actors.
//!
//! WorkflowInstance lifecycle has two phases (ADR-016):
//! - `InstancePhase::Replay`: replaying the JetStream event log — no I/O effects.
//! - `InstancePhase::Live`: processing new events — effects execute normally.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::procedural::WorkflowFn;
use bytes::Bytes;
use ractor::RpcReplyPort;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wtf_common::{InstanceId, NamespaceId, WtfError};
use wtf_storage::NatsClient;

// ============================================================================
// Paradigm selection
// ============================================================================

/// The three execution paradigms supported by wtf-engine (ADR-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowParadigm {
    /// State Machine — transitions recorded as `TransitionApplied` events.
    Fsm,
    /// Directed Acyclic Graph — activities dispatched by dependency order.
    Dag,
    /// Arbitrary async Rust code with checkpoint-based determinism.
    Procedural,
}

/// Serialized form of a workflow graph for FSM or DAG paradigms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowDefinition {
    /// Paradigm this definition implements.
    pub paradigm: WorkflowParadigm,
    /// JSON or YAML representation of the graph (nodes/edges or transitions).
    pub graph_raw: String,
    /// Human-readable description (optional).
    pub description: Option<String>,
}

// ============================================================================
// WorkflowInstance spawn arguments
// ============================================================================

/// Arguments passed to a WorkflowInstance actor when it is spawned.
///
/// These are immutable for the lifetime of the actor — set once at spawn time,
/// never mutated during replay or live execution.
#[derive(Debug, Clone)]
pub struct InstanceArguments {
    /// Namespace the instance belongs to (e.g. `"payments"`).
    pub namespace: NamespaceId,
    /// Stable ID of this workflow instance.
    pub instance_id: InstanceId,
    /// Workflow type name (selects the execution function).
    pub workflow_type: String,
    /// Which paradigm drives execution for this instance.
    pub paradigm: WorkflowParadigm,
    /// Initial input bytes (passed to workflow code on first start).
    pub input: Bytes,
    /// Unique identifier for this engine node (written to the heartbeat KV).
    pub engine_node_id: String,
    /// NATS client for JetStream and KV operations (optional for tests).
    pub nats: Option<NatsClient>,
    /// Sled database handle for snapshot storage (optional for tests).
    pub snapshot_db: Option<sled::Db>,
    /// Procedural workflow function (if paradigm is Procedural).
    pub procedural_workflow: Option<Arc<dyn WorkflowFn>>,
    /// FSM or DAG definition (if paradigm is Fsm or Dag).
    pub workflow_definition: Option<WorkflowDefinition>,
}

// ============================================================================
// Two-phase execution state
// ============================================================================

/// Execution phase of a WorkflowInstance (ADR-016).
///
/// The phase is stored in the actor's state and checked on every event:
/// - During `Replay`: apply the event but skip side effects.
/// - During `Live`: apply the event and execute side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstancePhase {
    /// Replaying the JetStream event log. Side effects are suppressed.
    ///
    /// Ends when the replay consumer reaches the stream tail (timeout).
    Replay,
    /// Processing new events in real time. Side effects execute normally.
    Live,
}

// ============================================================================
// WorkflowInstance messages
// ============================================================================

/// Messages that can be sent to a `WorkflowInstance` actor.
#[derive(Debug)]
pub enum InstanceMsg {
    /// A new `WorkflowEvent` arrived on the live subscription.
    ///
    /// `seq` is the JetStream sequence number (used for idempotency checks).
    InjectEvent {
        seq: u64,
        event: wtf_common::WorkflowEvent,
    },

    /// A signal was delivered to this instance.
    ///
    /// The actor records a `SignalReceived` event and dispatches to
    /// workflow code if in Live Phase.
    InjectSignal {
        signal_name: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<(), WtfError>>,
    },

    /// Periodic heartbeat tick — actor writes to `wtf-heartbeats` KV.
    ///
    /// Sent by the actor's own timer. If the actor stops writing, the
    /// 10s TTL expires and the heartbeat watcher triggers recovery.
    Heartbeat,

    /// Graceful cancellation request from the MasterOrchestrator.
    Cancel {
        reason: String,
        reply: RpcReplyPort<Result<(), WtfError>>,
    },

    /// Query: get current status (for health and API responses).
    GetStatus(RpcReplyPort<InstanceStatusSnapshot>),

    // ── Procedural Paradigm (ADR-017) ────────────────────────────────────────
    /// Request a checkpoint lookup for a procedural operation.
    GetProceduralCheckpoint {
        operation_id: u32,
        reply: RpcReplyPort<Option<crate::procedural::Checkpoint>>,
    },

    /// Dispatch a procedural activity and wait for completion.
    ProceduralDispatch {
        activity_type: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<Bytes, wtf_common::WtfError>>,
    },

    /// Procedural sleep request.
    ProceduralSleep {
        duration: std::time::Duration,
        reply: RpcReplyPort<Result<(), wtf_common::WtfError>>,
    },

    /// Request the current deterministic timestamp for a procedural operation.
    ProceduralNow {
        reply: ractor::RpcReplyPort<chrono::DateTime<chrono::Utc>>,
    },

    /// Request a deterministic random u64 for a procedural operation.
    ProceduralRandom {
        reply: ractor::RpcReplyPort<u64>,
    },

    /// Procedural workflow task completed successfully.
    ProceduralWorkflowCompleted,

    /// Procedural workflow task failed with an error.
    ProceduralWorkflowFailed(String),
}

/// A point-in-time snapshot of an instance's status, returned by `GetStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStatusSnapshot {
    pub instance_id: InstanceId,
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

/// Serializable view of the phase (can't serialize `InstancePhase` enum directly
/// because it would pull in actor-internal state).
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

// ============================================================================
// OrchestratorMsg
// ============================================================================

/// Messages that can be sent to the `MasterOrchestrator` root supervisor.
#[derive(Debug)]
pub enum OrchestratorMsg {
    /// Start a new workflow instance.
    StartWorkflow {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: RpcReplyPort<Result<InstanceId, StartError>>,
    },

    /// Deliver a signal to a running instance.
    Signal {
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<(), WtfError>>,
    },

    /// Request graceful termination of a running instance.
    Terminate {
        instance_id: InstanceId,
        reason: String,
        reply: RpcReplyPort<Result<(), TerminateError>>,
    },

    /// Query the status of a running instance.
    GetStatus {
        instance_id: InstanceId,
        reply: RpcReplyPort<Option<InstanceStatusSnapshot>>,
    },

    /// List all currently active instances.
    ListActive {
        reply: RpcReplyPort<Vec<InstanceStatusSnapshot>>,
    },

    /// Request the NATS client for JetStream/KV operations.
    GetNatsContext {
        reply: RpcReplyPort<Option<NatsClient>>,
    },

    /// Request the snapshot database handle.
    GetSnapshotDb {
        reply: RpcReplyPort<Option<sled::Db>>,
    },

    /// Heartbeat entry expired in NATS KV — trigger crash recovery for this instance.
    ///
    /// Sent by the heartbeat watcher task (ADR-014). The orchestrator
    /// checks if it already owns this instance; if so, the actor is still alive
    /// and the KV entry is refreshed. If not, the instance needs to be recovered.
    HeartbeatExpired { instance_id: InstanceId },
}

// ============================================================================
// Error types
// ============================================================================

/// Error starting a new workflow instance.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum StartError {
    #[error("orchestrator is at capacity ({running}/{max} instances)")]
    AtCapacity { running: usize, max: usize },
    #[error("instance {0} already exists")]
    AlreadyExists(InstanceId),
    #[error("failed to spawn actor: {0}")]
    SpawnFailed(String),
}

/// Error terminating a workflow instance.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum TerminateError {
    #[error("instance not found: {0}")]
    NotFound(InstanceId),
    #[error("termination failed: {0}")]
    Failed(String),
}

/// Error during heartbeat-driven crash recovery.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RecoveryError {
    #[error("instance metadata not found in KV: {0}")]
    InstanceNotFound(InstanceId),
    #[error("failed to create replay consumer: {0}")]
    ReplayFailed(String),
    #[error("failed to spawn actor: {0}")]
    SpawnFailed(String),
    #[error("NATS client unavailable for recovery")]
    NoNatsClient,
}

/// Metadata stored in `wtf-instances` KV for each running instance.
///
/// Used by crash recovery to respawn an instance without needing the original
/// `InstanceArguments` (which may have been lost if the actor process crashed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetadata {
    pub namespace: NamespaceId,
    pub instance_id: InstanceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub engine_node_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_phase_view_from_replay() {
        let view = InstancePhaseView::from(InstancePhase::Replay);
        assert_eq!(view, InstancePhaseView::Replay);
    }

    #[test]
    fn instance_phase_view_from_live() {
        let view = InstancePhaseView::from(InstancePhase::Live);
        assert_eq!(view, InstancePhaseView::Live);
    }

    #[test]
    fn workflow_paradigm_variants_are_distinct() {
        assert_ne!(WorkflowParadigm::Fsm, WorkflowParadigm::Dag);
        assert_ne!(WorkflowParadigm::Dag, WorkflowParadigm::Procedural);
        assert_ne!(WorkflowParadigm::Fsm, WorkflowParadigm::Procedural);
    }

    #[test]
    fn start_error_at_capacity_message() {
        let err = StartError::AtCapacity {
            running: 100,
            max: 100,
        };
        let msg = err.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("capacity"));
    }

    #[test]
    fn terminate_error_not_found_contains_id() {
        let id = InstanceId::new("test-id");
        let err = TerminateError::NotFound(id);
        assert!(err.to_string().contains("test-id"));
    }

    #[test]
    fn instance_arguments_optional_fields_default_to_none() {
        // Compile-time contract: all optional fields must accept None.
        // This guards callers (master.rs, instance) against missing fields.
        let args = InstanceArguments {
            namespace: NamespaceId::new("ns"),
            instance_id: InstanceId::new("inst"),
            workflow_type: "wf".into(),
            paradigm: WorkflowParadigm::Fsm,
            input: bytes::Bytes::new(),
            engine_node_id: "node".into(),
            nats: None,
            snapshot_db: None,
            procedural_workflow: None,
            workflow_definition: None,
        };
        assert!(args.snapshot_db.is_none());
        assert!(args.workflow_definition.is_none());
        assert!(args.nats.is_none());
        assert!(args.procedural_workflow.is_none());
    }
}
