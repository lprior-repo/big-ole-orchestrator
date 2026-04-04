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
// Tests: check_stale_fence() behavior
// ---------------------------------------------------------------------------

#[test]
fn check_stale_fence_returns_true_when_current_token_differs() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let supplied = fence_token(lease.token().inner().get() + 1);

    assert_eq!(
        store.check_stale_fence(&sample_instance_id(), &sample_step_id(), &supplied),
        Ok(true)
    );
}

#[test]
fn check_stale_fence_returns_true_when_supplied_token_is_lower_than_current() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.set_time(1);
    let _current = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        store.check_stale_fence(&sample_instance_id(), &sample_step_id(), first.token()),
        Ok(true)
    );
}

#[test]
fn check_stale_fence_returns_false_when_current_token_matches() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        store.check_stale_fence(&sample_instance_id(), &sample_step_id(), lease.token()),
        Ok(false)
    );
}

#[test]
fn check_stale_fence_returns_false_when_pair_absent() {
    let store = DeterministicLeaseStore::new();

    assert_eq!(
        store.check_stale_fence(&sample_instance_id(), &sample_step_id(), &fence_token(1)),
        Ok(false)
    );
}

#[test]
fn check_stale_fence_is_observational() {
    let store = DeterministicLeaseStore::new();
    let current = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let different = fence_token(current.token().inner().get() + 1);

    assert_eq!(
        (
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *current.token()
            ),
            stale_result(&store, &sample_instance_id(), &sample_step_id(), different),
            store.release(&current),
        ),
        (false, true, Ok(()))
    );
}

fn verify_check_stale_fence_observational_across_expiry(
    before_match: bool,
    before_mismatch: bool,
    reacquired: LeaseRecord,
    current: LeaseRecord,
    store: &DeterministicLeaseStore,
) {
    assert_eq!(before_match, false);
    assert_eq!(before_mismatch, true);
    assert_eq!(reacquired.token().inner().get(), 2);
    assert!(stale_result(
        store,
        &sample_instance_id(),
        &sample_step_id(),
        *current.token()
    ));
    assert!(!stale_result(
        store,
        &sample_instance_id(),
        &sample_step_id(),
        *reacquired.token()
    ));
}

fn setup_stale_fence_across_expiry(
    store: &DeterministicLeaseStore,
) -> (bool, bool, LeaseRecord, LeaseRecord) {
    let current = acquire_lease(store, &sample_instance_id(), &sample_step_id(), 1);
    let different = fence_token(2);
    let before_match = stale_result(
        store,
        &sample_instance_id(),
        &sample_step_id(),
        *current.token(),
    );
    let before_mismatch = stale_result(store, &sample_instance_id(), &sample_step_id(), different);
    store.set_time(1);
    let reacquired = acquire_lease(store, &sample_instance_id(), &sample_step_id(), 5_000);
    (before_match, before_mismatch, reacquired, current)
}

#[test]
fn check_stale_fence_is_observational_across_expiry_boundary() {
    let store = DeterministicLeaseStore::new();
    let (before_match, before_mismatch, reacquired, current) =
        setup_stale_fence_across_expiry(&store);
    verify_check_stale_fence_observational_across_expiry(
        before_match,
        before_mismatch,
        reacquired,
        current,
        &store,
    );
}

#[test]
fn check_stale_fence_returns_storage_error_when_lookup_fails() {
    let store = DeterministicLeaseStore::new();
    store.set_faults(FaultConfig {
        stale_lookup: Some("stale lookup failed".to_string()),
        ..FaultConfig::default()
    });

    assert_eq!(
        store.check_stale_fence(&sample_instance_id(), &sample_step_id(), &fence_token(1)),
        Err(LeaseStoreError::Storage {
            reason: "stale lookup failed".to_string(),
        })
    );
}
