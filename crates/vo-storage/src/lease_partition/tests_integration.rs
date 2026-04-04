#![allow(clippy::unwrap_used)]
use super::*;
use std::collections::HashMap;

fn test_instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn test_step_id() -> StepId {
    StepId::parse("step-1").unwrap()
}

fn test_fence_token(v: u64) -> FenceToken {
    FenceToken::new(v).unwrap()
}

// ========================================================================
// Trait Integration — via MockLeaseStore
// ========================================================================

struct MockLeaseStore {
    leases: std::cell::RefCell<HashMap<String, LeaseEntry>>,
    fence_counter: std::cell::Cell<u64>,
}

impl MockLeaseStore {
    fn new() -> Self {
        Self {
            leases: std::cell::RefCell::new(HashMap::new()),
            fence_counter: std::cell::Cell::new(1),
        }
    }
}

impl LeaseStore for MockLeaseStore {
    fn acquire(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        ttl_ms: u64,
    ) -> Result<LeaseRecord, LeaseStoreError> {
        if ttl_ms == 0 {
            return Err(LeaseStoreError::InvalidArgument);
        }
        let key = format!("{instance_id}::{step_id}");
        let now = 0u64;
        let mut leases = self.leases.borrow_mut();

        if let Some(existing) = leases.get(&key) {
            if !existing.is_expired(now) {
                return Err(LeaseStoreError::LeaseAlreadyHeld {
                    instance_id: format!("{instance_id}"),
                    step_id: format!("{step_id}"),
                });
            }
        }

        let fence = self.fence_counter.get();
        self.fence_counter.set(fence + 1);
        let entry = LeaseEntry::new(
            format!("{instance_id}"),
            format!("{step_id}"),
            fence,
            ttl_ms,
        )?;
        let record = entry.to_lease_record()?;
        leases.insert(key, entry);
        Ok(record)
    }

    fn release(&self, lease: &LeaseRecord) -> Result<(), LeaseStoreError> {
        let key = format!("{}::{}", lease.instance_id(), lease.step_id());
        let mut leases = self.leases.borrow_mut();

        let existing = leases.get(&key).ok_or_else(|| LeaseStoreError::NotFound {
            instance_id: format!("{}", lease.instance_id()),
            step_id: format!("{}", lease.step_id()),
        })?;

        let existing_token = existing.fence_token();
        let lease_token = lease.token().inner().get();

        if existing_token != lease_token {
            return Err(LeaseStoreError::StaleFence {
                expected: existing_token.to_string(),
                actual: lease_token.to_string(),
            });
        }

        leases.remove(&key);
        Ok(())
    }

    fn check_stale_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        token: &FenceToken,
    ) -> Result<bool, LeaseStoreError> {
        let key = format!("{instance_id}::{step_id}");
        let leases = self.leases.borrow();

        if let Some(existing) = leases.get(&key) {
            if existing.fence_token() != token.inner().get() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[test]
fn acquire_lease_returns_valid_lease_record() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    let result = store.acquire(&iid, &sid, 5000);
    assert!(result.is_ok());
    let lease = result.unwrap();
    assert_eq!(lease.instance_id(), &iid);
    assert_eq!(lease.step_id(), &sid);
}

#[test]
fn release_lease_succeeds() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    let lease = store.acquire(&iid, &sid, 5000).unwrap();
    let result = store.release(&lease);
    assert!(result.is_ok());
}

#[test]
fn double_acquire_returns_lease_already_held_error() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    store.acquire(&iid, &sid, 5000).unwrap();
    let result = store.acquire(&iid, &sid, 5000);
    assert!(matches!(
        result,
        Err(LeaseStoreError::LeaseAlreadyHeld { .. })
    ));
}

#[test]
fn acquire_returns_error_for_zero_ttl() {
    let store = MockLeaseStore::new();
    let result = store.acquire(&test_instance_id(), &test_step_id(), 0);
    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn release_with_stale_fence_returns_stale_fence_error() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    let _lease = store.acquire(&iid, &sid, 5000).unwrap();
    let stale = LeaseRecord::new(iid, sid, test_fence_token(999));
    let result = store.release(&stale);
    assert!(matches!(result, Err(LeaseStoreError::StaleFence { .. })));
}

#[test]
fn check_stale_fence_returns_true_for_stale_token() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    store.acquire(&iid, &sid, 5000).unwrap();
    let stale_token = test_fence_token(999);
    let result = store.check_stale_fence(&iid, &sid, &stale_token).unwrap();
    assert!(result);
}

#[test]
fn check_stale_fence_returns_false_for_current_token() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    let lease = store.acquire(&iid, &sid, 5000).unwrap();
    let result = store.check_stale_fence(&iid, &sid, lease.token()).unwrap();
    assert!(!result);
}

#[test]
fn check_stale_fence_returns_false_when_no_lease() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    let result = store
        .check_stale_fence(&iid, &sid, &test_fence_token(1))
        .unwrap();
    assert!(!result);
}

#[test]
fn release_nonexistent_lease_returns_not_found() {
    let store = MockLeaseStore::new();
    let iid = test_instance_id();
    let sid = test_step_id();
    let lease = LeaseRecord::new(iid, sid, test_fence_token(1));
    let result = store.release(&lease);
    assert!(matches!(result, Err(LeaseStoreError::NotFound { .. })));
}
