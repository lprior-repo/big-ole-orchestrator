//! Storage abstraction traits for wtf-engine.
//!
//! Removes direct dependency on `wtf-storage` in core actor types (ADR-006 drift).

use crate::{InstanceId, InstanceMetadata, NamespaceId, WorkflowEvent, WtfError};
use async_trait::async_trait;
use bytes::Bytes;

/// A single decoded event from the replay stream.
#[derive(Debug, Clone)]
pub struct ReplayedEvent {
    pub seq: u64,
    pub event: WorkflowEvent,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A single fetched batch from the replay consumer.
#[derive(Debug)]
pub enum ReplayBatch {
    Event(ReplayedEvent),
    TailReached,
}

/// Interface for writing and replaying workflow events.
#[async_trait]
pub trait EventStore: Send + Sync + std::fmt::Debug + 'static {
    /// Publish a workflow event to the event log.
    async fn publish(
        &self,
        ns: &NamespaceId,
        inst: &InstanceId,
        event: WorkflowEvent,
    ) -> Result<u64, WtfError>;

    /// Create a stream of events for replay.
    async fn open_replay_stream(
        &self,
        ns: &NamespaceId,
        inst: &InstanceId,
        from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError>;
}

/// Interface for replaying events.
#[async_trait]
pub trait ReplayStream: Send + Sync + 'static {
    /// Fetch the next event, or detect stream tail via timeout.
    async fn next_event(&mut self) -> Result<ReplayBatch, WtfError>;

    /// Fetch the next event without timeout (for Live Phase).
    async fn next_live_event(&mut self) -> Result<ReplayedEvent, WtfError>;
}

/// Interface for managing instance lifecycle state and heartbeats.
#[async_trait]
pub trait StateStore: Send + Sync + std::fmt::Debug + 'static {
    /// Persist instance metadata to the global instance registry.
    async fn put_instance_metadata(&self, metadata: InstanceMetadata) -> Result<(), WtfError>;

    /// Retrieve instance metadata from the registry by ID.
    async fn get_instance_metadata(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Option<InstanceMetadata>, WtfError>;

    /// Update the heartbeat for a running instance.
    async fn put_heartbeat(&self, node_id: &str, instance_id: &InstanceId) -> Result<(), WtfError>;

    /// Register a timer in the durable timer store.
    async fn put_timer(&self, timer_id: &str, payload: Bytes) -> Result<(), WtfError>;
}

/// Interface for dispatching activities to workers.
#[async_trait]
pub trait TaskQueue: Send + Sync + std::fmt::Debug + 'static {
    /// Dispatch a task (activity) to the worker queue.
    async fn dispatch(&self, activity_type: &str, payload: Bytes) -> Result<(), WtfError>;
}
