use super::*;
use std::thread;

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

// ---------------------------------------------------------------------------
// InMemoryLeaseStore integration tests
// ---------------------------------------------------------------------------

#[test]
fn inmem_full_lifecycle_acquire_release_reacquire() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();

    let first = store.acquire(&iid, &sid, 5_000).unwrap();
    assert_eq!(first.token().inner().get(), 1);
    assert_eq!(
        store.check_stale_fence(&iid, &sid, first.token()).unwrap(),
        false
    );
    assert_eq!(store.release(&first), Ok(()));

    let second = store.acquire(&iid, &sid, 5_000).unwrap();
    assert_eq!(second.token().inner().get(), 2);
    assert_eq!(
        store.check_stale_fence(&iid, &sid, first.token()).unwrap(),
        true
    );
    assert_eq!(
        store.check_stale_fence(&iid, &sid, second.token()).unwrap(),
        false
    );
    assert_eq!(store.release(&second), Ok(()));
}

#[test]
fn inmem_renewal_via_release_reacquire_advances_fence() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();

    let mut prev_token: u64 = 0;
    for cycle in 0..10u32 {
        let lease = store.acquire(&iid, &sid, 5_000).unwrap();
        assert!(
            lease.token().inner().get() > prev_token,
            "cycle {cycle}: token {} not > {prev_token}",
            lease.token().inner().get()
        );
        prev_token = lease.token().inner().get();
        store.release(&lease).unwrap();
    }
    assert_eq!(prev_token, 10);
}

#[test]
fn inmem_expiry_allows_reacquire_with_new_fence() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();

    let first = store.acquire(&iid, &sid, 50).unwrap();
    assert_eq!(first.token().inner().get(), 1);

    thread::sleep(std::time::Duration::from_millis(80));

    let second = store.acquire(&iid, &sid, 5_000).unwrap();
    assert_eq!(second.token().inner().get(), 2);

    assert_eq!(
        store.check_stale_fence(&iid, &sid, first.token()).unwrap(),
        true
    );
    assert_eq!(
        store.check_stale_fence(&iid, &sid, second.token()).unwrap(),
        false
    );
}

#[test]
fn inmem_stale_fence_rejects_old_holder_after_expiry() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();

    let old = store.acquire(&iid, &sid, 50).unwrap();
    thread::sleep(std::time::Duration::from_millis(80));

    let _new = store.acquire(&iid, &sid, 5_000).unwrap();

    assert_eq!(
        store.release(&old),
        Err(LeaseStoreError::StaleFence {
            expected: "2".to_string(),
            actual: "1".to_string(),
        })
    );
}

#[test]
fn inmem_unexpired_lease_blocks_reacquire() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();

    let _lease = store.acquire(&iid, &sid, 30_000).unwrap();

    assert_eq!(
        store.acquire(&iid, &sid, 5_000),
        Err(LeaseStoreError::LeaseAlreadyHeld {
            instance_id: iid.to_string(),
            step_id: sid.to_string(),
        })
    );
}

#[test]
fn inmem_double_release_returns_not_found_then_stale_fence() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();

    let lease = store.acquire(&iid, &sid, 5_000).unwrap();
    assert_eq!(store.release(&lease), Ok(()));

    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::NotFound {
            instance_id: iid.to_string(),
            step_id: sid.to_string(),
        })
    );
}

#[test]
fn inmem_independent_pairs_have_independent_fences() {
    let store = InMemoryLeaseStore::new();
    let iid_a = sample_instance_id();
    let sid_a = sample_step_id();
    let iid_b = alternate_instance_id();
    let sid_b = alternate_step_id();

    let lease_a = store.acquire(&iid_a, &sid_a, 5_000).unwrap();
    let lease_b = store.acquire(&iid_b, &sid_b, 5_000).unwrap();

    assert_eq!(lease_a.token().inner().get(), 1);
    assert_eq!(lease_b.token().inner().get(), 1);

    store.release(&lease_a).unwrap();

    let lease_a2 = store.acquire(&iid_a, &sid_a, 5_000).unwrap();
    assert_eq!(lease_a2.token().inner().get(), 2);

    assert_eq!(
        store
            .check_stale_fence(&iid_a, &sid_a, lease_a.token())
            .unwrap(),
        true
    );
    assert_eq!(
        store
            .check_stale_fence(&iid_b, &sid_b, lease_b.token())
            .unwrap(),
        false
    );
}

#[test]
fn inmem_rapid_expiry_cycle_rejects_all_previous_tokens() {
    let store = InMemoryLeaseStore::new();
    let iid = sample_instance_id();
    let sid = sample_step_id();
    let cycles: u32 = 5;

    let mut tokens: Vec<FenceToken> = Vec::new();
    for i in 0..cycles {
        let lease = store.acquire(&iid, &sid, 50).unwrap();
        tokens.push(*lease.token());
        if i < cycles - 1 {
            thread::sleep(std::time::Duration::from_millis(80));
        }
    }

    for (idx, token) in tokens.iter().enumerate() {
        let is_last = idx == tokens.len() - 1;
        assert_eq!(
            store.check_stale_fence(&iid, &sid, token).unwrap(),
            !is_last,
            "token {token:?} at index {idx} staleness mismatch"
        );
    }
}

#[test]
fn inmem_zero_ttl_returns_invalid_argument() {
    let store = InMemoryLeaseStore::new();
    let result = store.acquire(&sample_instance_id(), &sample_step_id(), 0);
    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn inmem_release_nonexistent_returns_not_found() {
    let store = InMemoryLeaseStore::new();
    let lease = LeaseRecord::new(
        sample_instance_id(),
        sample_step_id(),
        FenceToken::new(1).unwrap(),
    );
    assert_eq!(
        store.release(&lease),
        Err(LeaseStoreError::NotFound {
            instance_id: sample_instance_id().to_string(),
            step_id: sample_step_id().to_string(),
        })
    );
}

#[test]
fn inmem_stale_fence_on_nonexistent_pair_returns_false() {
    let store = InMemoryLeaseStore::new();
    assert_eq!(
        store.check_stale_fence(
            &sample_instance_id(),
            &sample_step_id(),
            &FenceToken::new(1).unwrap()
        ),
        Ok(false)
    );
}

// ---------------------------------------------------------------------------
// FjallLeaseStore integration tests
// ---------------------------------------------------------------------------

mod fjall_integration {
    use super::*;
    use tempfile::tempdir;

    fn create_store() -> FjallLeaseStore {
        let dir = tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        FjallLeaseStore::open(&db).unwrap()
    }

    #[test]
    fn fjall_full_lifecycle_acquire_release_reacquire() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let first = store.acquire(&iid, &sid, 5_000).unwrap();
        assert_eq!(first.token().inner().get(), 1);
        assert_eq!(
            store.check_stale_fence(&iid, &sid, first.token()).unwrap(),
            false
        );
        assert_eq!(store.release(&first), Ok(()));

        let second = store.acquire(&iid, &sid, 5_000).unwrap();
        assert_eq!(second.token().inner().get(), 2);
        assert_eq!(
            store.check_stale_fence(&iid, &sid, first.token()).unwrap(),
            true
        );
        assert_eq!(
            store.check_stale_fence(&iid, &sid, second.token()).unwrap(),
            false
        );
        assert_eq!(store.release(&second), Ok(()));
    }

    #[test]
    fn fjall_renewal_via_release_reacquire_advances_fence() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let mut prev_token: u64 = 0;
        for cycle in 0..10u32 {
            let lease = store.acquire(&iid, &sid, 5_000).unwrap();
            assert!(
                lease.token().inner().get() > prev_token,
                "cycle {cycle}: token {} not > {prev_token}",
                lease.token().inner().get()
            );
            prev_token = lease.token().inner().get();
            store.release(&lease).unwrap();
        }
        assert_eq!(prev_token, 10);
    }

    #[test]
    fn fjall_expiry_allows_reacquire_with_new_fence() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let first = store.acquire(&iid, &sid, 50).unwrap();
        assert_eq!(first.token().inner().get(), 1);

        thread::sleep(std::time::Duration::from_millis(80));

        let second = store.acquire(&iid, &sid, 5_000).unwrap();
        assert_eq!(second.token().inner().get(), 2);

        assert_eq!(
            store.check_stale_fence(&iid, &sid, first.token()).unwrap(),
            true
        );
        assert_eq!(
            store.check_stale_fence(&iid, &sid, second.token()).unwrap(),
            false
        );
    }

    #[test]
    fn fjall_stale_fence_rejects_old_holder_after_expiry() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let old = store.acquire(&iid, &sid, 50).unwrap();
        thread::sleep(std::time::Duration::from_millis(80));

        let _new = store.acquire(&iid, &sid, 5_000).unwrap();

        assert_eq!(
            store.release(&old),
            Err(LeaseStoreError::StaleFence {
                expected: "2".to_string(),
                actual: "1".to_string(),
            })
        );
    }

    #[test]
    fn fjall_unexpired_lease_blocks_reacquire() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let _lease = store.acquire(&iid, &sid, 30_000).unwrap();

        assert_eq!(
            store.acquire(&iid, &sid, 5_000),
            Err(LeaseStoreError::LeaseAlreadyHeld {
                instance_id: iid.to_string(),
                step_id: sid.to_string(),
            })
        );
    }

    #[test]
    fn fjall_double_release_returns_not_found_then_stale_fence() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let lease = store.acquire(&iid, &sid, 5_000).unwrap();
        assert_eq!(store.release(&lease), Ok(()));

        assert_eq!(
            store.release(&lease),
            Err(LeaseStoreError::NotFound {
                instance_id: iid.to_string(),
                step_id: sid.to_string(),
            })
        );
    }

    #[test]
    fn fjall_independent_pairs_have_independent_fences() {
        let store = create_store();
        let iid_a = sample_instance_id();
        let sid_a = sample_step_id();
        let iid_b = alternate_instance_id();
        let sid_b = alternate_step_id();

        let lease_a = store.acquire(&iid_a, &sid_a, 5_000).unwrap();
        let lease_b = store.acquire(&iid_b, &sid_b, 5_000).unwrap();

        assert_eq!(lease_a.token().inner().get(), 1);
        assert_eq!(lease_b.token().inner().get(), 1);

        store.release(&lease_a).unwrap();

        let lease_a2 = store.acquire(&iid_a, &sid_a, 5_000).unwrap();
        assert_eq!(lease_a2.token().inner().get(), 2);

        assert_eq!(
            store
                .check_stale_fence(&iid_a, &sid_a, lease_a.token())
                .unwrap(),
            true
        );
        assert_eq!(
            store
                .check_stale_fence(&iid_b, &sid_b, lease_b.token())
                .unwrap(),
            false
        );
    }

    #[test]
    fn fjall_rapid_expiry_cycle_rejects_all_previous_tokens() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();
        let cycles: u32 = 5;

        let mut tokens: Vec<FenceToken> = Vec::new();
        for i in 0..cycles {
            let lease = store.acquire(&iid, &sid, 50).unwrap();
            tokens.push(*lease.token());
            if i < cycles - 1 {
                thread::sleep(std::time::Duration::from_millis(80));
            }
        }

        for (idx, token) in tokens.iter().enumerate() {
            let is_last = idx == tokens.len() - 1;
            assert_eq!(
                store.check_stale_fence(&iid, &sid, token).unwrap(),
                !is_last,
                "token {token:?} at index {idx} staleness mismatch"
            );
        }
    }

    #[test]
    fn fjall_zero_ttl_returns_invalid_argument() {
        let store = create_store();
        let result = store.acquire(&sample_instance_id(), &sample_step_id(), 0);
        assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
    }

    #[test]
    fn fjall_release_nonexistent_returns_not_found() {
        let store = create_store();
        let lease = LeaseRecord::new(
            sample_instance_id(),
            sample_step_id(),
            FenceToken::new(1).unwrap(),
        );
        assert_eq!(
            store.release(&lease),
            Err(LeaseStoreError::NotFound {
                instance_id: sample_instance_id().to_string(),
                step_id: sample_step_id().to_string(),
            })
        );
    }

    #[test]
    fn fjall_stale_fence_on_nonexistent_pair_returns_false() {
        let store = create_store();
        assert_eq!(
            store.check_stale_fence(
                &sample_instance_id(),
                &sample_step_id(),
                &FenceToken::new(1).unwrap()
            ),
            Ok(false)
        );
    }

    #[test]
    fn fjall_full_adr029_crash_recovery_lifecycle() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let engine = store.acquire(&iid, &sid, 50).unwrap();
        let subprocess_fence = *engine.token();
        assert_eq!(subprocess_fence.inner().get(), 1);

        thread::sleep(std::time::Duration::from_millis(80));

        let recovery = store.acquire(&iid, &sid, 5_000).unwrap();
        assert_eq!(recovery.token().inner().get(), 2);

        assert_eq!(
            store.release(&LeaseRecord::new(
                iid.clone(),
                sid.clone(),
                subprocess_fence
            )),
            Err(LeaseStoreError::StaleFence {
                expected: "2".to_string(),
                actual: "1".to_string(),
            })
        );

        assert_eq!(
            store
                .check_stale_fence(&iid, &sid, recovery.token())
                .unwrap(),
            false
        );
        assert_eq!(store.release(&recovery), Ok(()));
    }

    #[test]
    fn fjall_mixed_release_and_expiry_cycles_produce_monotonic_tokens() {
        let store = create_store();
        let iid = sample_instance_id();
        let sid = sample_step_id();

        let mut prev_token: u64 = 0;
        for cycle in 0..6u32 {
            let lease = store.acquire(&iid, &sid, 50).unwrap();
            assert!(
                lease.token().inner().get() > prev_token,
                "cycle {cycle}: token {} not > {prev_token}",
                lease.token().inner().get()
            );
            prev_token = lease.token().inner().get();

            if cycle % 2 == 0 {
                store.release(&lease).unwrap();
            } else {
                thread::sleep(std::time::Duration::from_millis(80));
            }
        }
    }
}
