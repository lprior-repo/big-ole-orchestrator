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
// Tests: LeaseAlreadyHeld does not extend expiry
// ---------------------------------------------------------------------------

fn verify_acquire_lease_already_held_token_and_staleness(
    store: &DeterministicLeaseStore,
    first: &LeaseRecord,
    second: &LeaseRecord,
) {
    assert_eq!(second.token().inner().get(), 2);
    assert!(stale_result(
        store,
        &sample_instance_id(),
        &sample_step_id(),
        *first.token()
    ));
    assert!(!stale_result(
        store,
        &sample_instance_id(),
        &sample_step_id(),
        *second.token()
    ));
}

fn verify_acquire_lease_already_held_does_not_extend(
    store: &DeterministicLeaseStore,
    first: LeaseRecord,
    retry_before_expiry: Result<LeaseRecord, LeaseStoreError>,
    second: LeaseRecord,
) {
    assert_eq!(
        retry_before_expiry,
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
    verify_acquire_lease_already_held_token_and_staleness(store, &first, &second);
    assert_eq!(store.release(&second), Ok(()));
}

#[test]
fn acquire_lease_already_held_does_not_extend_original_expiry() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    let retry_before_expiry = store.acquire(&sample_instance_id(), &sample_step_id(), 1);
    store.set_time(1);
    let second = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    verify_acquire_lease_already_held_does_not_extend(&store, first, retry_before_expiry, second);
}

// ---------------------------------------------------------------------------
// Tests: Independent lease pairs
// ---------------------------------------------------------------------------

fn verify_pair_remains_releasable(
    store: &DeterministicLeaseStore,
    lease: &LeaseRecord,
    instance_id: &InstanceId,
    step_id: &StepId,
) {
    assert_eq!(
        (
            stale_result(store, instance_id, step_id, *lease.token()),
            store.release(lease),
        ),
        (false, Ok(()))
    );
}

#[test]
fn acquire_on_one_pair_leaves_original_pair_current_and_releasable_when_different_pair_acquires() {
    let store = DeterministicLeaseStore::new();
    let lease_a = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let lease_b = acquire_lease(
        &store,
        &alternate_instance_id(),
        &alternate_step_id(),
        5_000,
    );
    verify_pair_remains_releasable(&store, &lease_a, &sample_instance_id(), &sample_step_id());
    verify_pair_remains_releasable(
        &store,
        &lease_b,
        &alternate_instance_id(),
        &alternate_step_id(),
    );
}

// ---------------------------------------------------------------------------
// Tests: Storage fault injection
// ---------------------------------------------------------------------------

#[test]
fn acquire_returns_storage_error_when_lookup_fails() {
    let store = DeterministicLeaseStore::new();
    store.set_faults(FaultConfig {
        acquire_lookup: Some("lookup failed".to_string()),
        ..FaultConfig::default()
    });

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::Storage {
            reason: "lookup failed".to_string(),
        })
    );
}

#[test]
fn acquire_returns_storage_error_when_persist_fails() {
    let store = DeterministicLeaseStore::new();
    store.set_faults(FaultConfig {
        acquire_persist: Some("persist failed".to_string()),
        ..FaultConfig::default()
    });

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::Storage {
            reason: "persist failed".to_string(),
        })
    );
}

// ---------------------------------------------------------------------------
// Tests: Stale fence release does not extend expiry
// ---------------------------------------------------------------------------

fn verify_release_stale_fence_token_and_staleness(
    store: &DeterministicLeaseStore,
    current: &LeaseRecord,
    reacquired: &LeaseRecord,
) {
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

fn verify_release_stale_fence_does_not_extend(
    store: &DeterministicLeaseStore,
    current: LeaseRecord,
    stale_release: Result<(), LeaseStoreError>,
    reacquired: LeaseRecord,
) {
    assert_eq!(
        stale_release,
        Err(LeaseStoreError::StaleFence {
            expected: "1".to_string(),
            actual: "2".to_string(),
        })
    );
    verify_release_stale_fence_token_and_staleness(store, &current, &reacquired);
    assert_eq!(store.release(&reacquired), Ok(()));
}

#[test]
fn release_stale_fence_does_not_extend_current_lease_expiry() {
    let store = DeterministicLeaseStore::new();
    let current = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    let stale = LeaseRecord::new(sample_instance_id(), sample_step_id(), fence_token(2));
    let stale_release = store.release(&stale);
    store.set_time(1);
    let reacquired = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    verify_release_stale_fence_does_not_extend(&store, current, stale_release, reacquired);
}

/// AQ-06: Release after lease expired and pair re-acquired returns StaleFence.
#[test]
fn release_fails_when_lease_expired_and_pair_reacquired() {
    let store = DeterministicLeaseStore::new();

    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.set_time(1);
    let _new_owner = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::StaleFence {
            expected: "2".to_string(),
            actual: "1".to_string(),
        })
    );
}

/// AQ-07: Crash recovery retry cycles advance fence without explicit releases.
#[test]
fn crash_recovery_retry_cycles_advance_fence_without_release() {
    let store = DeterministicLeaseStore::new();
    let mut tokens: Vec<u64> = Vec::new();

    for i in 0..15u64 {
        let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
        tokens.push(lease.token().inner().get());
        store.set_time((i + 1) * 2);
    }

    for window in tokens.windows(2) {
<<<<<<< HEAD
        assert!(
            window[1] > window[0],
            "token {} not > {}",
            window[1],
            window[0]
        );
=======
        assert!(window[1] > window[0], "token {} not > {}", window[1], window[0]);
>>>>>>> origin/polecat/synth-mnw6kj8v
    }

    assert!(stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        fence_token(tokens[0])
    ));
    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        fence_token(*tokens.last().unwrap())
    ));
}

/// AQ-08: u64::MAX TTL blocks reacquire until time reaches u64::MAX.
#[test]
fn near_infinite_lease_blocks_reacquire_until_far_future() {
    let store = DeterministicLeaseStore::new();
    let _immortal = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), u64::MAX);

    store.set_time(u64::MAX - 1);
    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );

    store.set_time(u64::MAX);
    let renewed = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(renewed.token().inner().get(), 2);
}

// ---------------------------------------------------------------------------
// Tests: Fence token exhaustion
// ---------------------------------------------------------------------------

#[test]
fn acquire_returns_fence_token_exhausted_after_u64_max_release() {
    let store = DeterministicLeaseStore::new();
    let key = DeterministicLeaseStore::key(&sample_instance_id(), &sample_step_id());
    store
        .next_fence_by_key
        .borrow_mut()
        .insert(key, FenceAllocatorState::Next(u64::MAX));

    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    assert_eq!(first.token().inner().get(), u64::MAX);
    assert_eq!(store.release(&first), Ok(()));

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 1),
        Err(LeaseStoreError::FenceTokenExhausted {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

#[test]
fn acquire_returns_fence_token_exhausted_after_u64_max_expiry() {
    let store = DeterministicLeaseStore::new();
    let key = DeterministicLeaseStore::key(&sample_instance_id(), &sample_step_id());
    store
        .next_fence_by_key
        .borrow_mut()
        .insert(key, FenceAllocatorState::Next(u64::MAX));

    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    assert_eq!(first.token().inner().get(), u64::MAX);
    store.set_time(1);

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 1),
        Err(LeaseStoreError::FenceTokenExhausted {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

/// AQ-16: Fence exhaustion persists after lease expiry.
#[test]
fn fence_exhaustion_persists_after_lease_expiry() {
    let store = DeterministicLeaseStore::new();
    let key = DeterministicLeaseStore::key(&sample_instance_id(), &sample_step_id());
    store
        .next_fence_by_key
        .borrow_mut()
        .insert(key, FenceAllocatorState::Next(u64::MAX));

    let max_lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    assert_eq!(max_lease.token().inner().get(), u64::MAX);
    store.release(&max_lease).unwrap();

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::FenceTokenExhausted {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );

    store.set_time(1);
    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::FenceTokenExhausted {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

/// AQ-20: Saturating TTL arithmetic at u64::MAX boundaries does not panic.
#[test]
fn saturating_ttl_arithmetic_does_not_panic_at_u64_boundaries() {
    let store = DeterministicLeaseStore::new();
    store.set_time(u64::MAX - 1);

    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 2);

    // expires_at = u64::MAX - 1 + 2 = u64::MAX (saturating)
    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );

    store.set_time(u64::MAX);
    let _renewed = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert!(stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *lease.token()
    ));
}

/// AQ-21: Fence exhaustion on one pair does not affect other pairs.
#[test]
fn fence_exhaustion_is_per_pair_not_global() {
    let store = DeterministicLeaseStore::new();
    let iid_b = alternate_instance_id();
    let sid_b = alternate_step_id();

    let key_a = DeterministicLeaseStore::key(&sample_instance_id(), &sample_step_id());
    store
        .next_fence_by_key
        .borrow_mut()
        .insert(key_a, FenceAllocatorState::Next(u64::MAX));

    let max_lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.release(&max_lease).unwrap();

    // Pair A is exhausted
<<<<<<< HEAD
    assert!(store
        .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
        .is_err());
=======
    assert!(store.acquire(&sample_instance_id(), &sample_step_id(), 5_000).is_err());
>>>>>>> origin/polecat/synth-mnw6kj8v

    // Pair B is unaffected — gets token 1
    let lease_b = acquire_lease(&store, &iid_b, &sid_b, 5_000);
    assert_eq!(lease_b.token().inner().get(), 1);
}
