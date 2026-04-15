//! In-memory implementation of `LeaseStore` for testing and development.

use std::collections::HashMap;
use std::sync::Mutex;

use vo_types::{FenceToken, InstanceId, LeaseRecord, StepId};

use super::{LeaseEntry, LeaseStore, LeaseStoreError};

/// In-memory implementation of `LeaseStore` for testing and development.
#[derive(Debug, Default)]
pub struct InMemoryLeaseStore {
    leases: Mutex<HashMap<String, LeaseEntry>>,
    fences: Mutex<HashMap<String, u64>>,
}

impl InMemoryLeaseStore {
    /// Creates a new empty `InMemoryLeaseStore`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            leases: Mutex::new(HashMap::new()),
            fences: Mutex::new(HashMap::new()),
        }
    }

    fn lease_key(instance_id: &InstanceId, step_id: &StepId) -> String {
        format!("{instance_id}::{step_id}")
    }

    fn fence_key(instance_id: &InstanceId, step_id: &StepId) -> String {
        format!("{instance_id}::{step_id}::fence")
    }

    #[allow(clippy::cast_possible_truncation, clippy::expect_used)]
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after UNIX epoch")
            .as_millis() as u64
    }
}

impl LeaseStore for InMemoryLeaseStore {
    fn acquire(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        ttl_ms: u64,
    ) -> Result<LeaseRecord, LeaseStoreError> {
        if ttl_ms == 0 {
            return Err(LeaseStoreError::InvalidArgument);
        }

        let now_ms = Self::now_ms();
        let expires_at = now_ms.saturating_add(ttl_ms);
        let key = Self::lease_key(instance_id, step_id);

        let mut leases = self.leases.lock().map_err(|e| LeaseStoreError::Storage {
            reason: e.to_string(),
        })?;
        let mut fences = self.fences.lock().map_err(|e| LeaseStoreError::Storage {
            reason: e.to_string(),
        })?;

        if let Some(entry) = leases.get(&key) {
            if !entry.is_expired(now_ms) {
                drop(leases);
                drop(fences);
                return Err(LeaseStoreError::LeaseAlreadyHeld {
                    instance_id: instance_id.to_string(),
                    step_id: step_id.to_string(),
                });
            }
        }

        let fence_key = Self::fence_key(instance_id, step_id);
        let current_token = fences.get(&fence_key).copied().unwrap_or(0);
        let next_token =
            current_token
                .checked_add(1)
                .ok_or_else(|| LeaseStoreError::FenceTokenExhausted {
                    instance_id: instance_id.to_string(),
                    step_id: step_id.to_string(),
                })?;

        fences.insert(fence_key, next_token);

        let entry = LeaseEntry::new(
            instance_id.to_string(),
            step_id.to_string(),
            next_token,
            expires_at,
        )?;
        leases.insert(key, entry.clone());
        drop(leases);
        drop(fences);

        entry.to_lease_record()
    }

    fn release(&self, lease: &LeaseRecord) -> Result<(), LeaseStoreError> {
        let mut leases = self.leases.lock().map_err(|e| LeaseStoreError::Storage {
            reason: e.to_string(),
        })?;

        let key = Self::lease_key(lease.instance_id(), lease.step_id());
        let entry = leases.get(&key).ok_or_else(|| LeaseStoreError::NotFound {
            instance_id: lease.instance_id().to_string(),
            step_id: lease.step_id().to_string(),
        })?;

        let expected = entry.fence_token().to_string();
        if entry.fence_token() != lease.token().inner().get() {
            drop(leases);
            return Err(LeaseStoreError::StaleFence {
                expected,
                actual: lease.token().inner().get().to_string(),
            });
        }

        leases.remove(&key);
        drop(leases);
        Ok(())
    }

    fn check_stale_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        token: &FenceToken,
    ) -> Result<bool, LeaseStoreError> {
        let now_ms = Self::now_ms();
        let key = Self::lease_key(instance_id, step_id);

        let leases = self.leases.lock().map_err(|e| LeaseStoreError::Storage {
            reason: e.to_string(),
        })?;

        let result = leases.get(&key).is_some_and(|entry| {
            !entry.is_expired(now_ms) && entry.fence_token() != token.inner().get()
        });
        drop(leases);
        Ok(result)
    }
}
