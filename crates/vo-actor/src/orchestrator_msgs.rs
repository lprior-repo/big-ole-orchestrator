//! Orchestrator message types and instance snapshot.

use super::domain_types::{InstancePhaseView, WorkflowParadigm};
use super::error_types::{CompensateError, SignalError, TerminateError};
use crate::{InstanceId, NamespaceId};
use bytes::Bytes;

pub fn run_heartbeat_watcher() {}

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
        reply: ractor::port::RpcReplyPort<Result<(), super::error_types::StartError>>,
    },
    /// Get status of a workflow instance
    GetStatus {
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Option<InstanceSnapshot>>,
    },
    /// Terminate a workflow instance
    Terminate {
        instance_id: InstanceId,
        reason: String,
        reply: ractor::port::RpcReplyPort<Result<(), TerminateError>>,
    },
    /// List all active workflow instances
    ListActive {
        reply: ractor::port::RpcReplyPort<Vec<InstanceSnapshot>>,
    },
    /// Compensate a completed workflow
    Compensate {
        instance_id: InstanceId,
        reply: ractor::port::RpcReplyPort<Result<(), CompensateError>>,
    },
    /// Send a signal to a workflow instance
    Signal {
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
        reply: ractor::port::RpcReplyPort<Result<(), SignalError>>,
    },
}
