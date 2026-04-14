//! Fjall-backed persistent implementation of `DedupeStore` for production use.
//!
//! Concurrency model: striped `parking_lot::Mutex` guards the check-and-insert
//! critical section per-key-shard, preventing TOCTOU races between `get` and
//! `insert` on the same dedupe key while allowing independent keys to proceed
//! in parallel.

use std::sync::Arc;

use parking_lot::Mutex;
use vo_types::{DedupeKey, InstanceId};

use super::{AdmissionResult, DedupeStore, DedupeStoreError, DEDUPE_PARTITION};

const NUM_STRIPES: usize = 64;
const PURGE_BATCH_SIZE: usize = 1024;

fn stripe_for_key(key_bytes: &[u8]) -> usize {
    crc32fast::hash(key_bytes) as usize % NUM_STRIPES
}

pub struct FjallDedupeStore {
    keyspace: Arc<fjall::Keyspace>,
    partition: Arc<fjall::PartitionHandle>,
    stripes: Vec<Mutex<()>>,
}

impl FjallDedupeStore {
    /// Opens a new `FjallDedupeStore` backed by the given keyspace.
    ///
    /// # Errors
    ///
    /// Returns `DedupeStoreError::Storage` if the dedupe partition cannot be opened.
    pub fn open(keyspace: &fjall::Keyspace) -> Result<Self, DedupeStoreError> {
        let partition = keyspace
            .open_partition(DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
            .map_err(|e| DedupeStoreError::Storage {
                reason: format!("failed to open dedupe partition: {e}"),
            })?;
        let stripes = (0..NUM_STRIPES).map(|_| Mutex::new(())).collect();
        Ok(Self {
            keyspace: Arc::new(keyspace.clone()),
            partition: Arc::new(partition),
            stripes,
        })
    }

    #[expect(clippy::expect_used, clippy::cast_possible_truncation)]
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect(
                "system time is guaranteed to be after UNIX epoch on properly configured systems",
            )
            .as_millis() as u64
    }
}

impl DedupeStore for FjallDedupeStore {
    fn check_and_insert(
        &self,
        key: &DedupeKey,
        instance_id: &InstanceId,
        ttl_ms: u64,
    ) -> Result<AdmissionResult, DedupeStoreError> {
        if ttl_ms == 0 {
            return Err(DedupeStoreError::InvalidArgument);
        }

        let encoded_key = super::encode_dedupe_key(key);
        let stripe_idx = stripe_for_key(&encoded_key);
        let _guard = self.stripes[stripe_idx].lock();

        let now_ms = Self::now_ms();
        let expires_at = now_ms.saturating_add(ttl_ms);

        if let Ok(Some(value_bytes)) = self.partition.get(&encoded_key) {
            let entry = super::decode_dedupe_entry(&value_bytes)?;
            if !entry.is_expired(now_ms) {
                return Ok(AdmissionResult::Duplicate {
                    instance_id: entry.instance_id().to_string(),
                });
            }
        }

        let entry = super::DedupeEntry::new(
            key.as_str().to_string(),
            instance_id.to_string(),
            expires_at,
        )?;
        let value_bytes = super::encode_dedupe_entry(&entry)?;
        self.partition
            .insert(&encoded_key, &value_bytes)
            .map_err(|e| DedupeStoreError::Storage {
                reason: e.to_string(),
            })?;

        Ok(AdmissionResult::Admitted)
    }

    fn purge_expired(&self, now_ms: u64) -> Result<u64, DedupeStoreError> {
        let mut purged_count = 0u64;
        let mut keys_to_delete = Vec::with_capacity(PURGE_BATCH_SIZE);

        let iter = self.partition.iter();
        for item in iter {
            let (key_bytes, value_bytes) = item.map_err(|e| DedupeStoreError::Storage {
                reason: e.to_string(),
            })?;

            if let Ok(entry) = super::decode_dedupe_entry(&value_bytes) {
                if entry.is_expired(now_ms) {
                    keys_to_delete.push(key_bytes.to_vec());
                }
            }

            if keys_to_delete.len() >= PURGE_BATCH_SIZE {
                let mut batch = self.keyspace.batch();
                for key in std::mem::take(&mut keys_to_delete) {
                    batch.remove(&self.partition, key);
                }
                batch.commit().map_err(|e| DedupeStoreError::Storage {
                    reason: e.to_string(),
                })?;
                purged_count += keys_to_delete.capacity() as u64;
                keys_to_delete.clear();
            }
        }

        if !keys_to_delete.is_empty() {
            let mut batch = self.keyspace.batch();
            for key in &keys_to_delete {
                batch.remove(&self.partition, key.clone());
            }
            batch.commit().map_err(|e| DedupeStoreError::Storage {
                reason: e.to_string(),
            })?;
            purged_count += keys_to_delete.len() as u64;
        }

        Ok(purged_count)
    }

    fn contains(&self, key: &DedupeKey) -> Result<bool, DedupeStoreError> {
        let encoded_key = super::encode_dedupe_key(key);
        let now_ms = Self::now_ms();

        match self.partition.get(&encoded_key) {
            Ok(Some(value_bytes)) => super::decode_dedupe_entry(&value_bytes)
                .map(|entry| Ok(!entry.is_expired(now_ms)))
                .unwrap_or(Ok(false)),
            Ok(None) => Ok(false),
            Err(e) => Err(DedupeStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_keyspace() -> fjall::Keyspace {
        let dir = tempdir().unwrap();
        fjall::Config::new(dir.path()).open().unwrap()
    }

    fn sample_instance_id() -> InstanceId {
        InstanceId::from_bytes([1u8; 16])
    }

    #[test]
    fn fjall_dedupe_store_check_and_insert_returns_admitted_for_new_key() {
        let keyspace = create_test_keyspace();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let key = DedupeKey::parse("new-key").unwrap();

        let result = store.check_and_insert(&key, &sample_instance_id(), 5000);

        assert_eq!(result, Ok(AdmissionResult::Admitted));
    }

    #[test]
    fn fjall_dedupe_store_check_and_insert_returns_duplicate_for_existing_key() {
        let keyspace = create_test_keyspace();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let key = DedupeKey::parse("dup-key").unwrap();

        store
            .check_and_insert(&key, &sample_instance_id(), 5000)
            .unwrap();
        let result = store.check_and_insert(&key, &sample_instance_id(), 5000);

        assert!(matches!(result, Ok(AdmissionResult::Duplicate { .. })));
    }

    #[test]
    fn fjall_dedupe_store_check_and_insert_returns_error_for_zero_ttl() {
        let keyspace = create_test_keyspace();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let key = DedupeKey::parse("ttl-key").unwrap();

        let result = store.check_and_insert(&key, &sample_instance_id(), 0);

        assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
    }

    #[test]
    fn fjall_dedupe_store_contains_returns_true_for_existing_unexpired_key() {
        let keyspace = create_test_keyspace();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let key = DedupeKey::parse("contains-key").unwrap();

        store
            .check_and_insert(&key, &sample_instance_id(), 99999)
            .unwrap();

        assert_eq!(store.contains(&key), Ok(true));
    }

    #[test]
    fn fjall_dedupe_store_contains_returns_false_for_missing_key() {
        let keyspace = create_test_keyspace();
        let store = FjallDedupeStore::open(&keyspace).unwrap();
        let key = DedupeKey::parse("missing-key").unwrap();

        assert_eq!(store.contains(&key), Ok(false));
    }

    #[test]
    fn fjall_dedupe_store_striped_lock_prevents_double_admit() {
        let keyspace = create_test_keyspace();
        let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
        let key = DedupeKey::parse("striped-atomic-key").unwrap();
        let num_threads = 8usize;
        let barrier = Arc::new(std::sync::Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let store = Arc::clone(&store);
                let key = key.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let iid = InstanceId::from_bytes([i as u8; 16]);
                    store.check_and_insert(&key, &iid, 60_000)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let admitted_count = results
            .iter()
            .filter(|r| matches!(r, Ok(AdmissionResult::Admitted)))
            .count();
        let dup_count = results
            .iter()
            .filter(|r| matches!(r, Ok(AdmissionResult::Duplicate { .. })))
            .count();

        assert_eq!(
            admitted_count, 1,
            "Striped lock must enforce exactly one winner"
        );
        assert_eq!(dup_count, num_threads - 1);
    }
}
