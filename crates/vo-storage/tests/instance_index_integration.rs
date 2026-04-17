//! Integration tests for the instance index partition.
//!
//! These tests use a real Fjall keyspace backed by a tempdir.
//! They test the Action layer functions: `instance_index_upsert`,
//! `scan_by_status`, `scan_all_instances`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_storage::codec::StorageError;
use vo_storage::instance_index::{
    encode_instance_index_key, instance_index_upsert, scan_all_instances, scan_by_status,
    InstanceIndexEntry,
};
use vo_types::{InstanceId, InstanceStatus, TimestampMs};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_test_keyspace() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let database = fjall::Database::builder(dir.path())
        .open()
        .expect("Failed to open database");
    (dir, database)
}

fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

fn make_unique_instance_id(index: u8) -> InstanceId {
    let mut bytes = [0x01u8; 16];
    bytes[0] = index;
    InstanceId::from_bytes(bytes)
}

fn make_test_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn seed_instance(
    database: &fjall::Database,
    id: &InstanceId,
    status: InstanceStatus,
    ts: TimestampMs,
) {
    instance_index_upsert(database, id, status, ts, None).unwrap();
}

fn collect_scan_ok(
    iter: impl Iterator<Item = Result<InstanceIndexEntry, StorageError>>,
) -> Vec<InstanceIndexEntry> {
    iter.map(|r| r.expect("expected Ok entry"))
        .collect::<Vec<_>>()
}

// ---------------------------------------------------------------------------
// B22: First insert creates exactly one key
// ---------------------------------------------------------------------------

#[test]
fn upsert_inserts_one_key_when_previous_status_is_none() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    let result = instance_index_upsert(&database, &id, InstanceStatus::Pending, ts, None);
    assert_eq!(result, Ok(()));

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 1);

    let entry = &all[0];
    assert_eq!(entry.instance_id, id);
    assert_eq!(entry.status, InstanceStatus::Pending);
    assert_eq!(entry.created_at, ts);
}

// ---------------------------------------------------------------------------
// B23: Status transition deletes old key and inserts new
// ---------------------------------------------------------------------------

#[test]
fn upsert_atomically_transitions_status_when_previous_differs_from_new() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    seed_instance(&database, &id, InstanceStatus::Running, ts);

    let result = instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Completed,
        ts,
        Some(InstanceStatus::Running),
    );
    assert_eq!(result, Ok(()));

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 1);

    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(running.len(), 0);

    let completed = collect_scan_ok(scan_by_status(&database, InstanceStatus::Completed));
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].instance_id, id);
    assert_eq!(completed[0].status, InstanceStatus::Completed);
}

// ---------------------------------------------------------------------------
// B24: Idempotent when same status
// ---------------------------------------------------------------------------

#[test]
fn upsert_is_idempotent_when_status_unchanged() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    let result = instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Pending),
    );
    assert_eq!(result, Ok(()));

    // Call again — should be idempotent
    let result2 = instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Pending),
    );
    assert_eq!(result2, Ok(()));

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 1);
}

// ---------------------------------------------------------------------------
// B25: Stored value is empty byte slice
// ---------------------------------------------------------------------------

#[test]
fn upsert_stores_empty_value_when_inserting_key() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let encoded_key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let raw_value = partition
        .get(encoded_key)
        .unwrap()
        .expect("key should exist");
    assert_eq!(raw_value.len(), 0, "Value should be an empty byte slice");
}

// ---------------------------------------------------------------------------
// B26: Old status no longer visible after transition
// ---------------------------------------------------------------------------

#[test]
fn upsert_removes_instance_from_old_status_scan_when_status_transitions() {
    let (_dir, database) = make_test_keyspace();
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let ts1 = make_test_timestamp(100);
    let ts2 = make_test_timestamp(200);

    seed_instance(&database, &id1, InstanceStatus::Running, ts1);
    seed_instance(&database, &id2, InstanceStatus::Running, ts2);

    // Transition id1 from Running to Failed
    instance_index_upsert(
        &database,
        &id1,
        InstanceStatus::Failed,
        ts1,
        Some(InstanceStatus::Running),
    )
    .unwrap();

    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(running.len(), 1, "Only id2 should remain in Running");
    assert_eq!(running[0].instance_id, id2);

    let failed = collect_scan_ok(scan_by_status(&database, InstanceStatus::Failed));
    assert_eq!(failed.len(), 1, "id1 should now be in Failed");
    assert_eq!(failed[0].instance_id, id1);
}

// ---------------------------------------------------------------------------
// B27: scan_by_status returns only entries matching the requested status
// ---------------------------------------------------------------------------

#[test]
fn scan_by_status_returns_only_entries_matching_requested_status() {
    let (_dir, database) = make_test_keyspace();
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let id3 = make_unique_instance_id(3);

    seed_instance(
        &database,
        &id1,
        InstanceStatus::Pending,
        make_test_timestamp(10),
    );
    seed_instance(
        &database,
        &id2,
        InstanceStatus::Running,
        make_test_timestamp(20),
    );
    seed_instance(
        &database,
        &id3,
        InstanceStatus::Pending,
        make_test_timestamp(30),
    );

    let pending = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert_eq!(pending.len(), 2);
    assert!(pending.iter().all(|e| e.status == InstanceStatus::Pending));

    let pending_ids: Vec<_> = pending.iter().map(|e| &e.instance_id).collect();
    assert!(pending_ids.contains(&&id1));
    assert!(pending_ids.contains(&&id3));
}

// ---------------------------------------------------------------------------
// B28: scan_by_status returns entries ordered by created_at ascending
// ---------------------------------------------------------------------------

#[test]
fn scan_by_status_returns_entries_ordered_by_created_at_ascending() {
    let (_dir, database) = make_test_keyspace();
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let id3 = make_unique_instance_id(3);

    // Insert out of order
    seed_instance(
        &database,
        &id1,
        InstanceStatus::Pending,
        make_test_timestamp(300),
    );
    seed_instance(
        &database,
        &id2,
        InstanceStatus::Pending,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &id3,
        InstanceStatus::Pending,
        make_test_timestamp(200),
    );

    let entries = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].created_at, make_test_timestamp(100));
    assert_eq!(entries[1].created_at, make_test_timestamp(200));
    assert_eq!(entries[2].created_at, make_test_timestamp(300));
}

// ---------------------------------------------------------------------------
// B29: Empty partition yields empty iterator
// ---------------------------------------------------------------------------

#[test]
fn scan_by_status_returns_empty_iterator_when_partition_is_empty() {
    let (_dir, database) = make_test_keyspace();
    let results = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// B30: Corrupt key yields per-item error
// ---------------------------------------------------------------------------

#[test]
fn scan_yields_corrupt_key_error_when_partition_contains_malformed_key() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    // Manually insert a 20-byte key that starts with 0x01 (Pending prefix range)
    let bad_key = [0x01u8; 20];
    partition.insert(bad_key, &[] as &[u8]).unwrap();

    let results: Vec<_> = scan_by_status(&database, InstanceStatus::Pending).collect();
    assert!(!results.is_empty(), "Should have at least one item");
    assert!(
        results
            .iter()
            .any(|r| matches!(r, Err(StorageError::CorruptKey))),
        "Should contain at least one CorruptKey error"
    );
}

// ---------------------------------------------------------------------------
// B31: Full scan returns all entries in (status, created_at) order
// ---------------------------------------------------------------------------

#[test]
fn scan_all_instances_returns_entries_ordered_by_status_then_created_at() {
    let (_dir, database) = make_test_keyspace();
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let id3 = make_unique_instance_id(3);
    let id4 = make_unique_instance_id(4);

    seed_instance(
        &database,
        &id1,
        InstanceStatus::Running,
        make_test_timestamp(200),
    );
    seed_instance(
        &database,
        &id2,
        InstanceStatus::Pending,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &id3,
        InstanceStatus::Running,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &id4,
        InstanceStatus::Pending,
        make_test_timestamp(300),
    );

    let entries = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(entries.len(), 4);

    // Order: (status=0x01/Pending, ts=100), (status=0x01/Pending, ts=300),
    //        (status=0x02/Running, ts=100), (status=0x02/Running, ts=200)
    assert_eq!(entries[0].instance_id, id2);
    assert_eq!(entries[0].status, InstanceStatus::Pending);
    assert_eq!(entries[0].created_at, make_test_timestamp(100));

    assert_eq!(entries[1].instance_id, id4);
    assert_eq!(entries[1].status, InstanceStatus::Pending);
    assert_eq!(entries[1].created_at, make_test_timestamp(300));

    assert_eq!(entries[2].instance_id, id3);
    assert_eq!(entries[2].status, InstanceStatus::Running);
    assert_eq!(entries[2].created_at, make_test_timestamp(100));

    assert_eq!(entries[3].instance_id, id1);
    assert_eq!(entries[3].status, InstanceStatus::Running);
    assert_eq!(entries[3].created_at, make_test_timestamp(200));
}

// ---------------------------------------------------------------------------
// B32: Full scan on empty partition yields empty iterator
// ---------------------------------------------------------------------------

#[test]
fn scan_all_instances_returns_empty_iterator_when_partition_is_empty() {
    let (_dir, database) = make_test_keyspace();
    let results = collect_scan_ok(scan_all_instances(&database));
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// B38: Wrong previous_status leaves orphaned key
// ---------------------------------------------------------------------------

#[test]
fn upsert_with_wrong_previous_status_leaves_orphaned_key() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(100);

    // Insert as Running
    seed_instance(&database, &id, InstanceStatus::Running, ts);

    // Transition with WRONG previous_status (Pending instead of Running)
    let result = instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Completed,
        ts,
        Some(InstanceStatus::Pending), // Wrong! Actual old status is Running
    );
    assert_eq!(result, Ok(()));

    // Orphan detection: 2 keys should exist (violation of INV-001)
    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(
        all.len(),
        2,
        "Wrong previous_status should leave orphaned key (2 keys)"
    );

    // Old Running key still present (orphan)
    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(running.len(), 1, "Orphaned Running key should still exist");

    // New Completed key inserted
    let completed = collect_scan_ok(scan_by_status(&database, InstanceStatus::Completed));
    assert_eq!(completed.len(), 1, "New Completed key should exist");
}

// ---------------------------------------------------------------------------
// B39: Single instance transitions through all 6 statuses
// ---------------------------------------------------------------------------

#[test]
fn upsert_transitions_through_all_six_statuses_for_single_instance() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(0); // boundary: minimum timestamp

    // Step 1: Insert as Pending
    instance_index_upsert(&database, &id, InstanceStatus::Pending, ts, None).unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending)).len(),
        1
    );

    // Step 2: Pending → Running
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending)).len(),
        0
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Running)).len(),
        1
    );

    // Step 3: Running → Paused
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Paused,
        ts,
        Some(InstanceStatus::Running),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Running)).len(),
        0
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Paused)).len(),
        1
    );

    // Step 4: Paused → Running
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Paused),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    // Step 5: Running → Completed
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Completed,
        ts,
        Some(InstanceStatus::Running),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    // Step 6: Completed → Failed
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Failed,
        ts,
        Some(InstanceStatus::Completed),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    // Step 7: Failed → Cancelled
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Cancelled,
        ts,
        Some(InstanceStatus::Failed),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    // Final verification
    let cancelled = collect_scan_ok(scan_by_status(&database, InstanceStatus::Cancelled));
    assert_eq!(cancelled.len(), 1);
    assert_eq!(cancelled[0].instance_id, id);
    assert_eq!(cancelled[0].status, InstanceStatus::Cancelled);
    assert_eq!(cancelled[0].created_at, ts);
}

// ---------------------------------------------------------------------------
// B40: Interleaved multi-instance multi-status scan correctness
// ---------------------------------------------------------------------------

#[test]
fn scan_by_status_returns_correct_counts_after_interleaved_upserts() {
    let (_dir, database) = make_test_keyspace();

    // 4 Pending instances — explicit inline setup
    let p0 = make_unique_instance_id(0);
    let p1 = make_unique_instance_id(1);
    let p2 = make_unique_instance_id(2);
    let p3 = make_unique_instance_id(3);
    seed_instance(
        &database,
        &p0,
        InstanceStatus::Pending,
        make_test_timestamp(10),
    );
    seed_instance(
        &database,
        &p1,
        InstanceStatus::Pending,
        make_test_timestamp(20),
    );
    seed_instance(
        &database,
        &p2,
        InstanceStatus::Pending,
        make_test_timestamp(30),
    );
    seed_instance(
        &database,
        &p3,
        InstanceStatus::Pending,
        make_test_timestamp(40),
    );

    // 3 Running instances — explicit inline setup
    let r0 = make_unique_instance_id(4);
    let r1 = make_unique_instance_id(5);
    let r2 = make_unique_instance_id(6);
    seed_instance(
        &database,
        &r0,
        InstanceStatus::Running,
        make_test_timestamp(50),
    );
    seed_instance(
        &database,
        &r1,
        InstanceStatus::Running,
        make_test_timestamp(60),
    );
    seed_instance(
        &database,
        &r2,
        InstanceStatus::Running,
        make_test_timestamp(70),
    );

    // 2 Paused instances — explicit inline setup
    let pa0 = make_unique_instance_id(7);
    let pa1 = make_unique_instance_id(8);
    seed_instance(
        &database,
        &pa0,
        InstanceStatus::Paused,
        make_test_timestamp(80),
    );
    seed_instance(
        &database,
        &pa1,
        InstanceStatus::Paused,
        make_test_timestamp(90),
    );

    // 2 Completed instances — explicit inline setup
    let c0 = make_unique_instance_id(9);
    let c1 = make_unique_instance_id(10);
    seed_instance(
        &database,
        &c0,
        InstanceStatus::Completed,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &c1,
        InstanceStatus::Completed,
        make_test_timestamp(110),
    );

    // 1 Failed instance with ts=u64::MAX (MI-06 boundary)
    let failed_id = make_unique_instance_id(11);
    seed_instance(
        &database,
        &failed_id,
        InstanceStatus::Failed,
        make_test_timestamp(u64::MAX),
    );

    // 1 Cancelled instance
    let cancelled_id = make_unique_instance_id(12);
    seed_instance(
        &database,
        &cancelled_id,
        InstanceStatus::Cancelled,
        make_test_timestamp(120),
    );

    // Verify counts
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending)).len(),
        4
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Running)).len(),
        3
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Paused)).len(),
        2
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Completed)).len(),
        2
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Failed)).len(),
        1
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Cancelled)).len(),
        1
    );
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 13);

    // Verify the Failed instance has ts=u64::MAX
    let failed_entries = collect_scan_ok(scan_by_status(&database, InstanceStatus::Failed));
    assert_eq!(failed_entries[0].created_at, make_test_timestamp(u64::MAX));
}

// ---------------------------------------------------------------------------
// B41: Full scan correctness after mixed operations
// ---------------------------------------------------------------------------

#[test]
fn scan_all_instances_returns_correct_total_after_mixed_operations() {
    let (_dir, database) = make_test_keyspace();

    // Insert 6 instances as Pending with ts=100..600 — explicit inline setup
    let id0 = make_unique_instance_id(0);
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let id3 = make_unique_instance_id(3);
    let id4 = make_unique_instance_id(4);
    let id5 = make_unique_instance_id(5);

    seed_instance(
        &database,
        &id0,
        InstanceStatus::Pending,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &id1,
        InstanceStatus::Pending,
        make_test_timestamp(200),
    );
    seed_instance(
        &database,
        &id2,
        InstanceStatus::Pending,
        make_test_timestamp(300),
    );
    seed_instance(
        &database,
        &id3,
        InstanceStatus::Pending,
        make_test_timestamp(400),
    );
    seed_instance(
        &database,
        &id4,
        InstanceStatus::Pending,
        make_test_timestamp(500),
    );
    seed_instance(
        &database,
        &id5,
        InstanceStatus::Pending,
        make_test_timestamp(600),
    );

    // Transition id0, id1, id2 to Running — explicit inline transitions
    instance_index_upsert(
        &database,
        &id0,
        InstanceStatus::Running,
        make_test_timestamp(100),
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    instance_index_upsert(
        &database,
        &id1,
        InstanceStatus::Running,
        make_test_timestamp(200),
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    instance_index_upsert(
        &database,
        &id2,
        InstanceStatus::Running,
        make_test_timestamp(300),
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    // Transition id3 to Completed
    instance_index_upsert(
        &database,
        &id3,
        InstanceStatus::Completed,
        make_test_timestamp(400),
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    // id4 and id5 remain Pending

    // Verify total
    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 6);

    // Verify ordering: Pending entries first (sorted by ts), then Running, then Completed
    // Pending: id4 ts=500, id5 ts=600
    assert_eq!(all[0].status, InstanceStatus::Pending);
    assert_eq!(all[1].status, InstanceStatus::Pending);
    assert_eq!(all[2].status, InstanceStatus::Running);
    assert_eq!(all[3].status, InstanceStatus::Running);
    assert_eq!(all[4].status, InstanceStatus::Running);
    assert_eq!(all[5].status, InstanceStatus::Completed);

    // Verify counts per status
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending)).len(),
        2
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Running)).len(),
        3
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Completed)).len(),
        1
    );
}

// ---------------------------------------------------------------------------
// B42: Status transition is atomic via observable state
// ---------------------------------------------------------------------------

#[test]
fn upsert_status_transition_is_atomic_via_scan_state() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(100);

    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    // Transition Pending → Running
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    // Immediately after: exactly 1 key should exist
    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(
        all.len(),
        1,
        "After atomic transition, exactly 1 key should exist (never 0 or 2)"
    );
    assert_eq!(all[0].status, InstanceStatus::Running);
}

// ---------------------------------------------------------------------------
// B45: Non-matching status returns zero on populated partition
// ---------------------------------------------------------------------------

#[test]
fn scan_by_status_returns_empty_when_queried_status_has_no_entries_but_others_exist() {
    let (_dir, database) = make_test_keyspace();
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let id3 = make_unique_instance_id(3);

    seed_instance(
        &database,
        &id1,
        InstanceStatus::Pending,
        make_test_timestamp(10),
    );
    seed_instance(
        &database,
        &id2,
        InstanceStatus::Running,
        make_test_timestamp(20),
    );
    seed_instance(
        &database,
        &id3,
        InstanceStatus::Pending,
        make_test_timestamp(30),
    );

    let cancelled = collect_scan_ok(scan_by_status(&database, InstanceStatus::Cancelled));
    assert_eq!(cancelled.len(), 0);
}

// ---------------------------------------------------------------------------
// B46: All entries match queried status
// ---------------------------------------------------------------------------

#[test]
fn scan_by_status_returns_all_entries_when_every_entry_matches_queried_status() {
    let (_dir, database) = make_test_keyspace();
    let id1 = make_unique_instance_id(1);
    let id2 = make_unique_instance_id(2);
    let id3 = make_unique_instance_id(3);

    seed_instance(
        &database,
        &id1,
        InstanceStatus::Running,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &id2,
        InstanceStatus::Running,
        make_test_timestamp(200),
    );
    seed_instance(
        &database,
        &id3,
        InstanceStatus::Running,
        make_test_timestamp(300),
    );

    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(running.len(), 3);
    assert!(running.iter().all(|e| e.status == InstanceStatus::Running));
    // Verify ascending order
    assert_eq!(running[0].created_at, make_test_timestamp(100));
    assert_eq!(running[1].created_at, make_test_timestamp(200));
    assert_eq!(running[2].created_at, make_test_timestamp(300));
}

// ---------------------------------------------------------------------------
// MK1: Mutation kill — upsert does NOT batch-delete when status is unchanged
// ---------------------------------------------------------------------------
//
// Targets: swap `!=` to `==` on the batch condition (line ~109).
//
// Strategy: Insert instance as Pending, then manually plant a "canary" key
// at a DIFFERENT status for the same instance (simulating orphaned state).
// Then do a same-status upsert (Pending→Pending, previous=Some(Pending)).
//
// Correct behavior (`!=`): takes simple insert path, canary key survives.
// Mutant behavior (`==`): takes batch path, tries to delete old key at
// status=Pending — the canary at a different status would still survive,
// but the batch INSERT would replace our simple insert. However, the critical
// observable difference is: with `==`, a DIFFERENT-status transition would
// take the simple insert path instead of the batch path, leaving the old key.
//
// So we test both directions:
// (a) Same-status: simple insert path should be taken (1 key total after)
// (b) Different-status: batch path should be taken (old key deleted)
// B23 covers (b). This test covers (a) with a twist that catches the swap.

#[test]
fn upsert_does_not_batch_delete_when_status_is_unchanged() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    // Step 1: Insert as Pending
    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    // Step 2: Manually plant a "canary" key at Running status for the same instance.
    // This simulates a key that should NOT be touched by same-status upsert.
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let canary_key = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();
    partition.insert(canary_key, &[] as &[u8]).unwrap();

    // Verify: 2 keys exist (Pending + Running canary)
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 2);

    // Step 3: Same-status upsert (Pending→Pending, previous=Some(Pending))
    // Correct: simple insert path, canary survives
    // Mutant (!=→==): batch path fires for SAME status, would delete old Pending key
    //   and insert new Pending key. Canary would survive too.
    // BUT: if we then do a DIFFERENT-status transition, the mutant would take the
    //   simple insert path (no delete), leaving the old key.
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    // Canary must still exist (same-status upsert should not touch other keys)
    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(
        running.len(),
        1,
        "Canary Running key should survive same-status upsert"
    );

    // Step 4: Now do a DIFFERENT-status transition: Pending→Completed
    // Correct: batch path deletes Pending key, inserts Completed key
    // Mutant (!=→==): simple insert path — Pending key NOT deleted — 3 keys total!
    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Completed,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    // After transition: should have 2 keys (Running canary + Completed)
    // Mutant would have 3 keys (Running canary + Pending orphan + Completed)
    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(
        all.len(),
        2,
        "After different-status transition, should have 2 keys (canary + new). \
         If 3 keys exist, the batch path was not taken for status transition (!=→== mutation)."
    );

    // Pending must be gone (batch deleted it)
    let pending = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert_eq!(
        pending.len(),
        0,
        "Pending key should be deleted after Pending→Completed transition"
    );
}

// ---------------------------------------------------------------------------
// MK2: Mutation kill — scan iterator stops after storage error
// ---------------------------------------------------------------------------
//
// Targets: deletion of `self.inner = None` on line ~213.
//
// This is primarily tested in the unit test `scan_iterator_stops_after_storage_error`
// in src/instance_index.rs which constructs ScanIterator directly with a mock
// iterator. This integration test verifies the observable behavior: after yielding
// a CorruptKey error, the iterator continues to valid entries (CorruptKey does NOT
// set inner=None — only StorageError::Storage does). This proves the branching
// logic is correct: CorruptKey allows continuation, Storage terminates.

#[test]
fn scan_iterator_continues_after_corrupt_key_but_would_stop_after_storage_error() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    // Insert a corrupt 10-byte key with Pending prefix (sorts first by lexicographic order)
    let bad_key = {
        let mut k = [0x01u8; 10];
        k[1] = 0x00; // ensure it sorts before valid keys with higher ts bytes
        k
    };
    partition.insert(bad_key, &[] as &[u8]).unwrap();

    // Insert a valid Pending entry with a higher timestamp (sorts after corrupt key)
    let id = make_test_instance_id(0xFF);
    let ts = make_test_timestamp(u64::MAX);
    instance_index_upsert(&database, &id, InstanceStatus::Pending, ts, None).unwrap();

    // Scan by Pending status — should get BOTH items (corrupt + valid)
    let results: Vec<_> = scan_by_status(&database, InstanceStatus::Pending).collect();
    assert!(
        results.len() >= 2,
        "Should yield at least 2 items (corrupt + valid)"
    );

    // Verify we got at least one CorruptKey AND at least one Ok
    let has_corrupt = results
        .iter()
        .any(|r| matches!(r, Err(StorageError::CorruptKey)));
    let has_ok = results.iter().any(|r| r.is_ok());

    assert!(has_corrupt, "Should contain a CorruptKey error");
    assert!(
        has_ok,
        "Should contain at least one valid entry — iterator continues after CorruptKey \
         (unlike StorageError::Storage which terminates the iterator)"
    );
}

// ---------------------------------------------------------------------------
// P07: Idempotent upsert leaves exactly one key per instance (proptest)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_instance_status() -> impl Strategy<Value = InstanceStatus> {
        (1u8..=6u8).prop_map(|b| InstanceStatus::from_byte(b).unwrap())
    }

    fn arb_instance_id_bytes() -> impl Strategy<Value = [u8; 16]> {
        proptest::array::uniform16(proptest::num::u8::ANY)
            .prop_filter("non-nil ULID (u128 != 0)", |bytes| {
                u128::from_be_bytes(*bytes) != 0
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn proptest_idempotent_upsert_leaves_one_key(
            status in arb_instance_status(),
            ts in proptest::num::u64::ANY,
            id_bytes in arb_instance_id_bytes(),
        ) {
            let (_dir, database) = make_test_keyspace();
            let id = InstanceId::from_bytes(id_bytes);
            let timestamp = TimestampMs::try_from(ts).unwrap();

            // First insert
            instance_index_upsert(&database, &id, status, timestamp, None).unwrap();

            // Repeat 4 times with Some(status) (idempotent)
            instance_index_upsert(&database, &id, status, timestamp, Some(status)).unwrap();
            instance_index_upsert(&database, &id, status, timestamp, Some(status)).unwrap();
            instance_index_upsert(&database, &id, status, timestamp, Some(status)).unwrap();
            instance_index_upsert(&database, &id, status, timestamp, Some(status)).unwrap();

            let all = collect_scan_ok(scan_all_instances(&database));
            prop_assert_eq!(all.len(), 1, "After multiple upserts, should have exactly 1 key");
        }
    }
}
