//! Actor message types for workflow instance actors.
//!
//! This module was moved from vo-actor/src/lib.rs as part of the
//! ADR-016/v2 module split refactoring.

use bytes::Bytes;
use ractor::port::RpcReplyPort;
use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId, WorkflowName};

use crate::start_budget::StartError;

pub type NamespaceId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowParadigm {
    Fsm,
    Dag,
    Procedural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstancePhaseView {
    Replay,
    Live,
    Terminated,
}

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("signal failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompensateError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("compensation failed: {0}")]
    Failed(String),
}

/// Instance snapshot for status queries.
#[derive(Debug, Clone)]
pub struct InstanceSnapshot {
    pub instance_id: InstanceId,
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

/// Messages sent to the orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorMsg {
    /// Start a new workflow instance
    StartWorkflow {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: RpcReplyPort<Result<(), StartError>>,
    },
    /// Reserve resources for starting a workflow (two-phase start, phase 1)
    ReserveWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: RpcReplyPort<Result<(), StartError>>,
    },
    /// Commit a reserved workflow start (two-phase start, phase 2)
    CommitWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: RpcReplyPort<Result<(), StartError>>,
    },
    /// Abort a reserved workflow start
    AbortWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: RpcReplyPort<()>,
    },
    /// Get status of a workflow instance
    GetStatus {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: RpcReplyPort<Option<InstanceSnapshot>>,
    },
    /// Terminate a workflow instance
    Terminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: RpcReplyPort<Result<(), TerminateError>>,
    },
    /// Reserve termination of a workflow instance (two-phase terminate, phase 1)
    ReserveTerminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: RpcReplyPort<Result<(), TerminateError>>,
    },
    /// Commit a reserved termination (two-phase terminate, phase 2)
    CommitTerminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: RpcReplyPort<Result<(), TerminateError>>,
    },
    /// Abort a reserved workflow transition
    AbortWorkflowTransition {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: RpcReplyPort<()>,
    },
    /// List all active workflow instances
    ListActive {
        reply: RpcReplyPort<Vec<InstanceSnapshot>>,
    },
    /// Compensate a completed workflow instance
    Compensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Reserve compensation for a workflow instance (two-phase compensate, phase 1)
    ReserveCompensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Commit a reserved compensation (two-phase compensate, phase 2)
    CommitCompensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Send a signal to a workflow instance
    Signal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<(), SignalError>>,
    },
    /// Reserve a signal for a workflow instance
    ReserveSignal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        reply: RpcReplyPort<Result<(), SignalError>>,
    },
    /// Commit a reserved signal
    CommitSignal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<(), SignalError>>,
    },
}

/// Messages sent to/from workflow instance actors.
///
/// These are commands that drive the workflow instance lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceActorMessage {
    /// Start a new workflow instance
    StartWorkflow {
        instance_id: InstanceId,
        workflow_name: WorkflowName,
        node_name: NodeName,
    },
    /// A step in the workflow completed
    StepCompleted {
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
    },
    /// A step in the workflow failed
    StepFailed {
        instance_id: InstanceId,
        node_name: NodeName,
        sequence: SequenceNumber,
        error: String,
    },
    /// A timer fired
    TimerFired {
        instance_id: InstanceId,
        timer_id: TimerId,
    },
    /// Cancellation was requested
    CancelRequested { instance_id: InstanceId },
    /// Get current status query
    GetStatus { instance_id: InstanceId },
}

/// Control messages for lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlActorMessage {
    /// Request cancellation of an instance
    Cancel { instance_id: InstanceId },
    /// Request resumption of a paused instance
    Resume { instance_id: InstanceId },
    /// Atomically accept a signal and resume the waiting instance.
    AcceptAndResume {
        instance_id: InstanceId,
        wait_key: crate::WaitKey,
        signal_id: String,
        payload: crate::SignalPayload,
    },
    /// Request continue-as-new rollover to a new epoch (ADR-038).
    ContinueAsNew {
        instance_id: InstanceId,
        lineage_id: String,
        new_instance_id: InstanceId,
    },
}
