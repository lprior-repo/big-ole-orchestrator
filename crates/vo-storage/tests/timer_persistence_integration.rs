//! TDD-RED: Atomic timer persistence and resumption tests (ve-8fz8t / ve-0dd02)
//!
//! Tests cover four contract scenarios:
//! 1. Timer persistence on crash — timers survive storage restart
//! 2. Resumption from checkpoint — due timers found after restart
//! 3. Duplicate timer prevention — fencing prevents double-dispatch
//! 4. Timer expiry after restart — expired timers detected after crash
//!
//! Contract invariants (from ve-0dd02):
//! - Timers are never fired before their expiration timestamp
//! - Dispatches atomically mark timers as triggered (single-delivery)
//! - Storage returns expired timers with fence tokens to prevent duplicates

#![allow(clippy::unwrap_used, clippy::similar_names)]

use vo_storage::codec::StorageError;
use vo_storage::timer_index::{
    scan_all_timers_for_instance, scan_due_timers, timer_delete, timer_set, Storage, TimerRecord,
};
use vo_types::{InstanceId, TimerId};

// ── Stub types for poll_expired_timers (TDD-RED) ──────────────────────────────
// These stubs define the contract for the atomic timer claiming API.
// Replace with `use vo_storage::timer_index::{poll_expired_timers, ClaimedTimer};`
// when the implementation lands.

/// A timer atomically claimed with a fence token to prevent duplicate dispatch.
#[derive(Debug, Clone)]
struct ClaimedTimer {
    record: TimerRecord,
    fence_token: u64,
}

/// Error type for poll_expired_timers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum PollError {
    #[error("not implemented: poll_expired_timers has not been implemented yet")]
    NotImplemented,
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

/// Atomically scans for expired timers and claims them with fence tokens.
/// STUB: Always returns `PollError::NotImplemented`.
fn poll_expired_timers(
    _storage: &mut FjallTimerStorage,
    _now_ms: u64,
    _max_count: usize,
    _fence_token: u64,
) -> Result<Vec<ClaimedTimer>, PollError> {
    Err(PollError::NotImplemented)
}

// ── FjallStorage adapter ──────────────────────────────────────────────────────

/// Real fjall-backed implementation of `timer_index::Storage`.
/// In fjall 3, `Keyspace` is the partition handle with insert/get/remove/range.
struct FjallTimerStorage {
    ks: fjall::Keyspace,
}

impl FjallTimerStorage {
    fn new(ks: fjall::Keyspace) -> Self {
        Self { ks }
    }
}

impl Storage for FjallTimerStorage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.ks.insert(key, value)?;
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        match self.ks.get(key) {
            Ok(Some(v)) => Ok(Some(v.to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::from(e)),
        }
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.ks.remove(key)?;
        Ok(())
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let mut result = Vec::new();
        for item in self.ks.range(start.to_vec()..end.to_vec()) {
            let (k, v) = item.into_inner().map_err(StorageError::from)?;
            result.push((k.to_vec(), v.to_vec()));
        }
        Ok(result)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup_fjall() -> (tempfile::TempDir, fjall::Database, FjallTimerStorage) {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let ks = db
        .keyspace("timers", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let storage = FjallTimerStorage::new(ks);
    (dir, db, storage)
}

fn reopen_fjall(dir: &tempfile::TempDir) -> (fjall::Database, FjallTimerStorage) {
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let ks = db
        .keyspace("timers", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let storage = FjallTimerStorage::new(ks);
    (db, storage)
}

fn make_instance_id(n: u8) -> InstanceId {
    InstanceId::from_bytes([n; 16])
}

fn make_timer_id(n: u8) -> TimerId {
    TimerId::from_bytes([n; 16])
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 1: Timer Persistence on Crash
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn timer_persists_across_fjall_restart() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(1);
    let tid = make_timer_id(1);

    // Store timer: fires at t=5000, triggered at t=4000, duration 1000ms
    timer_set(
        &mut storage,
        iid.clone(),
        tid.clone(),
        5000,
        4000,
        1000,
        1000,
    )
    .unwrap();

    // Verify stored before crash
    let timers = scan_due_timers(&storage, &iid, 5000).unwrap();
    assert_eq!(timers.len(), 1, "timer should be found before restart");

    // Simulate crash: drop everything
    drop(storage);
    drop(_db);

    // Restart: reopen the same database
    let (_db2, storage2) = reopen_fjall(&dir);

    // Verify timer survived restart
    let timers_after = scan_due_timers(&storage2, &iid, 5000).unwrap();
    assert_eq!(timers_after.len(), 1, "timer should survive crash restart");
    assert_eq!(timers_after[0].fire_at_ms, 5000);
    assert_eq!(timers_after[0].timer_id, tid);
    assert_eq!(timers_after[0].instance_id, iid);
    assert_eq!(timers_after[0].duration_ms, 1000);
}

#[test]
fn timer_value_persists_correctly_across_restart() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(5);
    let tid = make_timer_id(5);

    timer_set(
        &mut storage,
        iid.clone(),
        tid.clone(),
        10000,
        7000,
        3000,
        5000,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, storage2) = reopen_fjall(&dir);
    let timers = scan_due_timers(&storage2, &iid, 10000).unwrap();

    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].duration_ms, 3000, "duration must persist exactly");
    assert_eq!(
        timers[0].trigger_time_ms, 7000,
        "trigger_time must reconstruct"
    );
}

#[test]
fn deleted_timer_stays_deleted_across_restart() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(11);
    let tid = make_timer_id(11);

    timer_set(
        &mut storage,
        iid.clone(),
        tid.clone(),
        5000,
        4000,
        1000,
        1000,
    )
    .unwrap();
    timer_delete(&mut storage, &iid, tid, 5000).unwrap();

    drop(storage);
    drop(_db);

    let (_db2, storage2) = reopen_fjall(&dir);
    let timers = scan_due_timers(&storage2, &iid, 5000).unwrap();
    assert!(
        timers.is_empty(),
        "deleted timer must not resurface after restart"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test Group 2: Resumption from Checkpoint
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_timers_resume_from_checkpoint_after_crash() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(2);

    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(1),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(2),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(3),
        3000,
        2500,
        500,
        0,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, storage2) = reopen_fjall(&dir);

    let all_timers = scan_all_timers_for_instance(&storage2, &iid).unwrap();
    assert_eq!(all_timers.len(), 3, "all timers should survive crash");

    assert_eq!(scan_due_timers(&storage2, &iid, 1000).unwrap().len(), 1);
    assert_eq!(scan_due_timers(&storage2, &iid, 2000).unwrap().len(), 2);
    assert_eq!(scan_due_timers(&storage2, &iid, 3000).unwrap().len(), 3);
}

#[test]
fn timers_from_multiple_instances_resume_independently() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid_a = make_instance_id(10);
    let iid_b = make_instance_id(20);

    timer_set(
        &mut storage,
        iid_a.clone(),
        make_timer_id(1),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid_b.clone(),
        make_timer_id(2),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, storage2) = reopen_fjall(&dir);

    let timers_a = scan_due_timers(&storage2, &iid_a, 2000).unwrap();
    assert_eq!(timers_a.len(), 1);
    assert_eq!(timers_a[0].instance_id, iid_a);

    let timers_b = scan_due_timers(&storage2, &iid_b, 2000).unwrap();
    assert_eq!(timers_b.len(), 1);
    assert_eq!(timers_b[0].instance_id, iid_b);
}

#[test]
fn scan_all_includes_future_timers_after_crash() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(30);

    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(1),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(2),
        99999,
        98999,
        1000,
        0,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, storage2) = reopen_fjall(&dir);

    let all = scan_all_timers_for_instance(&storage2, &iid).unwrap();
    assert_eq!(all.len(), 2, "both past and future timers should survive");

    let due = scan_due_timers(&storage2, &iid, 5000).unwrap();
    assert_eq!(due.len(), 1, "only past timer is due");
    assert_eq!(due[0].fire_at_ms, 1000);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test Group 3: Duplicate Timer Prevention (TDD-RED — FAILING via stub)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn poll_expired_timers_prevents_duplicate_dispatch_via_fencing() {
    let (_dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(3);
    let tid = make_timer_id(3);

    timer_set(
        &mut storage,
        iid.clone(),
        tid.clone(),
        5000,
        4000,
        1000,
        1000,
    )
    .unwrap();

    let claimed_1 = poll_expired_timers(&mut storage, 6000, 10, 1);
    assert!(
        claimed_1.is_ok(),
        "first poll should succeed: {:?}",
        claimed_1.err()
    );
    assert_eq!(claimed_1.as_ref().unwrap().len(), 1);
    assert_eq!(claimed_1.as_ref().unwrap()[0].fence_token, 1);
    assert_eq!(claimed_1.as_ref().unwrap()[0].record.fire_at_ms, 5000);

    let claimed_2 = poll_expired_timers(&mut storage, 6000, 10, 2);
    assert!(
        claimed_2.as_ref().map(|v| v.is_empty()).unwrap_or(false),
        "second poll should return empty (duplicate prevented)"
    );
}

#[test]
fn poll_expired_timers_claims_atomically_with_max_count() {
    let (_dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(4);

    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(1),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(2),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    let claimed = poll_expired_timers(&mut storage, 2000, 1, 1);
    assert!(claimed.is_ok(), "poll should succeed: {:?}", claimed.err());
    assert_eq!(
        claimed.as_ref().unwrap().len(),
        1,
        "max_count limits claims"
    );

    let claimed_2 = poll_expired_timers(&mut storage, 2000, 10, 2);
    assert!(claimed_2.is_ok(), "second poll should succeed");
    assert_eq!(
        claimed_2.as_ref().unwrap().len(),
        1,
        "remaining timer claimed"
    );
}

#[test]
fn poll_expired_timers_returns_empty_when_no_timers_due() {
    let (_dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(9);

    timer_set(&mut storage, iid, make_timer_id(9), 10000, 9000, 1000, 0).unwrap();

    let claimed = poll_expired_timers(&mut storage, 5000, 10, 1);
    assert!(claimed.is_ok(), "poll should succeed with no due timers");
    assert!(
        claimed.as_ref().map(|v| v.is_empty()).unwrap_or(false),
        "no timers before expiration (invariant)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test Group 4: Timer Expiry After Restart (TDD-RED — FAILING via stub)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn expired_timer_found_after_crash_with_advanced_clock() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(6);
    let tid = make_timer_id(6);

    timer_set(
        &mut storage,
        iid.clone(),
        tid.clone(),
        5000,
        4000,
        1000,
        1000,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, mut storage2) = reopen_fjall(&dir);
    let claimed = poll_expired_timers(&mut storage2, 10000, 10, 1);

    assert!(
        claimed.is_ok(),
        "poll should find expired timer: {:?}",
        claimed.err()
    );
    assert_eq!(claimed.as_ref().unwrap().len(), 1);
    assert_eq!(claimed.as_ref().unwrap()[0].record.fire_at_ms, 5000);
    assert_eq!(claimed.as_ref().unwrap()[0].fence_token, 1);
}

#[test]
fn overdue_timer_found_after_crash() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(7);
    let tid = make_timer_id(7);

    timer_set(&mut storage, iid.clone(), tid.clone(), 1000, 999, 1, 0).unwrap();

    drop(storage);
    drop(_db);

    let (_db2, mut storage2) = reopen_fjall(&dir);
    let claimed = poll_expired_timers(&mut storage2, 999999, 10, 1);

    assert!(
        claimed.is_ok(),
        "overdue timer should be found: {:?}",
        claimed.err()
    );
    assert_eq!(claimed.as_ref().unwrap().len(), 1);
    assert_eq!(claimed.as_ref().unwrap()[0].record.fire_at_ms, 1000);
}

#[test]
fn future_timer_not_claimed_after_restart() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(8);

    timer_set(
        &mut storage,
        iid,
        make_timer_id(8),
        100000,
        99000,
        1000,
        50000,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, mut storage2) = reopen_fjall(&dir);
    let claimed = poll_expired_timers(&mut storage2, 50000, 10, 1);

    assert!(claimed.is_ok(), "poll should succeed: {:?}", claimed.err());
    assert!(
        claimed.as_ref().map(|v| v.is_empty()).unwrap_or(false),
        "future timer must not be claimed (invariant)"
    );
}

#[test]
fn multiple_expired_timers_found_in_key_order_after_crash() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(12);

    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(1),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(2),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(3),
        3000,
        2500,
        500,
        0,
    )
    .unwrap();

    drop(storage);
    drop(_db);

    let (_db2, mut storage2) = reopen_fjall(&dir);
    let claimed = poll_expired_timers(&mut storage2, 5000, 10, 42);

    assert!(
        claimed.is_ok(),
        "all expired timers should be found: {:?}",
        claimed.err()
    );
    assert_eq!(claimed.as_ref().unwrap().len(), 3);

    let fire_times: Vec<u64> = claimed
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| c.record.fire_at_ms)
        .collect();
    assert_eq!(fire_times, vec![1000, 2000, 3000], "timers in key order");

    for ct in claimed.as_ref().unwrap() {
        assert_eq!(ct.fence_token, 42);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test Group 5: Fencing Invariant — claimed state survives crash
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn claimed_timer_stays_claimed_across_restart() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(13);
    let tid = make_timer_id(13);

    timer_set(
        &mut storage,
        iid.clone(),
        tid.clone(),
        5000,
        4000,
        1000,
        1000,
    )
    .unwrap();

    let claimed = poll_expired_timers(&mut storage, 6000, 10, 1);
    assert!(claimed.is_ok(), "claim should succeed");

    drop(storage);
    drop(_db);

    let (_db2, mut storage2) = reopen_fjall(&dir);
    let claimed_2 = poll_expired_timers(&mut storage2, 6000, 10, 2);
    assert!(
        claimed_2.as_ref().map(|v| v.is_empty()).unwrap_or(false),
        "claimed timer must not be re-dispatched after crash (single-delivery)"
    );
}

#[test]
fn partially_claimed_timers_resume_after_crash() {
    let (dir, _db, mut storage) = setup_fjall();
    let iid = make_instance_id(14);

    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(1),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(2),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        iid.clone(),
        make_timer_id(3),
        3000,
        2500,
        500,
        0,
    )
    .unwrap();

    let claimed_1 = poll_expired_timers(&mut storage, 3000, 1, 1);
    assert!(claimed_1.is_ok(), "first claim should succeed");
    assert_eq!(claimed_1.as_ref().unwrap().len(), 1);

    drop(storage);
    drop(_db);

    let (_db2, mut storage2) = reopen_fjall(&dir);
    let claimed_2 = poll_expired_timers(&mut storage2, 3000, 10, 2);

    assert!(claimed_2.is_ok(), "second claim should succeed");
    assert_eq!(
        claimed_2.as_ref().unwrap().len(),
        2,
        "unclaimed timers found after crash recovery"
    );
}
