//! EventStore trait for append-only event log storage (ADR-002).
//!
//! Architecture: Data (`EventStoreError`) → Calc (`append_events_checked`) → Actions
//! (`EventStore` async trait).
//!
//! This module defines the async trait for append-only event storage with
//! Optimistic Concurrency Control (OCC) support.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use vo_types::events::EventEnvelope;
use vo_types::InstanceId;

use crate::key_encoding::{decode_event_key, encode_event_key};
use crate::partitions::EVENTS_PARTITION;

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
            EventStoreError::OccConflict { .. } => {
                vo_types::events::Error::PayloadDecodeSkipped
            }
            EventStoreError::Storage { .. } => vo_types::events::Error::PayloadDecodeFailed(
                Box::new(vo_types::events::Error::InvalidEnvelopeFormat),
            ),
            EventStoreError::InvalidArgument { .. } => {
                vo_types::events::Error::InvalidEnvelopeFormat
            }
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

    async fn get_sequence(
        &self,
        instance_id: &InstanceId,
    ) -> Result<u64, EventStoreError>;
}

#[derive(Debug, Clone)]
pub struct InMemoryEventStore {
    sequences: Arc<RwLock<HashMap<InstanceId, u64>>>,
    events: Arc<RwLock<HashMap<InstanceId, Vec<EventEnvelope>>>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            sequences: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub fn with_events(
        self,
        instance_id: InstanceId,
        events: Vec<EventEnvelope>,
    ) -> Self {
        let mut seq_store = self.sequences.write().unwrap();
        let mut event_store = self.events.write().unwrap();
        if let Some(last) = events.last() {
            seq_store.insert(instance_id.clone(), last.sequence);
        }
        event_store.insert(instance_id, events);
        drop(seq_store);
        drop(event_store);
        self
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append(
        &self,
        instance_id: &InstanceId,
        events: Vec<EventEnvelope>,
    ) -> Result<u64, EventStoreError> {
        if events.is_empty() {
            return Err(EventStoreError::InvalidArgument {
                reason: "events batch cannot be empty".to_string(),
            });
        }

        let expected_sequence = {
            let sequences = self.sequences.read().unwrap();
            sequences.get(instance_id).copied().unwrap_or(0)
        };

        let first_sequence = events
            .first()
            .ok_or(EventStoreError::InvalidArgument {
                reason: "events batch cannot be empty".to_string(),
            })?
            .sequence;

        if first_sequence != expected_sequence + 1 {
            let actual_sequence = {
                let events_store = self.events.read().unwrap();
                events_store
                    .get(instance_id)
                    .and_then(|e| e.last())
                    .map(|e| e.sequence)
                    .unwrap_or(0)
            };
            return Err(EventStoreError::OccConflict {
                instance_id: instance_id.to_string(),
                expected_sequence: expected_sequence + 1,
                actual_sequence,
            });
        }

        for window in events.windows(2) {
            if let [a, b] = window {
                if b.sequence != a.sequence + 1 {
                    return Err(EventStoreError::InvalidArgument {
                        reason: format!(
                            "events are not sequentially ordered: {} followed by {}",
                            a.sequence, b.sequence
                        ),
                    });
                }
            }
        }

        let final_sequence = events.last().unwrap().sequence;

        {
            let mut sequences = self.sequences.write().unwrap();
            let mut events_store = self.events.write().unwrap();
            sequences.insert(instance_id.clone(), final_sequence);
            events_store
                .entry(instance_id.clone())
                .or_insert_with(Vec::new);
            if let Some(existing) = events_store.get_mut(instance_id) {
                existing.extend(events);
            }
        }

        Ok(final_sequence)
    }

    async fn get_sequence(
        &self,
        instance_id: &InstanceId,
    ) -> Result<u64, EventStoreError> {
        let sequences = self.sequences.read().unwrap();
        Ok(sequences.get(instance_id).copied().unwrap_or(0))
    }
}

pub struct FjallEventStore {
    keyspace: Arc<fjall::Keyspace>,
    partition: Arc<fjall::PartitionHandle>,
}

impl FjallEventStore {
    pub fn open(keyspace: &fjall::Keyspace) -> Result<Self, EventStoreError> {
        let partition = keyspace
            .open_partition(EVENTS_PARTITION, fjall::PartitionCreateOptions::default())
            .map_err(|e| EventStoreError::Storage {
                reason: format!("failed to open events partition: {e}"),
            })?;
        Ok(Self {
            keyspace: Arc::new(keyspace.clone()),
            partition: Arc::new(partition),
        })
    }

    fn get_highest_sequence(&self, instance_id: &InstanceId) -> Result<u64, EventStoreError> {
        let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
        let start_key = {
            let mut k = Vec::with_capacity(24);
            k.extend_from_slice(&iid_bytes);
            k.extend_from_slice(&[0u8; 8]);
            k
        };
        let end_key = {
            let mut k = Vec::with_capacity(24);
            k.extend_from_slice(&iid_bytes);
            k.extend_from_slice(&[0xffu8; 8]);
            k
        };

        let mut highest_seq = 0u64;
        let mut cursor = self.partition.range(start_key..end_key);
        while let Some(item) = cursor.next() {
            let (key, _value) = item.map_err(|e| EventStoreError::Storage {
                reason: format!("failed to scan events: {e}"),
            })?;
            if let Ok((_instance_id, seq)) = decode_event_key(&key) {
                let seq_u64 = seq.as_u64();
                if seq_u64 > highest_seq {
                    highest_seq = seq_u64;
                }
            }
        }
        Ok(highest_seq)
    }
}

#[async_trait]
impl EventStore for FjallEventStore {
    async fn append(
        &self,
        instance_id: &InstanceId,
        events: Vec<EventEnvelope>,
    ) -> Result<u64, EventStoreError> {
        if events.is_empty() {
            return Err(EventStoreError::InvalidArgument {
                reason: "events batch cannot be empty".to_string(),
            });
        }

        let expected_sequence = self.get_highest_sequence(instance_id)?;

        let first_sequence = events
            .first()
            .ok_or(EventStoreError::InvalidArgument {
                reason: "events batch cannot be empty".to_string(),
            })?
            .sequence;

        if first_sequence != expected_sequence + 1 {
            return Err(EventStoreError::OccConflict {
                instance_id: instance_id.to_string(),
                expected_sequence: expected_sequence + 1,
                actual_sequence: first_sequence,
            });
        }

        for window in events.windows(2) {
            if let [a, b] = window {
                if b.sequence != a.sequence + 1 {
                    return Err(EventStoreError::InvalidArgument {
                        reason: format!(
                            "events are not sequentially ordered: {} followed by {}",
                            a.sequence, b.sequence
                        ),
                    });
                }
            }
        }

        let final_sequence = events.last().unwrap().sequence;

        let mut batch = self.keyspace.batch();
        for event in &events {
            let seq_num = vo_types::SequenceNumber::try_from(event.sequence)
                .map_err(|_| EventStoreError::InvalidArgument {
                    reason: "sequence number cannot be zero".to_string(),
                })?;
            let key = encode_event_key(instance_id, seq_num);
            let value = serde_json::to_vec(event).map_err(|e| EventStoreError::Storage {
                reason: format!("failed to serialize event: {e}"),
            })?;
            batch.insert(&*self.partition, key, value);
        }
        batch.commit().map_err(|e| EventStoreError::Storage {
            reason: format!("failed to commit events: {e}"),
        })?;

        Ok(final_sequence)
    }

    async fn get_sequence(
        &self,
        instance_id: &InstanceId,
    ) -> Result<u64, EventStoreError> {
        self.get_highest_sequence(instance_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    fn make_envelope(instance_id: &InstanceId, sequence: u64) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            sequence,
            timestamp_ms: 1000 + sequence,
            payload: serde_json::json!({"type": "TestEvent", "seq": sequence}),
            metadata: vo_types::events::EventMetadata::default(),
        }
    }

    #[tokio::test]
    async fn test_in_memory_store_append_single_event() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_store_append_sequential_events() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
            make_envelope(&instance_id, 3),
        ];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_store_get_sequence_returns_zero_for_new_instance() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();

        let result = store.get_sequence(&instance_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_store_get_sequence_after_append() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
        ];

        store.append(&instance_id, events).await.unwrap();
        let result = store.get_sequence(&instance_id).await;
        assert_eq!(result.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_store_occ_conflict_on_wrong_sequence() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        store.append(&instance_id, events).await.unwrap();

        let conflicting_events = vec![make_envelope(&instance_id, 10)];
        let result = store.append(&instance_id, conflicting_events).await;

        assert!(matches!(
            result,
            Err(EventStoreError::OccConflict {
                instance_id: _,
                expected_sequence: 2,
                actual_sequence: 1
            })
        ));
    }

    #[tokio::test]
    async fn test_in_memory_store_rejects_non_sequential_events() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 3),
        ];

        let result = store.append(&instance_id, events).await;
        assert!(matches!(
            result,
            Err(EventStoreError::InvalidArgument { .. })
        ));
    }

    #[tokio::test]
    async fn test_in_memory_store_rejects_empty_batch() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();
        let events = vec![];

        let result = store.append(&instance_id, events).await;
        assert!(matches!(
            result,
            Err(EventStoreError::InvalidArgument { .. })
        ));
    }

    #[tokio::test]
    async fn test_in_memory_store_allows_separate_instances() {
        let store = InMemoryEventStore::new();
        let instance1 = InstanceId::from_bytes([1u8; 16]);
        let instance2 = InstanceId::from_bytes([2u8; 16]);

        let events1 = vec![make_envelope(&instance1, 1)];
        let events2 = vec![make_envelope(&instance2, 1)];

        assert!(store.append(&instance1, events1).await.is_ok());
        assert!(store.append(&instance2, events2).await.is_ok());

        assert_eq!(store.get_sequence(&instance1).await.unwrap(), 1);
        assert_eq!(store.get_sequence(&instance2).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_store_continuation_works() {
        let store = InMemoryEventStore::new();
        let instance_id = make_instance_id();

        let batch1 = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
        ];
        store.append(&instance_id, batch1).await.unwrap();

        let batch2 = vec![
            make_envelope(&instance_id, 3),
            make_envelope(&instance_id, 4),
        ];
        store.append(&instance_id, batch2).await.unwrap();

        assert_eq!(store.get_sequence(&instance_id).await.unwrap(), 4);
    }

    #[tokio::test]
    async fn test_mock_can_intentionally_return_error_on_append() {
        struct FailingEventStore;
        #[async_trait]
        impl EventStore for FailingEventStore {
            async fn append(
                &self,
                _instance_id: &InstanceId,
                _events: Vec<EventEnvelope>,
            ) -> Result<u64, EventStoreError> {
                Err(EventStoreError::Storage {
                    reason: "intentional failure for testing".to_string(),
                })
            }

            async fn get_sequence(
                &self,
                _instance_id: &InstanceId,
            ) -> Result<u64, EventStoreError> {
                Ok(0)
            }
        }

        let store = FailingEventStore;
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        let result = store.append(&instance_id, events).await;
        assert!(matches!(
            result,
            Err(EventStoreError::Storage { .. })
        ));
    }

    #[tokio::test]
    async fn test_fjall_event_store_append_single_event() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_fjall_event_store_append_sequential_events() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
            make_envelope(&instance_id, 3),
        ];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_fjall_event_store_get_sequence_returns_zero_for_new_instance() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance_id = make_instance_id();

        let result = store.get_sequence(&instance_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_fjall_event_store_get_sequence_after_append() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
        ];

        store.append(&instance_id, events).await.unwrap();
        let result = store.get_sequence(&instance_id).await;
        assert_eq!(result.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_fjall_event_store_occ_conflict_on_wrong_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        store.append(&instance_id, events).await.unwrap();

        let conflicting_events = vec![make_envelope(&instance_id, 10)];
        let result = store.append(&instance_id, conflicting_events).await;

        assert!(matches!(
            result,
            Err(EventStoreError::OccConflict {
                instance_id: _,
                expected_sequence: 2,
                actual_sequence: 10
            })
        ));
    }

    #[tokio::test]
    async fn test_fjall_event_store_continuation_works() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance_id = make_instance_id();

        let batch1 = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
        ];
        store.append(&instance_id, batch1).await.unwrap();

        let batch2 = vec![
            make_envelope(&instance_id, 3),
            make_envelope(&instance_id, 4),
        ];
        store.append(&instance_id, batch2).await.unwrap();

        assert_eq!(store.get_sequence(&instance_id).await.unwrap(), 4);
    }

    #[tokio::test]
    async fn test_fjall_event_store_allows_separate_instances() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let store = FjallEventStore::open(&keyspace).unwrap();
        let instance1 = InstanceId::from_bytes([1u8; 16]);
        let instance2 = InstanceId::from_bytes([2u8; 16]);

        let events1 = vec![make_envelope(&instance1, 1)];
        let events2 = vec![make_envelope(&instance2, 1)];

        assert!(store.append(&instance1, events1).await.is_ok());
        assert!(store.append(&instance2, events2).await.is_ok());

        assert_eq!(store.get_sequence(&instance1).await.unwrap(), 1);
        assert_eq!(store.get_sequence(&instance2).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_fjall_event_store_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let keyspace = fjall::Config::new(dir.path()).open().unwrap();
        let instance_id = make_instance_id();

        {
            let store = FjallEventStore::open(&keyspace).unwrap();
            let events = vec![make_envelope(&instance_id, 1)];
            store.append(&instance_id, events).await.unwrap();
        }

        let keyspace2 = fjall::Config::new(dir.path()).open().unwrap();
        let store2 = FjallEventStore::open(&keyspace2).unwrap();
        assert_eq!(store2.get_sequence(&instance_id).await.unwrap(), 1);
    }
}
