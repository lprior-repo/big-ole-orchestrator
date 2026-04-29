use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FaultConfig {
    acquire_lookup: Option<String>,
    acquire_persist: Option<String>,
    release_lookup: Option<String>,
    release_delete: Option<String>,
    stale_lookup: Option<String>,
}

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

fn parse_instance_id(raw: &str) -> InstanceId {
    InstanceId::parse(raw).unwrap()
}

fn parse_step_id(raw: &str) -> StepId {
    StepId::parse(raw).unwrap()
}

#[test]
fn fence_token_monotonically_increases_for_same_pair() {
    let store = DeterministicLeaseStore::new();
    let instance_id = parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let step_id = parse_step_id("step-1");

    let lease1 = store.acquire(&instance_id, &step_id, 5_000).unwrap();
    let lease2 = store.acquire(&instance_id, &step_id, 5_000).unwrap();
    let lease3 = store.acquire(&instance_id, &step_id, 5_000).unwrap();

    assert_eq!(lease1.token().inner().get(), 1);
    assert_eq!(lease2.token().inner().get(), 2);
    assert_eq!(lease3.token().inner().get(), 3);

    assert!(lease1.token().inner().get() < lease2.token().inner().get());
    assert!(lease2.token().inner().get() < lease3.token().inner().get());
}

#[test]
fn fence_token_strictly_increases_after_release_and_reacquire() {
    let store = DeterministicLeaseStore::new();
    let instance_id = parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let step_id = parse_step_id("step-1");

    let lease1 = store.acquire(&instance_id, &step_id, 5_000).unwrap();
    assert_eq!(lease1.token().inner().get(), 1);

    store.release(&lease1).unwrap();

    let lease2 = store.acquire(&instance_id, &step_id, 5_000).unwrap();
    assert_eq!(lease2.token().inner().get(), 2);

    assert!(lease1.token().inner().get() < lease2.token().inner().get());
}

#[test]
fn fence_token_increases_after_lease_expires() {
    let store = DeterministicLeaseStore::new();
    let instance_id = parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let step_id = parse_step_id("step-1");

    let lease1 = store.acquire(&instance_id, &step_id, 1).unwrap();
    assert_eq!(lease1.token().inner().get(), 1);

    store.set_time(1);

    let lease2 = store.acquire(&instance_id, &step_id, 5_000).unwrap();
    assert_eq!(lease2.token().inner().get(), 2);

    assert!(lease1.token().inner().get() < lease2.token().inner().get());
}

#[test]
fn fence_token_is_strictly_increasing_across_interleaved_pairs() {
    let store = DeterministicLeaseStore::new();
    let iid1 = parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let sid1 = parse_step_id("step-1");
    let iid2 = parse_instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA");
    let sid2 = parse_step_id("step-b");

    let a1 = store.acquire(&iid1, &sid1, 5_000).unwrap();
    let b1 = store.acquire(&iid2, &sid2, 5_000).unwrap();

    store.release(&a1).unwrap();

    let a2 = store.acquire(&iid1, &sid1, 5_000).unwrap();
    let b2 = store.acquire(&iid2, &sid2, 5_000).unwrap();

    assert_eq!(a1.token().inner().get(), 1);
    assert_eq!(b1.token().inner().get(), 1);
    assert_eq!(a2.token().inner().get(), 2);
    assert_eq!(b2.token().inner().get(), 2);

    assert!(a1.token().inner().get() < a2.token().inner().get());
    assert!(b1.token().inner().get() < b2.token().inner().get());
}

#[test]
fn fence_token_starts_at_one_for_new_pair() {
    let store = DeterministicLeaseStore::new();
    let instance_id = parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let step_id = parse_step_id("step-1");

    let lease = store.acquire(&instance_id, &step_id, 5_000).unwrap();
    assert_eq!(lease.token().inner().get(), 1);
}

#[test]
fn fence_token_never_decreases_across_full_lease_lifecycle() {
    let store = DeterministicLeaseStore::new();
    let instance_id = parse_instance_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let step_id = parse_step_id("step-1");

    let lease1 = store.acquire(&instance_id, &step_id, 1).unwrap();
    store.set_time(1);

    let lease2 = store.acquire(&instance_id, &step_id, 1).unwrap();
    store.set_time(2);

    let lease3 = store.acquire(&instance_id, &step_id, 1).unwrap();
    store.set_time(3);

    let lease4 = store.acquire(&instance_id, &step_id, 5_000).unwrap();

    assert!(lease1.token().inner().get() < lease2.token().inner().get());
    assert!(lease2.token().inner().get() < lease3.token().inner().get());
    assert!(lease3.token().inner().get() < lease4.token().inner().get());
}
