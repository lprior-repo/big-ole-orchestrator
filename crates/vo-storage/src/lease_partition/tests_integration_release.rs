use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Test harness: FaultConfig (shared)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FaultConfig {
    acquire_lookup: Option<String>,
    acquire_persist: Option<String>,
    release_lookup: Option<String>,
    release_delete: Option<String>,
    stale_lookup: Option<String>,
}

// ---------------------------------------------------------------------------
// Test harness: DeterministicLeaseStore (shared)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum FenceAllocatorState {
    Next(u64),
    Exhausted,
}

struct DeterministicLeaseStore {
    leases: RefCell<HashMap<String, LeaseEntry>>,
    next_fence_by_key: RefCell<HashMap<String, FenceAllocatorState>>,
    now_ms: Cell<u64>,
    faults: RefCell<FaultConfig>,
}

impl DeterministicLeaseStore {
    fn new() -> Self {
        Self {
            leases: RefCell::new(HashMap::new()),
            next_fence_by_key: RefCell::new(HashMap::new()),
            now_ms: Cell::new(0),
            faults: RefCell::new(FaultConfig::default()),
        }
    }

    fn set_time(&self, now_ms: u64) {
        self.now_ms.set(now_ms);
    }

    fn set_faults(&self, faults: FaultConfig) {
        *self.faults.borrow_mut() = faults;
    }

    fn key(instance_id: &InstanceId, step_id: &StepId) -> String {
        format!("{instance_id}::{step_id}")
    }

    fn cloned_lease(&self, key: &str) -> Option<LeaseEntry> {
        self.leases.borrow().get(key).cloned()
    }

    fn lookup_fault(reason: Option<String>) -> Result<(), LeaseStoreError> {
        reason.map_or(Ok(()), |reason| Err(LeaseStoreError::Storage { reason }))
    }

    fn ensure_pair_acquirable(
        existing: Option<&LeaseEntry>,
        now_ms: u64,
        instance_id: &InstanceId,
        step_id: &StepId,
    ) -> Result<(), LeaseStoreError> {
        existing
            .filter(|entry| !entry.is_expired(now_ms))
            .map_or(Ok(()), |_| {
                Err(LeaseStoreError::LeaseAlreadyHeld {
                    instance_id: instance_id.to_string(),
                    step_id: step_id.to_string(),
                })
            })
    }

    fn allocate_next_fence(
        &self,
        key: &str,
        instance_id: &InstanceId,
        step_id: &StepId,
    ) -> Result<u64, LeaseStoreError> {
        let next_state = self
            .next_fence_by_key
            .borrow()
            .get(key)
            .cloned()
            .unwrap_or(FenceAllocatorState::Next(1));

        match next_state {
            FenceAllocatorState::Next(token) => {
                let updated = if token == u64::MAX {
                    FenceAllocatorState::Exhausted
                } else {
                    FenceAllocatorState::Next(token + 1)
                };
                self.next_fence_by_key
                    .borrow_mut()
                    .insert(key.to_string(), updated);
                Ok(token)
            }
            FenceAllocatorState::Exhausted => Err(LeaseStoreError::FenceTokenExhausted {
                instance_id: instance_id.to_string(),
                step_id: step_id.to_string(),
            }),
        }
    }

    fn current_lease_or_not_found(
        &self,
        lease: &LeaseRecord,
        key: &str,
    ) -> Result<LeaseEntry, LeaseStoreError> {
        self.cloned_lease(key)
            .ok_or_else(|| LeaseStoreError::NotFound {
                instance_id: lease.instance_id().to_string(),
                step_id: lease.step_id().to_string(),
            })
    }

    fn ensure_matching_fence(existing: &LeaseEntry, actual: u64) -> Result<(), LeaseStoreError> {
        let expected = existing.fence_token();
        if expected == actual {
            Ok(())
        } else {
            Err(LeaseStoreError::StaleFence {
                expected: expected.to_string(),
                actual: actual.to_string(),
            })
        }
    }

    fn acquire_entry(
        &self,
        key: &str,
        ctx: AcquireContext<'_>,
    ) -> Result<LeaseEntry, LeaseStoreError> {
        Self::lookup_fault(ctx.faults.acquire_lookup.clone())?;
        let existing = self.cloned_lease(key);
        Self::ensure_pair_acquirable(existing.as_ref(), ctx.now_ms, ctx.instance_id, ctx.step_id)?;
        Self::lookup_fault(ctx.faults.acquire_persist.clone())?;
        let next_fence = self.allocate_next_fence(key, ctx.instance_id, ctx.step_id)?;
        LeaseEntry::new(
            ctx.instance_id.to_string(),
            ctx.step_id.to_string(),
            next_fence,
            ctx.now_ms.saturating_add(ctx.ttl_ms),
        )
    }
}

impl LeaseStore for DeterministicLeaseStore {
    fn acquire(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        ttl_ms: u64,
    ) -> Result<LeaseRecord, LeaseStoreError> {
        if ttl_ms == 0 {
            return Err(LeaseStoreError::InvalidArgument);
        }

        let key = Self::key(instance_id, step_id);
        let now_ms = self.now_ms.get();
        let faults = self.faults.borrow().clone();
        let ctx = AcquireContext {
            instance_id,
            step_id,
            ttl_ms,
            now_ms,
            faults: &faults,
        };
        let entry = self.acquire_entry(&key, ctx)?;
        let record = entry.to_lease_record()?;
        self.leases.borrow_mut().insert(key, entry);
        Ok(record)
    }

    fn release(&self, lease: &LeaseRecord) -> Result<(), LeaseStoreError> {
        let key = Self::key(lease.instance_id(), lease.step_id());
        let faults = self.faults.borrow().clone();

        Self::lookup_fault(faults.release_lookup)?;
        let existing = self.current_lease_or_not_found(lease, &key)?;
        Self::ensure_matching_fence(&existing, lease.token().inner().get())?;
        Self::lookup_fault(faults.release_delete)?;

        self.leases.borrow_mut().remove(&key);
        Ok(())
    }

    fn check_stale_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        token: &FenceToken,
    ) -> Result<bool, LeaseStoreError> {
        if let Some(reason) = self.faults.borrow().stale_lookup.clone() {
            return Err(LeaseStoreError::Storage { reason });
        }

        let key = Self::key(instance_id, step_id);
        let leases = self.leases.borrow();
        let existing = leases.get(&key).cloned();
        drop(leases);

        Ok(existing.is_some_and(|entry| entry.fence_token() != token.inner().get()))
    }
}

struct AcquireContext<'a> {
    instance_id: &'a InstanceId,
    step_id: &'a StepId,
    ttl_ms: u64,
    now_ms: u64,
    faults: &'a FaultConfig,
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn sample_instance_id() -> InstanceId {
    parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV")
}

fn sample_step_id() -> StepId {
    parse_step_id("step-1")
}

fn alternate_instance_id() -> InstanceId {
    parse_instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA")
}

fn alternate_step_id() -> StepId {
    parse_step_id("step-b")
}

fn fence_token(value: u64) -> FenceToken {
    FenceToken::new(value).unwrap()
}

fn parse_instance_id(raw: &str) -> InstanceId {
    InstanceId::parse(raw).unwrap()
}

fn parse_step_id(raw: &str) -> StepId {
    StepId::parse(raw).unwrap()
}

fn acquire_lease(
    store: &DeterministicLeaseStore,
    instance_id: &InstanceId,
    step_id: &StepId,
    ttl_ms: u64,
) -> LeaseRecord {
    store.acquire(instance_id, step_id, ttl_ms).unwrap()
}

fn stale_result(
    store: &DeterministicLeaseStore,
    instance_id: &InstanceId,
    step_id: &StepId,
    token: FenceToken,
) -> bool {
    store
        .check_stale_fence(instance_id, step_id, &token)
        .unwrap()
}

// ---------------------------------------------------------------------------
// Tests: release() behavior
// ---------------------------------------------------------------------------

#[test]
fn release_returns_not_found_on_second_release_when_first_release_deleted_current_lease() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        (store.release(&lease), store.release(&lease)),
        (
            Ok(()),
            Err(LeaseStoreError::NotFound {
                instance_id: sample_instance_id().to_string(),
                step_id: sample_step_id().to_string(),
            }),
        )
    );
}

#[test]
fn release_allows_reacquire_with_next_token_when_current_lease_was_deleted() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let released = store.release(&first);
    let second = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        (
            released,
            second.token().inner().get(),
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *first.token()
            ),
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *second.token()
            ),
        ),
        (Ok(()), 2, true, false)
    );
}

#[test]
fn release_returns_not_found_when_pair_absent() {
    let store = DeterministicLeaseStore::new();
    let lease = LeaseRecord::new(sample_instance_id(), sample_step_id(), fence_token(1));

    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::NotFound {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

#[test]
fn release_not_found_leaves_pair_acquirable() {
    let store = DeterministicLeaseStore::new();
    let lease = LeaseRecord::new(sample_instance_id(), sample_step_id(), fence_token(1));
    let not_found = store.release(&lease);
    let acquired = acquire_lease(&store, lease.instance_id(), lease.step_id(), 5_000);

    assert_eq!(
        (not_found, acquired.token().inner().get()),
        (
            Err(LeaseStoreError::NotFound {
                instance_id: sample_instance_id().to_string(),
                step_id: sample_step_id().to_string(),
            }),
            1,
        )
    );
}

#[test]
fn release_returns_stale_fence_when_token_mismatches_current() {
    let store = DeterministicLeaseStore::new();
    let current = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let stale = LeaseRecord::new(sample_instance_id(), sample_step_id(), fence_token(2));

    assert_eq!(
        store.release(&stale),
        Err(LeaseStoreError::StaleFence {
            expected: current.token().inner().get().to_string(),
            actual: "2".to_string(),
        })
    );
}

#[test]
fn release_returns_stale_fence_when_token_is_lower_than_current() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.set_time(1);
    let _current = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        store.release(&first),
        Err(LeaseStoreError::StaleFence {
            expected: "2".to_string(),
            actual: "1".to_string(),
        })
    );
}

#[test]
fn release_stale_fence_leaves_current_lease_intact() {
    let store = DeterministicLeaseStore::new();
    let current = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let stale = LeaseRecord::new(sample_instance_id(), sample_step_id(), fence_token(2));

    assert_eq!(
        (
            store.release(&stale),
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *current.token()
            ),
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *stale.token()
            ),
            store.release(&current),
        ),
        (
            Err(LeaseStoreError::StaleFence {
                expected: "1".to_string(),
                actual: "2".to_string(),
            }),
            false,
            true,
            Ok(()),
        )
    );
}

#[test]
fn release_returns_storage_error_when_lookup_fails() {
    let store = DeterministicLeaseStore::new();
    store.set_faults(FaultConfig {
        release_lookup: Some("release lookup failed".to_string()),
        ..FaultConfig::default()
    });
    let lease = LeaseRecord::new(sample_instance_id(), sample_step_id(), fence_token(1));

    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::Storage {
            reason: "release lookup failed".to_string(),
        })
    );
}

#[test]
fn release_returns_storage_error_when_delete_fails() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    store.set_faults(FaultConfig {
        release_delete: Some("delete failed".to_string()),
        ..FaultConfig::default()
    });

    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::Storage {
            reason: "delete failed".to_string(),
        })
    );
}

/// AQ-13: Release delete failure leaves lease persistent; retry succeeds.
#[test]
fn release_delete_failure_leaves_lease_persistent_retry_succeeds() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    store.set_faults(FaultConfig {
        release_delete: Some("disk full".to_string()),
        ..FaultConfig::default()
    });
    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::Storage {
            reason: "disk full".to_string(),
        })
    );

    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *lease.token()
    ));

    store.set_faults(FaultConfig::default());
    assert_eq!(store.release(&lease), Ok(()));
}

/// AQ-12: Release lookup failure leaves lease intact; retry succeeds.
#[test]
fn release_lookup_failure_does_not_corrupt_lease_state() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    store.set_faults(FaultConfig {
        release_lookup: Some("io error".to_string()),
        ..FaultConfig::default()
    });
    assert!(store.release(&lease).is_err());

    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *lease.token()
    ));

    store.set_faults(FaultConfig::default());
    assert_eq!(store.release(&lease), Ok(()));

    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::NotFound {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

/// AQ-14: Late completion from crashed holder after recovery re-acquire is rejected.
#[test]
fn late_completion_from_crashed_holder_after_recovery_is_rejected() {
    let store = DeterministicLeaseStore::new();

    let crashed = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 100);
    let crashed_fence = *crashed.token();

    store.set_time(100);
    let recovery = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(recovery.token().inner().get(), 2);

    assert_eq!(
        store.release(&LeaseRecord::new(
            sample_instance_id(),
            sample_step_id(),
            crashed_fence
        )),
        Err(LeaseStoreError::StaleFence {
            expected: "2".to_string(),
            actual: "1".to_string(),
        })
    );

    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *recovery.token()
    ));
    assert_eq!(store.release(&recovery), Ok(()));
}

/// AQ-17: Full ADR-029 lifecycle — acquire, crash, recovery re-acquire,
/// late completion rejected, recovery completion succeeds.
#[test]
fn full_adr029_lifecycle_crash_recovery_rejects_late_completion() {
    let store = DeterministicLeaseStore::new();

    let engine = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 100);
    let subprocess_fence = *engine.token();

    // Subprocess crashes, lease expires
    store.set_time(100);

    // Recovery acquires new lease
    let recovery = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(recovery.token().inner().get(), 2);

    // Late completion from crashed subprocess — STALE
    assert_eq!(
        store.release(&LeaseRecord::new(
            sample_instance_id(),
            sample_step_id(),
            subprocess_fence
        )),
        Err(LeaseStoreError::StaleFence {
            expected: "2".to_string(),
            actual: "1".to_string(),
        })
    );

    // Recovery completion succeeds
    assert_eq!(store.release(&recovery), Ok(()));
}

/// AQ-19: Timeout recovery advances fence; just-in-time subprocess completion rejected.
#[test]
fn timeout_recovery_advances_fence_rejects_just_in_time_completion() {
    let store = DeterministicLeaseStore::new();

    let original = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 50);
    store.set_time(50);

    let recovery = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        store.release(&original),
        Err(LeaseStoreError::StaleFence {
            expected: "2".to_string(),
            actual: "1".to_string(),
        })
    );

    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *recovery.token()
    ));
}
