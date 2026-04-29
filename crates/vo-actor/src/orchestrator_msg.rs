//! Orchestrator-level types: messages, errors, and state snapshots.

use bytes::Bytes;
use vo_types::InstanceId;

pub use crate::fairness::WorkloadClass;

pub type NamespaceId = String;

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
    Terminated,
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

#[derive(Debug, Clone)]
pub struct InstanceSnapshot {
    pub instance_id: InstanceId,
    pub namespace: NamespaceId,
    pub workflow_type: String,
    pub paradigm: WorkflowParadigm,
    pub phase: InstancePhaseView,
    pub events_applied: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    #[error("Budget exhausted for {class:?}: requested {requested}, available {available}")]
    BudgetExhaustion {
        class: WorkloadClass,
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

#[derive(Debug, Clone)]
pub struct ReservedPermitBudget {
    max_per_class: u32,
    class_counts: std::collections::HashMap<WorkloadClass, u32>,
}

impl ReservedPermitBudget {
    #[must_use]
    pub fn new(max_per_class: u32) -> Self {
        assert!(max_per_class > 0, "max_per_class must be > 0");
        Self {
            max_per_class,
            class_counts: std::collections::HashMap::new(),
        }
    }

    pub fn try_acquire(&mut self, class: WorkloadClass) -> Result<(), StartError> {
        let current = self.class_counts.get(&class).copied().unwrap_or(0);
        if current >= self.max_per_class {
            return Err(StartError::BudgetExhaustion {
                class,
                requested: 1,
                available: self.max_per_class - current,
            });
        }
        *self.class_counts.entry(class).or_insert(0) += 1;
        Ok(())
    }

    pub fn release(&mut self, class: WorkloadClass) {
        let count = self.class_counts.get(&class).copied().unwrap_or(0);
        if count == 0 {
            return;
        }
        self.class_counts.insert(class, count - 1);
    }

    pub fn reset(&mut self) {
        self.class_counts.clear();
    }

    #[must_use]
    pub fn available(&self, class: WorkloadClass) -> u32 {
        let used = self.class_counts.get(&class).copied().unwrap_or(0);
        self.max_per_class.saturating_sub(used)
    }

    #[must_use]
    pub fn is_exhausted(&self, class: WorkloadClass) -> bool {
        self.available(class) == 0
    }
}

#[derive(Debug)]
pub enum OrchestratorMsg {
    StartWorkflow {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), StartError>>,
    },
    ReserveWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), StartError>>,
    },
    CommitWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), StartError>>,
    },
    AbortWorkflowStart {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<()>,
    },
    GetStatus {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Option<InstanceSnapshot>>,
    },
    Terminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    ReserveTerminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    CommitTerminate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    AbortWorkflowTransition {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<()>,
    },
    ListActive {
        reply: ractor::port::RpcReplyPort<Vec<InstanceSnapshot>>,
    },
    Compensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    ReserveCompensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    CommitCompensate {
        namespace: NamespaceId,
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    Signal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
    ReserveSignal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
    CommitSignal {
        namespace: NamespaceId,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
}
