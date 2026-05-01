//! `EventStore` trait for append-only event log storage (ADR-002).
//!
//! Architecture: Data (`EventStoreError`) → Calc (`append_events_checked`) → Actions
//! (`EventStore` async trait).

pub mod encoding;
pub mod fjall_event_store;
pub mod in_memory;

pub use fjall_event_store::FjallEventStore;
pub use in_memory::InMemoryEventStore;

pub use crate::hot_spot::HotSpotConfig;

use async_trait::async_trait;
use vo_types::events::EventEnvelope;
use vo_types::InstanceId;

#[derive(Debug, Clone, Default)]
pub struct FjallEventStoreOptions {
    pub hot_spot_config: Option<HotSpotConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventStoreError {
    #[error("optimistic concurrency control conflict for instance {instance_id}: expected {expected_sequence} but found {actual_sequence}")]
    OccConflict {
        instance_id: String,
        expected_sequence: u64,
        actual_sequence: u64,
    },
    #[error("storage error: {reason}")]
    Storage { reason: String },
    #[error("invalid argument: {reason}")]
    InvalidArgument { reason: String },
}

impl From<EventStoreError> for vo_types::events::Error {
    fn from(e: EventStoreError) -> Self {
        match e {
            EventStoreError::OccConflict { .. } => vo_types::events::Error::PayloadDecodeSkipped,
            EventStoreError::Storage { .. } => vo_types::events::Error::PayloadDecodeFailed {
                source: Box::new(vo_types::events::Error::InvalidEnvelopeFormat),
            },
            EventStoreError::InvalidArgument { .. } => Self::InvalidEnvelopeFormat,
        }
    }
}

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(
        &self,
        instance_id: &InstanceId,
        events: Vec<EventEnvelope>,
    ) -> Result<u64, EventStoreError>;

    async fn get_sequence(&self, instance_id: &InstanceId) -> Result<u64, EventStoreError>;
}
