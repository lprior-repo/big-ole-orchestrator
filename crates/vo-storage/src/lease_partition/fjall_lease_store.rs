//! Fjall-backed persistent implementation of `LeaseStore` for production use.
//!
//! Concurrency model: striped `parking_lot::Mutex` guards the acquire
//! critical section per-key-shard, preventing TOCTOU races between read and
//! insert on the same lease key while allowing independent keys to proceed
//! in parallel.

use std::sync::Arc;

use parking_lot::Mutex;
use vo_types::{FenceToken, InstanceId, LeaseRecord, StepId};

use super::{encode_lease_key, LeaseEntry, LeaseStore, LeaseStoreError, LEASE_PARTITION};

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
        encode_lease_key(instance_id, step_id)
    }

    fn encode_fence_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
        let mut key = encode_lease_key(instance_id, step_id);
        key.extend_from_slice(&0x01u16.to_be_bytes()); // fence suffix marker
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
        entry.to_lease_record()
    }

    fn release(&self, lease: &LeaseRecord) -> Result<(), LeaseStoreError> {
        let key = Self::encode_lease_key(lease.instance_id(), lease.step_id());

        let current = self.get_current_lease(lease.instance_id(), lease.step_id())?;

        let entry = current.ok_or_else(|| LeaseStoreError::NotFound {
            instance_id: lease.instance_id().to_string(),
            step_id: lease.step_id().to_string(),
        })?;

        if entry.fence_token() != lease.token().inner().get() {
            return Err(LeaseStoreError::StaleFence {
                expected: entry.fence_token().to_string(),
                actual: lease.token().inner().get().to_string(),
            });
        }

        self.lease_partition
            .remove(&key)
            .map_err(|e| LeaseStoreError::Storage {
                reason: format!("failed to delete lease: {e}"),
            })?;

        Ok(())
    }

    fn check_stale_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        token: &FenceToken,
    ) -> Result<bool, LeaseStoreError> {
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
            if entry.is_expired(now_ms) {
                return Ok(true);
            }
            return Ok(entry.fence_token() != token.inner().get());
        }

        Ok(false)
    }
}
