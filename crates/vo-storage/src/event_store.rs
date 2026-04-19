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
            EventStoreError::Storage { .. } => vo_types::events::Error::PayloadDecodeFailed {
                source: Box::new(vo_types::events::Error::InvalidEnvelopeFormat),
            },
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

const META_SEQ_KEY_PREFIX: u8 = 0x00;

fn encode_sequence_key(instance_id: &InstanceId) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + 16);
    key.push(META_SEQ_KEY_PREFIX);
    key.extend_from_slice(&instance_id.to_bytes().unwrap_or([0u8; 16]));
    key
}

pub struct FjallEventStore {
    partition: Arc<fjall::Keyspace>,
}

impl std::fmt::Debug for FjallEventStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallEventStore").finish()
    }
}

impl FjallEventStore {
    pub fn open(db: &fjall::Database) -> Result<Self, EventStoreError> {
        let partition = db
            .keyspace(EVENTS_PARTITION, || {
                fjall::KeyspaceCreateOptions::default()
            })
            .map_err(|e| EventStoreError::Storage {
                reason: format!("failed to open events partition: {e}"),
            })?;
        Ok(Self {
            partition: Arc::new(partition),
        })
    }

    fn get_sequence_sync(partition: &fjall::Keyspace, instance_id: &InstanceId) -> Result<u64, EventStoreError> {
        let key = encode_sequence_key(instance_id);
        match partition.get(&key) {
            Ok(Some(bytes)) => {
                let bytes: &[u8] = bytes.as_ref();
                if bytes.len() != 8 {
                    return Err(EventStoreError::Storage {
                        reason: format!("invalid sequence bytes length: {}", bytes.len()),
                    });
                }
                let arr: [u8; 8] = bytes
                    .try_into()
                    .map_err(|_| EventStoreError::Storage {
                        reason: "failed to decode sequence bytes".to_string(),
                    })?;
                Ok(u64::from_be_bytes(arr))
            }
            Ok(None) => Ok(0),
            Err(e) => Err(EventStoreError::Storage {
                reason: e.to_string(),
            }),
        }
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

        let partition = self.partition.clone();
        let iid = instance_id.clone();

        let expected_sequence = tokio::task::spawn_blocking(move || {
            Self::get_sequence_sync(&partition, &iid)
        })
        .await
        .map_err(|e| EventStoreError::Storage {
            reason: format!("join error: {}", e),
        })??;

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
                actual_sequence: expected_sequence,
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

        let partition = self.partition.clone();
        let iid = instance_id.clone();
        let events_data: Vec<(u64, Vec<u8>)> = events
            .into_iter()
            .map(|e| {
                let seq = e.sequence;
                let bytes = serde_json::to_vec(&e).map_err(|err| EventStoreError::Storage {
                    reason: format!("failed to serialize event: {}", err),
                })?;
                Ok((seq, bytes))
            })
            .collect::<Result<Vec<_>, _>>()?;

        tokio::task::spawn_blocking(move || {
            for (seq, bytes) in &events_data {
                let key = encode_event_key(&iid, vo_types::SequenceNumber::new_unchecked(*seq));
                partition.insert(&key, bytes).map_err(|e| EventStoreError::Storage {
                    reason: format!("failed to persist event: {}", e),
                })?;
            }
            let meta_key = encode_sequence_key(&iid);
            partition.insert(&meta_key, &final_sequence.to_be_bytes()).map_err(|e| EventStoreError::Storage {
                reason: format!("failed to persist sequence: {}", e),
            })?;
            Ok::<(), EventStoreError>(())
        })
        .await
        .map_err(|e| EventStoreError::Storage {
            reason: format!("join error: {}", e),
        })??;

        Ok(final_sequence)
    }

    async fn get_sequence(
        &self,
        instance_id: &InstanceId,
    ) -> Result<u64, EventStoreError> {
        let partition = self.partition.clone();
        let iid = instance_id.clone();
        tokio::task::spawn_blocking(move || {
            Self::get_sequence_sync(&partition, &iid)
        })
        .await
        .map_err(|e| EventStoreError::Storage {
            reason: format!("join error: {}", e),
        })?
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
}

#[cfg(test)]
mod fjall_tests {
    use super::*;

    fn create_test_db() -> (tempfile::TempDir, fjall::Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        (dir, db)
    }

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
    async fn test_fjall_store_append_single_event() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_fjall_store_append_sequential_events() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
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
    async fn test_fjall_store_get_sequence_returns_zero_for_new_instance() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
        let instance_id = make_instance_id();

        let result = store.get_sequence(&instance_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_fjall_store_get_sequence_after_append() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
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
    async fn test_fjall_store_occ_conflict_on_wrong_sequence() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
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
    async fn test_fjall_store_rejects_non_sequential_events() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
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
    async fn test_fjall_store_rejects_empty_batch() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
        let instance_id = make_instance_id();
        let events = vec![];

        let result = store.append(&instance_id, events).await;
        assert!(matches!(
            result,
            Err(EventStoreError::InvalidArgument { .. })
        ));
    }

    #[tokio::test]
    async fn test_fjall_store_allows_separate_instances() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
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
    async fn test_fjall_store_continuation_works() {
        let (_dir, db) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
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
    async fn test_fjall_store_persists_across_restart() {
        let (dir, db) = create_test_db();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
        ];

        {
            let store = FjallEventStore::open(&db).unwrap();
            store.append(&instance_id, events.clone()).await.unwrap();
        }

        drop(db);

        let db2 = fjall::Database::builder(dir.path()).open().unwrap();
        let store2 = FjallEventStore::open(&db2).unwrap();

        assert_eq!(store2.get_sequence(&instance_id).await.unwrap(), 2);
    }
}
