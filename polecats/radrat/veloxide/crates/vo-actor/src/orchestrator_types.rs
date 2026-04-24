//! Orchestrator-level types: message protocol, snapshots, and error types.

pub use vo_common::NamespaceId;
use bytes::Bytes;
use vo_types::InstanceId;

// WorkflowParadigm defined below in this module — no crate-level import needed

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("failed: {0}")]
    Failed(String),
}

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
        reply: ractor::port::RpcReplyPort<Result<(), crate::StartError>>,
    },
    /// Get status of a workflow instance
    GetStatus {
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Option<crate::InstanceSnapshot>>,
    },
    /// Terminate a workflow instance
    Terminate {
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    /// List all active workflow instances
    ListActive {
        reply: ractor::port::RpcReplyPort<Vec<crate::InstanceSnapshot>>,
    },
    /// Trigger compensation for a workflow instance
    Compensate {
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
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
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminate_error_variants_can_be_constructed() {
        let err_not_found = TerminateError::NotFound("wf-123".to_string());
        assert!(matches!(err_not_found, TerminateError::NotFound(msg) if msg == "wf-123"));

        let err_failed = TerminateError::Failed("crashed".to_string());
        assert!(matches!(err_failed, TerminateError::Failed(msg) if msg == "crashed"));
    }
}
