//! MasterOrchestrator message types.

use super::errors::{GetStatusError, StartError, TerminateError};
use super::instance::InstanceStatusSnapshot;
use bytes::Bytes;
use ractor::RpcReplyPort;
use std::sync::Arc;
use wtf_common::{
    EventStore, InstanceId, NamespaceId, StateStore, TaskQueue, WorkflowParadigm, WtfError,
};

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
        reply: RpcReplyPort<Result<Option<InstanceStatusSnapshot>, GetStatusError>>,
    },

    /// List all currently active instances.
    ListActive {
        reply: RpcReplyPort<Vec<InstanceStatusSnapshot>>,
    },

    /// Request the event store for JetStream operations.
    GetEventStore {
        reply: RpcReplyPort<Option<Arc<dyn EventStore>>>,
    },

    /// Request the state store for KV operations.
    GetStateStore {
        reply: RpcReplyPort<Option<Arc<dyn StateStore>>>,
    },

    /// Request the task queue for activity dispatch.
    GetTaskQueue {
        reply: RpcReplyPort<Option<Arc<dyn TaskQueue>>>,
    },

    /// Request the snapshot database handle.
    GetSnapshotDb {
        reply: RpcReplyPort<Option<sled::Db>>,
    },

    /// Heartbeat entry expired in NATS KV — trigger crash recovery for this instance.
    HeartbeatExpired { instance_id: InstanceId },
}
