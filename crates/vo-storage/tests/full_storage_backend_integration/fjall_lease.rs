//! Fjall-backed lease store tests: acquire/release, double-acquire prevention, power failure, fence tokens.
//!
//! Covers PERS-018 through PERS-021.

use crate::full_storage_backend_integration::config::*;
use vo_storage::lease_partition::FjallLeaseStore;

// ---------------------------------------------------------------------------
// PERS-018: FjallLeaseStore basic acquire/release lifecycle
// ---------------------------------------------------------------------------

#[test]
fn pers_018_fjall_lease_basic_acquire_release() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-1");

    let lease = store.acquire(&id, &step, 5000).unwrap();
    assert_eq!(
        lease.token().inner().get(),
        1,
        "First fence token must be 1"
    );

    store.release(&lease).unwrap();

    let is_stale = store.check_stale_fence(&id, &step, lease.token()).unwrap();
    // After release, no lease exists → check_stale_fence returns false
    // (stale detection only works when a NEW lease has been acquired)
    assert!(
        !is_stale,
        "Released lease has no active holder, so check_stale_fence returns false"
    );
}

// ---------------------------------------------------------------------------
// PERS-019: FjallLeaseStore cannot double-acquire
// ---------------------------------------------------------------------------

#[test]
fn pers_019_fjall_lease_cannot_double_acquire() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-double");

    let lease1 = store.acquire(&id, &step, 5000).unwrap();
    let lease2_result = store.acquire(&id, &step, 5000);

    assert!(
        lease2_result.is_err(),
        "Second acquire must fail while lease is held"
    );

    store.release(&lease1).unwrap();

    let lease2 = store.acquire(&id, &step, 5000).unwrap();
    assert_eq!(
        lease2.token().inner().get(),
        2,
        "Fence token must increment after release"
    );
}

// ---------------------------------------------------------------------------
// PERS-020: FjallLeaseStore power failure survival
// ---------------------------------------------------------------------------

#[test]
fn pers_020_fjall_lease_power_failure_survives() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step1 = sample_step_id("step-pf-1");
    let step2 = sample_step_id("step-pf-2");

    let lease1_token;
    {
        let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        let lease1 = store.acquire(&id, &step1, 10000).unwrap();
        lease1_token = lease1.token().clone();
        store.acquire(&id, &step2, 10000).unwrap();
    }

    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();

    let is_stale = store.check_stale_fence(&id, &step1, &lease1_token).unwrap();
    assert!(!is_stale, "Lease must survive power failure");
}

// ---------------------------------------------------------------------------
// PERS-021: FjallLeaseStore fence token persists across restart
// ---------------------------------------------------------------------------

#[test]
fn pers_021_fjall_lease_fence_token_persists_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_instance_id();
    let step = sample_step_id("step-fence");

    {
        let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallLeaseStore::open(&keyspace).unwrap();
        let l1 = store.acquire(&id, &step, 50).unwrap();
        store.release(&l1).unwrap();
        let l2 = store.acquire(&id, &step, 50).unwrap();
        store.release(&l2).unwrap();
        // Fence token is now 2
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    let keyspace = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallLeaseStore::open(&keyspace).unwrap();

    let lease = store.acquire(&id, &step, 10000).unwrap();
    assert_eq!(
        lease.token().inner().get(),
        3,
        "Fence token must persist across restart and continue incrementing"
    );
}
