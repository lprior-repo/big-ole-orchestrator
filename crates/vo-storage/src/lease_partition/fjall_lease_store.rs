//! Fjall-backed persistent implementation of `LeaseStore` for production use.
//!
//! Concurrency model: striped `parking_lot::Mutex` guards the acquire
//! critical section per-key-shard, preventing TOCTOU races between read and
//! insert on the same lease key while allowing independent keys to proceed
//! in parallel.

use std::sync::Arc;

use parking_lot::Mutex;
use vo_types::{FenceToken, InstanceId, LeaseRecord, StepId};

use super::{LeaseEntry, LeaseStore, LeaseStoreError, LEASE_PARTITION};

const FENCE_PARTITION: &str = "lease_fences";

const NUM_STRIPES: usize = 64;

#[expect(clippy::expect_used, clippy::cast_possible_truncation)]
fn stripe_for_key(key_bytes: &[u8]) -> usize {
    crc32fast::hash(key_bytes) as usize % NUM_STRIPES
}

pub struct FjallLeaseStore {
    lease_partition: Arc<fjall::Keyspace>,
    fence_partition: Arc<fjall::Keyspace>,
    stripes: Vec<Mutex<()>>,
}

impl FjallLeaseStore {
    /// Opens a new lease store backed by the given keyspace.
    ///
    /// # Errors
    ///
    /// Returns `LeaseStoreError::Storage` if any partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, LeaseStoreError> {
        let lease_partition = db
            .keyspace(LEASE_PARTITION, || fjall::KeyspaceCreateOptions::default())
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to open leases partition: {e}"),
            })?;
        let fence_partition = db
            .keyspace(FENCE_PARTITION, || fjall::KeyspaceCreateOptions::default())
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to open lease_fences partition: {e}"),
            })?;
        let stripes = (0..NUM_STRIPES).map(|_| Mutex::new(())).collect();
        Ok(Self {
            lease_partition: Arc::new(lease_partition),
            fence_partition: Arc::new(fence_partition),
            stripes,
        })
    }

    fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
        let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
        let step_bytes = step_id.as_str().as_bytes();
        let mut key = Vec::with_capacity(16 + 2 + step_bytes.len());
        key.extend_from_slice(&iid_bytes);
        key.extend_from_slice(&(step_bytes.len() as u16).to_be_bytes());
        key.extend_from_slice(step_bytes);
        key
    }

    fn encode_fence_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
        let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
        let step_bytes = step_id.as_str().as_bytes();
        let mut key = Vec::with_capacity(16 + 2 + step_bytes.len() + 1);
        key.extend_from_slice(&iid_bytes);
        key.extend_from_slice(&(step_bytes.len() as u16).to_be_bytes());
        key.extend_from_slice(step_bytes);
        key.push(0xFF);
        key
    }

    fn get_current_lease(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
    ) -> Result<Option<LeaseEntry>, LeaseStoreError> {
        let key = Self::encode_lease_key(instance_id, step_id);
        match self.lease_partition.get(&key) {
            Ok(Some(bytes)) => {
                let entry = super::decode_lease_entry(&bytes)?;
                Ok(Some(entry))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(LeaseStoreError::Storage {
                reason: e.to_string(),
            }),
        }
    }

    fn allocate_fence_token(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
    ) -> Result<u64, LeaseStoreError> {
        let fence_key = Self::encode_fence_key(instance_id, step_id);

        let current = match self.fence_partition.get(&fence_key) {
            Ok(Some(bytes)) => {
                let token: u64 =
                    serde_json::from_slice(&bytes).map_err(|e| LeaseStoreError::Codec {
                        reason: format!("failed to decode fence token: {e}"),
                    })?;
                token
            }
            Ok(None) => 0,
            Err(e) => {
                return Err(LeaseStoreError::Storage {
                    reason: format!("failed to read fence token: {e}"),
                })
            }
        };

        let next = current
            .checked_add(1)
            .ok_or_else(|| LeaseStoreError::FenceTokenExhausted {
                instance_id: instance_id.to_string(),
                step_id: step_id.to_string(),
            })?;

        let value = serde_json::to_vec(&next).map_err(|e| LeaseStoreError::Codec {
            reason: format!("failed to encode fence token: {e}"),
        })?;

        self.fence_partition
            .insert(&fence_key, &value)
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to persist fence token: {e}"),
            })?;

        Ok(next)
    }

    fn insert_lease(&self, entry: &LeaseEntry) -> Result<(), LeaseStoreError> {
        let key = Self::encode_lease_key(
            &InstanceId::parse(entry.instance_id()).map_err(|e| LeaseStoreError::Codec {
                reason: format!("invalid instance_id: {e}"),
            })?,
            &StepId::parse(entry.step_id()).map_err(|e| LeaseStoreError::Codec {
                reason: format!("invalid step_id: {e}"),
            })?,
        );
        let value = super::encode_lease_entry(entry)?;
        self.lease_partition
            .insert(&key, &value)
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to insert lease: {e}"),
            })
    }

    fn delete_lease(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
    ) -> Result<(), LeaseStoreError> {
        let key = Self::encode_lease_key(instance_id, step_id);
        self.lease_partition
            .remove(&key)
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to delete lease: {e}"),
            })
    }
}

impl LeaseStore for FjallLeaseStore {
    fn acquire(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        ttl_ms: u64,
    ) -> Result<LeaseRecord, LeaseStoreError> {
        if ttl_ms == 0 {
            return Err(LeaseStoreError::InvalidArgument);
        }

        let key = Self::encode_lease_key(instance_id, step_id);
        let stripe_idx = stripe_for_key(&key);
        let _guard = self.stripes[stripe_idx].lock();

        #[allow(clippy::cast_possible_truncation)]
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to get current time: {e}"),
            })?
            .as_millis();
        let now_ms = u64::try_from(now_ms).unwrap_or(u64::MAX);

        let current = self.get_current_lease(instance_id, step_id)?;
        if let Some(entry) = current {
            if !entry.is_expired(now_ms) {
                return Err(LeaseStoreError::LeaseAlreadyHeld {
                    instance_id: instance_id.to_string(),
                    step_id: step_id.to_string(),
                });
            }
        }

        let fence_token = self.allocate_fence_token(instance_id, step_id)?;

        let entry = LeaseEntry::new(
            instance_id.to_string(),
            step_id.to_string(),
            fence_token,
            now_ms.saturating_add(ttl_ms),
        )?;

        self.insert_lease(&entry)?;

        entry.to_lease_record()
    }

    fn release(&self, lease: &LeaseRecord) -> Result<(), LeaseStoreError> {
        let current = self.get_current_lease(lease.instance_id(), lease.step_id())?;
        let existing = current.ok_or_else(|| LeaseStoreError::NotFound {
            instance_id: lease.instance_id().to_string(),
            step_id: lease.step_id().to_string(),
        })?;

        if existing.fence_token() != lease.token().inner().get() {
            return Err(LeaseStoreError::StaleFence {
                expected: existing.fence_token().to_string(),
                actual: lease.token().inner().get().to_string(),
            });
        }

        self.delete_lease(lease.instance_id(), lease.step_id())
    }

    fn check_stale_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        token: &FenceToken,
    ) -> Result<bool, LeaseStoreError> {
        let current = self.get_current_lease(instance_id, step_id)?;
        Ok(current.is_some_and(|entry| entry.fence_token() != token.inner().get()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};
    use vo_types::StepId;

    fn sample_instance_id() -> InstanceId {
        InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
    }

    fn sample_step_id() -> StepId {
        StepId::parse("step-1").unwrap()
    }

    fn alternate_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    fn alternate_step_id() -> StepId {
        StepId::parse("step-b").unwrap()
    }

    fn create_test_keyspace() -> (fjall::Database, TempDir) {
        let dir = tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        (db, dir)
    }

    #[test]
    fn fjall_lease_acquire_returns_lease_record_when_pair_absent() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let result = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);
        assert!(result.is_ok());

        let lease = result.unwrap();
        assert_eq!(lease.token().inner().get(), 1);
        assert_eq!(lease.instance_id(), &sample_instance_id());
        assert_eq!(lease.step_id(), &sample_step_id());
    }

    #[test]
    fn fjall_lease_acquire_returns_lease_already_held_when_unexpired_exists() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let first = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        assert_eq!(first.token().inner().get(), 1);

        let second = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);
        assert!(matches!(
            second,
            Err(LeaseStoreError::LeaseAlreadyHeld { .. })
        ));
    }

    #[test]
    fn fjall_lease_acquire_returns_invalid_argument_when_ttl_zero() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let result = store.acquire(&sample_instance_id(), &sample_step_id(), 0);
        assert!(matches!(result, Err(LeaseStoreError::InvalidArgument)));
    }

    #[test]
    fn fjall_lease_release_succeeds_with_matching_token() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let lease = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        let result = store.release(&lease);
        assert!(result.is_ok());
    }

    #[test]
    fn fjall_lease_release_returns_not_found_when_no_lease() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let lease = LeaseRecord::new(
            sample_instance_id(),
            sample_step_id(),
            FenceToken::new(1).unwrap(),
        );
        let result = store.release(&lease);
        assert!(matches!(result, Err(LeaseStoreError::NotFound { .. })));
    }

    #[test]
    fn fjall_lease_release_returns_stale_fence_when_token_mismatches() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let _lease = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        let stale = LeaseRecord::new(
            sample_instance_id(),
            sample_step_id(),
            FenceToken::new(2).unwrap(),
        );
        let result = store.release(&stale);
        assert!(matches!(result, Err(LeaseStoreError::StaleFence { .. })));
    }

    #[test]
    fn fjall_lease_check_stale_fence_returns_false_when_token_matches() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let lease = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        let is_stale =
            store.check_stale_fence(&sample_instance_id(), &sample_step_id(), lease.token());
        assert_eq!(is_stale.unwrap(), false);
    }

    #[test]
    fn fjall_lease_check_stale_fence_returns_true_when_token_differs() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let _lease = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        let different_token = FenceToken::new(99).unwrap();
        let is_stale =
            store.check_stale_fence(&sample_instance_id(), &sample_step_id(), &different_token);
        assert_eq!(is_stale.unwrap(), true);
    }

    #[test]
    fn fjall_lease_check_stale_fence_returns_false_when_no_lease() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let token = FenceToken::new(1).unwrap();
        let is_stale = store.check_stale_fence(&sample_instance_id(), &sample_step_id(), &token);
        assert_eq!(is_stale.unwrap(), false);
    }

    #[test]
    fn fjall_lease_independent_pairs_work_independently() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let lease_a = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        let lease_b = store
            .acquire(&alternate_instance_id(), &alternate_step_id(), 5_000)
            .unwrap();

        assert_eq!(lease_a.token().inner().get(), 1);
        assert_eq!(lease_b.token().inner().get(), 1);

        assert!(store.release(&lease_a).is_ok());
        assert!(store.release(&lease_b).is_ok());
    }

    #[test]
    fn fjall_lease_acquire_increments_fence_token() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let first = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        assert_eq!(first.token().inner().get(), 1);

        store.release(&first).unwrap();

        let second = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        assert_eq!(second.token().inner().get(), 2);
    }

    #[test]
    fn fjall_lease_acquire_after_expiry_reacquires_with_higher_fence_token() {
        let (keyspace, _dir) = create_test_keyspace();
        let store = FjallLeaseStore::open(&keyspace).unwrap();

        let first = store
            .acquire(&sample_instance_id(), &sample_step_id(), 1)
            .unwrap();
        assert_eq!(first.token().inner().get(), 1);

        std::thread::sleep(std::time::Duration::from_millis(5));

        let second = store
            .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
            .unwrap();
        assert_eq!(second.token().inner().get(), 2);

        let is_stale = store.check_stale_fence(&sample_instance_id(), &sample_step_id(), first.token());
        assert_eq!(is_stale.unwrap(), true);

        let is_current = store.check_stale_fence(&sample_instance_id(), &sample_step_id(), second.token());
        assert_eq!(is_current.unwrap(), false);

        store.release(&second).unwrap();
    }
}
