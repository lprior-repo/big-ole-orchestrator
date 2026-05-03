//! Orchestrator-level message types.
//!
//! Messages sent to the orchestrator actor for workflow lifecycle management.

use bytes::Bytes;
use ractor::port::RpcReplyPort;
use vo_types::InstanceId;

use crate::{InstancePhaseView, WorkflowParadigm};

/// Messages sent to the orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorMsg {
    /// Start a new workflow instance
    StartWorkflow {
        namespace: String,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: RpcReplyPort<Result<(), StartError>>,
    },
    /// Get status of a workflow instance
    GetStatus {
        instance_id: InstanceId,
        reply: RpcReplyPort<Option<InstanceSnapshot>>,
    },
    /// Terminate a workflow instance
    Terminate {
        instance_id: InstanceId,
        reason: String,
        reply: RpcReplyPort<Result<(), TerminateError>>,
    },
    /// List all active workflow instances
    ListActive {
        reply: RpcReplyPort<Vec<InstanceSnapshot>>,
    },
    /// Compensate a completed workflow
    Compensate {
        instance_id: InstanceId,
        reply: RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Send a signal to a workflow instance
    Signal {
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: RpcReplyPort<Result<(), SignalError>>,
    },
}

/// Error type for signal operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("instance not found: {0}")]
    NotFound(String),
    #[error("signal failed: {0}")]
    Failed(String),
}

/// Error type for compensation operations.
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
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

/// Errors from actor start operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("Budget exhausted for {class:?}: requested {requested}, available {available}")]
    BudgetExhaustion {
        class: crate::WorkloadClass,
        requested: u32,
        available: u32,
    },
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("At capacity: {running}/{max} instances running")]
    AtCapacity { running: u32, max: u32 },
    #[error("Instance {0} already exists")]
    AlreadyExists(String),
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
}
