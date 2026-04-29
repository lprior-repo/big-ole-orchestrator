use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Test harness: FaultConfig
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
// Test harness: DeterministicLeaseStore
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
// Tests: acquire() behavior
// ---------------------------------------------------------------------------

#[test]
fn acquire_returns_invalid_argument_when_ttl_zero() {
    let store = DeterministicLeaseStore::new();

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 0),
        Err(LeaseStoreError::InvalidArgument)
    );
}

#[test]
fn acquire_establishes_authoritative_lease_when_pair_absent() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        (
            lease.instance_id().to_string(),
            lease.step_id().to_string(),
            lease.token().inner().get(),
            stale_result(&store, lease.instance_id(), lease.step_id(), *lease.token()),
            store.release(&lease),
        ),
        (
            sample_instance_id().to_string(),
            sample_step_id().to_string(),
            1,
            false,
            Ok(()),
        )
    );
}

#[test]
fn acquire_establishes_authoritative_lease_when_ttl_is_one() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);

    assert_eq!(
        (
            lease.instance_id().to_string(),
            lease.step_id().to_string(),
            lease.token().inner().get(),
        ),
        (
            sample_instance_id().to_string(),
            sample_step_id().to_string(),
            1,
        )
    );
}

#[test]
fn acquire_accepts_u64_max_ttl_when_pair_absent() {
    let store = DeterministicLeaseStore::new();
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), u64::MAX);

    assert!(!stale_result(
        &store,
        lease.instance_id(),
        lease.step_id(),
        *lease.token()
    ));
}

#[test]
fn acquire_returns_lease_already_held_when_unexpired_lease_exists() {
    let store = DeterministicLeaseStore::new();
    let _first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        store.acquire(&sample_instance_id(), &sample_step_id(), 5_000),
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

#[test]
fn acquire_does_not_replace_current_lease_when_lease_already_held() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let retry = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        (
            retry,
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *first.token()
            ),
            store.release(&first),
        ),
        (
            Err(LeaseStoreError::LeaseAlreadyHeld {
                instance_id: sample_instance_id().to_string(),
                step_id: sample_step_id().to_string(),
            }),
            false,
            Ok(()),
        )
    );
}

#[test]
fn acquire_returns_new_authoritative_lease_when_now_equals_expiry() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.set_time(1);
    let second = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        (
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
        (2, true, false)
    );
}

#[test]
fn acquire_returns_new_authoritative_lease_when_now_greater_than_expiry() {
    let store = DeterministicLeaseStore::new();
    let first = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.set_time(2);
    let second = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        (
            second.token().inner().get(),
            stale_result(
                &store,
                &sample_instance_id(),
                &sample_step_id(),
                *first.token()
            ),
        ),
        (2, true)
    );
}

/// AQ-09: Simulated concurrent acquisition on same pair — first acquirer wins,
/// second gets LeaseAlreadyHeld.
#[test]
fn concurrent_acquire_on_same_pair_first_writer_wins() {
    let store = DeterministicLeaseStore::new();

    let first = store
        .acquire(&sample_instance_id(), &sample_step_id(), 5_000)
        .unwrap();

    let second = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(
        second,
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );

    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *first.token()
    ));
}

/// AQ-10: Interleaved acquisition on different pairs — fence tokens don't cross-contaminate.
#[test]
fn interleaved_acquisition_on_different_pairs_is_independent() {
    let store = DeterministicLeaseStore::new();

    let lease_a1 = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let lease_b1 = acquire_lease(
        &store,
        &alternate_instance_id(),
        &alternate_step_id(),
        5_000,
    );
    let lease_a2_result = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);
    let lease_b2_result = store.acquire(&alternate_instance_id(), &alternate_step_id(), 5_000);

    assert!(lease_a2_result.is_err());
    assert!(lease_b2_result.is_err());

    assert_eq!(store.release(&lease_a1), Ok(()));
    assert!(lease_b2_result.is_err());

    let lease_a3 = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(lease_a3.token().inner().get(), 2);
    assert_eq!(lease_b1.token().inner().get(), 1);

    assert!(!stale_result(
        &store,
        &alternate_instance_id(),
        &alternate_step_id(),
        *lease_b1.token()
    ));
    assert!(stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *lease_a1.token()
    ));
}

/// AQ-11: Failed acquire due to transient fault has no side effects; retry succeeds.
#[test]
fn failed_acquire_has_no_side_effects_retry_succeeds() {
    let store = DeterministicLeaseStore::new();

    store.set_faults(FaultConfig {
        acquire_lookup: Some("transient error".to_string()),
        ..FaultConfig::default()
    });
    let result = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(
        result,
        Err(LeaseStoreError::Storage {
            reason: "transient error".to_string(),
        })
    );

    store.set_faults(FaultConfig::default());
    let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    assert_eq!(lease.token().inner().get(), 1);
}

/// AQ-18: Double-recovery race — first writer wins, second gets LeaseAlreadyHeld.
#[test]
fn double_recovery_race_first_writer_wins() {
    let store = DeterministicLeaseStore::new();

    let _original = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);
    store.set_time(1);

    let recovery_1 = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 5_000);
    let recovery_2 = store.acquire(&sample_instance_id(), &sample_step_id(), 5_000);

    assert_eq!(
        recovery_2,
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
    assert_eq!(recovery_1.token().inner().get(), 2);
    assert!(!stale_result(
        &store,
        &sample_instance_id(),
        &sample_step_id(),
        *recovery_1.token()
    ));
}

/// AQ-23: Each successful acquire returns a strictly increasing fence token.
#[test]
fn fence_token_is_strictly_monotonic_across_successive_acquires() {
    let store = DeterministicLeaseStore::new();
    let mut prev_token: u64 = 0;

    for cycle in 0..100u64 {
        let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);

        let current_token = lease.token().inner().get();
        assert!(
            current_token > prev_token,
            "token {} not strictly greater than {} in cycle {cycle}",
            current_token,
            prev_token
        );
        prev_token = current_token;

        store.set_time(cycle.saturating_add(1));
    }

    assert_eq!(prev_token, 100);
}

/// AQ-24: Fence tokens are strictly monotonic across alternating release and expiry cycles.
#[test]
fn fence_token_monotonic_across_mixed_release_and_expiry_cycles() {
    let store = DeterministicLeaseStore::new();
    let mut prev_token: u64 = 0;

    for cycle in 0..50u64 {
        let lease = acquire_lease(&store, &sample_instance_id(), &sample_step_id(), 1);

        let current_token = lease.token().inner().get();
        assert!(
            current_token > prev_token,
            "token {} not strictly greater than {} in cycle {cycle}",
            current_token,
            prev_token
        );
        prev_token = current_token;

        if cycle % 2 == 0 {
            store.set_time(cycle + 1);
        } else {
            store.release(&lease).unwrap();
        }
    }
}
