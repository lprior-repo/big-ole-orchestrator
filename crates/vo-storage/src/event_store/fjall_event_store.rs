//! Fjall-backed persistent implementation of `EventStore` for production use (ADR-002).
//!
//! Concurrency model: `parking_lot::Mutex` per-instance-id guards the OCC check-and-append
//! critical section. Uses a striped lock approach (like `FjallDedupeStore`) to allow
//! independent instances to proceed in parallel.
//!
//! Key format: `[instance_id(16)][sequence_u64_be(8)]` = 24 bytes.
//! Hot spot mitigation: optional XOR-based key scrambling via `HotSpotProvider`.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use vo_types::events::EventEnvelope;
use vo_types::InstanceId;

use super::{EventStore, EventStoreError, FjallEventStoreOptions};
use crate::hot_spot::{HotSpotProvider, HotSpotDetector};
use crate::partitions::EVENTS_PARTITION;

const NUM_STRIPES: usize = 64;

fn stripe_for_instance(id_bytes: &[u8]) -> usize {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(id_bytes);
    (hasher.finalize() as usize) % NUM_STRIPES
}

pub struct FjallEventStore {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
    stripes: Vec<Mutex<()>>,
    hot_spot: Option<Arc<dyn HotSpotProvider>>,
}

impl std::fmt::Debug for FjallEventStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FjallEventStore").finish()
    }
}

impl FjallEventStore {
    /// Opens a new event store backed by the given database.
    ///
    /// # Errors
    ///
    /// Returns `EventStoreError::Storage` if the events partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, EventStoreError> {
        Self::open_with_options(db, FjallEventStoreOptions::default())
    }

    /// Opens a new event store with optional hot spot detection.
    ///
    /// # Errors
    ///
    /// Returns `EventStoreError::Storage` if the events partition cannot be opened.
    pub fn open_with_options(
        db: &fjall::Database,
        options: FjallEventStoreOptions,
    ) -> Result<Self, EventStoreError> {
        let partition = db
            .keyspace(EVENTS_PARTITION, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| EventStoreError::Storage {
                reason: format!("failed to open events partition: {e}"),
            })?;
        let stripes = (0..NUM_STRIPES).map(|_| Mutex::new(())).collect();

        let hot_spot = options
            .hot_spot_config
            .map(|config| -> Arc<dyn HotSpotProvider> {
                Arc::new(HotSpotDetector::new(config))
            });

        Ok(Self {
            db: Arc::new(db.clone()),
            partition: Arc::new(partition),
            stripes,
            hot_spot,
        })
    }

    fn get_current_sequence(&self, instance_id: &InstanceId) -> Result<u64, EventStoreError> {
        let id_bytes = instance_id
            .to_bytes()
            .map_err(|_| EventStoreError::Storage {
                reason: format!("cannot convert instance_id {instance_id} to bytes"),
            })?;

        let mut max_seq: u64 = 0;
        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, _) = item.into_inner().map_err(|e| EventStoreError::Storage {
                reason: e.to_string(),
            })?;

            if key_bytes.len() < 24 {
                continue;
            }

            if !key_bytes[..16].starts_with(&id_bytes) {
                continue;
            }

            let seq_bytes: [u8; 8] =
                key_bytes[16..24]
                    .try_into()
                    .map_err(|_| EventStoreError::Storage {
                        reason: "malformed event key (sequence bytes)".to_string(),
                    })?;
            let seq = u64::from_be_bytes(seq_bytes);
            if seq > max_seq {
                max_seq = seq;
            }
        }

        Ok(max_seq)
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

        let first_sequence = events
            .first()
            .ok_or(EventStoreError::InvalidArgument {
                reason: "events batch cannot be empty".to_string(),
            })?
            .sequence;
        let final_sequence = events.last().expect("non-empty checked above").sequence;

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

        let id_bytes = instance_id
            .to_bytes()
            .map_err(|_| EventStoreError::Storage {
                reason: format!("cannot convert instance_id {instance_id} to bytes"),
            })?;
        let stripe_idx = stripe_for_instance(&id_bytes);
        let _guard = self.stripes[stripe_idx].lock();

        let current_sequence = self.get_current_sequence(instance_id)?;
        let expected_sequence = current_sequence + 1;

        if first_sequence != expected_sequence {
            return Err(EventStoreError::OccConflict {
                instance_id: instance_id.to_string(),
                expected_sequence,
                actual_sequence: current_sequence,
            });
        }

        // Hot spot detection: record append and check if instance is hot
        let is_hot = if let Some(ref detector) = self.hot_spot {
            detector.record_append(instance_id)
        } else {
            false
        };

        let mut batch = self.db.batch();
        for event in &events {
            let seq_bytes = event.sequence.to_be_bytes();
            let mut key = Vec::with_capacity(24);
            if is_hot {
                let scrambled = crate::hot_spot::scramble_instance_id(instance_id);
                key.extend_from_slice(&scrambled);
            } else {
                key.extend_from_slice(&id_bytes);
            }
            key.extend_from_slice(&seq_bytes);

            let value = serde_json::to_vec(event).map_err(|e| EventStoreError::Storage {
                reason: format!("failed to serialize event: {e}"),
            })?;

            batch.insert(&self.partition, key, value);
        }

        batch.commit().map_err(|e| EventStoreError::Storage {
            reason: format!("failed to commit event batch: {e}"),
        })?;

        Ok(final_sequence)
    }

    async fn get_sequence(&self, instance_id: &InstanceId) -> Result<u64, EventStoreError> {
        self.get_current_sequence(instance_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};
    use vo_types::events::EventMetadata;

    fn create_test_db() -> (fjall::Database, TempDir) {
        let dir = tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        (db, dir)
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
            metadata: EventMetadata::default(),
        }
    }

    #[tokio::test]
    async fn fjall_event_store_append_single_event() {
        let (db, _dir) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn fjall_event_store_append_sequential_events() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_get_sequence_returns_zero_for_new_instance() {
        let (db, _dir) = create_test_db();
        let store = FjallEventStore::open(&db).unwrap();
        let instance_id = make_instance_id();

        let result = store.get_sequence(&instance_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn fjall_event_store_get_sequence_after_append() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_occ_conflict_on_wrong_sequence() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_rejects_non_sequential_events() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_rejects_empty_batch() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_allows_separate_instances() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_continuation_works() {
        let (db, _dir) = create_test_db();
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
    async fn fjall_event_store_events_persist_across_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        let db1 = fjall::Database::builder(&path).open().unwrap();
        let store1 = FjallEventStore::open(&db1).unwrap();
        let instance_id = make_instance_id();
        let events = vec![
            make_envelope(&instance_id, 1),
            make_envelope(&instance_id, 2),
        ];
        store1.append(&instance_id, events).await.unwrap();
        drop(store1);
        drop(db1);

        let db2 = fjall::Database::builder(&path).open().unwrap();
        let store2 = FjallEventStore::open(&db2).unwrap();
        assert_eq!(store2.get_sequence(&instance_id).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn fjall_event_store_hot_spot_detected() {
        let (db, _dir) = create_test_db();
        let config = crate::hot_spot::HotSpotConfig {
            max_events: 10,
            max_writes_per_second: 1000,
            window_ms: 1000,
        };
        let options = FjallEventStoreOptions {
            hot_spot_config: Some(config),
        };
        let store = FjallEventStore::open_with_options(&db, options).unwrap();
        let instance_id = make_instance_id();

        for seq in 1..=10 {
            let events = vec![make_envelope(&instance_id, seq)];
            let _ = store.append(&instance_id, events).await;
        }
    }

    #[tokio::test]
    async fn fjall_event_store_default_options_no_hot_spot() {
        let (db, _dir) = create_test_db();
        let options = FjallEventStoreOptions::default();
        let store = FjallEventStore::open_with_options(&db, options).unwrap();
        let instance_id = make_instance_id();
        let events = vec![make_envelope(&instance_id, 1)];

        let result = store.append(&instance_id, events).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    async fn fjall_event_store_multi_instance_no_cross_contamination() {
        let (db, _dir) = create_test_db();
        let options = FjallEventStoreOptions::default();
        let store = FjallEventStore::open_with_options(&db, options).unwrap();
        let instance1 = InstanceId::from_bytes([1u8; 16]);
        let instance2 = InstanceId::from_bytes([2u8; 16]);

        let events1 = vec![make_envelope(&instance1, 1)];
        let events2 = vec![make_envelope(&instance2, 1)];

        assert!(store.append(&instance1, events1).await.is_ok());
        assert!(store.append(&instance2, events2).await.is_ok());

        assert_eq!(store.get_sequence(&instance1).await.unwrap(), 1);
        assert_eq!(store.get_sequence(&instance2).await.unwrap(), 1);
    }
}
