#![allow(clippy::needless_for_each)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::into_iter_on_ref)]
//! Red Queen adversarial tests for the instance index partition.
//!
//! These tests attempt to break the implementation through:
//! - Contract violation attempts
//! - Edge cases (boundary values, nil UUIDs, extreme timestamps)
//! - Key encoding attacks (prefix scan confusion, status byte boundaries)
//! - Invariant violations under stress (phantom entries, large volumes)
//! - Ordering verification under adversarial conditions
//!
//! bead_id: vel-ngt
//! bead_title: vo-storage: implement instance index partition
//! phase: 5

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_storage::codec::StorageError;
use vo_storage::instance_index::{
    decode_instance_index_key, encode_instance_index_key, instance_index_upsert,
    scan_all_instances, scan_by_status, InstanceIndexEntry,
};
use vo_types::{InstanceId, InstanceStatus, TimestampMs};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_test_keyspace() -> (tempfile::TempDir, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let keyspace = fjall::Config::new(dir.path())
        .open()
        .expect("Failed to open keyspace");
    (dir, keyspace)
}

fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

fn make_unique_instance_id(index: u16) -> InstanceId {
    let mut bytes = [0x01u8; 16];
    let idx_bytes = index.to_be_bytes();
    bytes[0] = idx_bytes[0];
    bytes[1] = idx_bytes[1];
    InstanceId::from_bytes(bytes)
}

fn make_test_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn seed_instance(
    keyspace: &fjall::Keyspace,
    id: &InstanceId,
    status: InstanceStatus,
    ts: TimestampMs,
) {
    instance_index_upsert(keyspace, id, status, ts, None).unwrap();
}

fn collect_scan_ok(
    iter: impl Iterator<Item = Result<InstanceIndexEntry, StorageError>>,
) -> Vec<InstanceIndexEntry> {
    iter.map(|r| r.expect("expected Ok entry"))
        .collect::<Vec<_>>()
}

// ===========================================================================
// DIMENSION 1: Contract Violation Attempts
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-CV01: Decode rejects every invalid status byte in [0x00, 0x07..=0xFF]
// ---------------------------------------------------------------------------

#[test]
fn rq_decode_rejects_every_invalid_status_byte_exhaustively() {
    (0u8..=0xFF).into_iter().for_each(|byte| {
        let mut key = [0x01u8; 25]; // valid 25-byte key template
        key[0] = byte;
        let result = decode_instance_index_key(&key);
        if (0x01..=0x06).contains(&byte) {
            let Ok(_val) = result else {
                panic!("Status byte 0x{byte:02X} should be valid, got {result:?}");
            };
        } else {
            assert_eq!(
                result,
                Err(StorageError::CorruptKey),
                "Status byte 0x{byte:02X} should be rejected"
            );
        }
    });
}

// ---------------------------------------------------------------------------
// RQ-CV02: Decode rejects every invalid length from 0..=50 (except 25)
// ---------------------------------------------------------------------------

#[test]
fn rq_decode_rejects_every_invalid_length_from_0_to_50() {
    (0usize..=50).into_iter().for_each(|len| {
        let key = vec![0x01u8; len];
        let result = decode_instance_index_key(&key);
        if len == 25 {
            let Ok(_val) = result else {
                panic!("Length 25 should be valid, got {result:?}");
            };
        } else {
            assert_eq!(
                result,
                Err(StorageError::CorruptKey),
                "Length {len} should be rejected"
            );
        }
    });
}

// ---------------------------------------------------------------------------
// RQ-CV03: Nil UUID (all-zero bytes) round-trip through encode/decode
// ---------------------------------------------------------------------------

#[test]
fn rq_nil_uuid_encode_decode_behavior_is_consistent() {
    let nil_id = InstanceId::from_bytes([0x00; 16]);
    let ts = make_test_timestamp(1000);

    // from_bytes([0x00; 16]) produces "00000000000000000000000000"
    // to_bytes() must re-parse this ULID string. It should succeed (ULID 0 is valid
    // at the ULID level) or fail with CorruptKey. Either way, behavior must be consistent.
    let encode_result = encode_instance_index_key(InstanceStatus::Pending, ts, &nil_id);

    match encode_result {
        Ok(key) => {
            // If encoding succeeds, decoding MUST succeed and round-trip
            let entry =
                decode_instance_index_key(&key).expect("Decode must succeed if encode succeeded");
            assert_eq!(entry.instance_id, nil_id);
            assert_eq!(entry.status, InstanceStatus::Pending);
            assert_eq!(entry.created_at, ts);
            assert_eq!(key.len(), 25);
            assert_eq!(&key[9..25], &[0x00u8; 16]);
        }
        Err(StorageError::CorruptKey) => {
            // Acceptable: type system rejects nil ULID during to_bytes()
            // This is the documented behavior in the existing test B43
        }
        Err(other) => {
            panic!("Unexpected error variant for nil UUID: {other:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// RQ-CV04: Nil UUID upsert behavior (if encoding succeeds)
// ---------------------------------------------------------------------------

#[test]
fn rq_nil_uuid_upsert_either_succeeds_consistently_or_fails_with_corrupt_key() {
    let (_dir, keyspace) = make_test_keyspace();
    let nil_id = InstanceId::from_bytes([0x00; 16]);
    let ts = make_test_timestamp(500);

    let result = instance_index_upsert(&keyspace, &nil_id, InstanceStatus::Pending, ts, None);

    match result {
        Ok(()) => {
            // If upsert succeeds, scan must find exactly one entry
            let all = collect_scan_ok(scan_all_instances(&keyspace));
            assert_eq!(
                all.len(),
                1,
                "Nil UUID upsert succeeded but scan found != 1 entry"
            );
            assert_eq!(all[0].instance_id, nil_id);
        }
        Err(StorageError::CorruptKey) => {
            // Acceptable: nil UUID rejected during encoding
            let all = collect_scan_ok(scan_all_instances(&keyspace));
            assert_eq!(all.len(), 0, "Failed upsert should leave no entries");
        }
        Err(other) => {
            panic!("Unexpected error variant for nil UUID upsert: {other:?}");
        }
    }
}

// ===========================================================================
// DIMENSION 2: Edge Cases (Boundary Values)
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-EC01: u64::MAX timestamp encode/decode round-trip
// ---------------------------------------------------------------------------

#[test]
fn rq_u64_max_timestamp_round_trips_through_encode_decode() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(u64::MAX);
    let key = encode_instance_index_key(InstanceStatus::Failed, ts, &id).unwrap();

    // Verify the timestamp bytes are 0xFF * 8
    assert_eq!(&key[1..9], &[0xFF; 8]);

    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.created_at, ts);
    assert_eq!(entry.instance_id, id);
}

// ---------------------------------------------------------------------------
// RQ-EC02: Zero timestamp encode/decode round-trip
// ---------------------------------------------------------------------------

#[test]
fn rq_zero_timestamp_round_trips_through_encode_decode() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(0);
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();

    assert_eq!(&key[1..9], &[0x00; 8]);

    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.created_at, ts);
}

// ---------------------------------------------------------------------------
// RQ-EC03: u64::MAX timestamp ordering — must sort AFTER all other timestamps
// ---------------------------------------------------------------------------

#[test]
fn rq_u64_max_timestamp_sorts_after_all_other_timestamps_in_scan() {
    let (_dir, keyspace) = make_test_keyspace();
    let id_early = make_unique_instance_id(1);
    let id_late = make_unique_instance_id(2);
    let id_max = make_unique_instance_id(3);

    seed_instance(
        &keyspace,
        &id_early,
        InstanceStatus::Pending,
        make_test_timestamp(100),
    );
    seed_instance(
        &keyspace,
        &id_late,
        InstanceStatus::Pending,
        make_test_timestamp(u64::MAX - 1),
    );
    seed_instance(
        &keyspace,
        &id_max,
        InstanceStatus::Pending,
        make_test_timestamp(u64::MAX),
    );

    let entries = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Pending));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].created_at, make_test_timestamp(100));
    assert_eq!(entries[1].created_at, make_test_timestamp(u64::MAX - 1));
    assert_eq!(entries[2].created_at, make_test_timestamp(u64::MAX));
}

// ---------------------------------------------------------------------------
// RQ-EC04: Same timestamp, different instance IDs — deterministic ordering
// ---------------------------------------------------------------------------

#[test]
fn rq_same_timestamp_different_ids_produce_deterministic_scan_order() {
    let (_dir, keyspace) = make_test_keyspace();
    let ts = make_test_timestamp(5000);

    // Create IDs with different byte patterns
    let id_low = InstanceId::from_bytes([0x01; 16]); // lowest
    let id_mid = InstanceId::from_bytes([0x80; 16]); // middle
    let id_high = InstanceId::from_bytes([0xFF; 16]); // highest

    // Insert in reverse order to verify sort is by key, not insertion order
    seed_instance(&keyspace, &id_high, InstanceStatus::Running, ts);
    seed_instance(&keyspace, &id_low, InstanceStatus::Running, ts);
    seed_instance(&keyspace, &id_mid, InstanceStatus::Running, ts);

    let entries = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Running));
    assert_eq!(entries.len(), 3);

    // With same timestamp, tiebreak is by instance_id bytes (lexicographic)
    // [0x01; 16] < [0x80; 16] < [0xFF; 16]
    assert_eq!(entries[0].instance_id, id_low);
    assert_eq!(entries[1].instance_id, id_mid);
    assert_eq!(entries[2].instance_id, id_high);
}

// ---------------------------------------------------------------------------
// RQ-EC05: All statuses identical — single scan returns all, other scans empty
// ---------------------------------------------------------------------------

#[test]
fn rq_all_instances_same_status_returns_all_in_status_scan_none_in_others() {
    let (_dir, keyspace) = make_test_keyspace();

    (0u16..10).into_iter().for_each(|i| {
        let id = make_unique_instance_id(i);
        seed_instance(
            &keyspace,
            &id,
            InstanceStatus::Paused,
            make_test_timestamp(u64::from(i) * 100),
        );
    });

    let paused = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Paused));
    assert_eq!(paused.len(), 10);

    // Every other status must be empty
    InstanceStatus::all_variants()
        .into_iter()
        .for_each(|status| {
            if *status != InstanceStatus::Paused {
                let scan = collect_scan_ok(scan_by_status(&keyspace, *status));
                assert_eq!(
                    scan.len(),
                    0,
                    "Status {:?} should have 0 entries, found {}",
                    status,
                    scan.len()
                );
            }
        });
}

// ---------------------------------------------------------------------------
// RQ-EC06: Circular status transitions (Pending→Running→Pending→Running)
// ---------------------------------------------------------------------------

#[test]
fn rq_circular_status_transitions_leave_exactly_one_key() {
    let (_dir, keyspace) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    // Insert as Pending
    instance_index_upsert(&keyspace, &id, InstanceStatus::Pending, ts, None).unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&keyspace)).len(), 1);

    // Pending → Running
    instance_index_upsert(
        &keyspace,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&keyspace)).len(), 1);

    // Running → Pending (circular!)
    instance_index_upsert(
        &keyspace,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Running),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&keyspace)).len(), 1);

    // Pending → Running again
    instance_index_upsert(
        &keyspace,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(
        all.len(),
        1,
        "After circular transitions, exactly 1 key must exist"
    );
    assert_eq!(all[0].status, InstanceStatus::Running);
}

// ---------------------------------------------------------------------------
// RQ-EC07: InstanceId with all 0xFF bytes
// ---------------------------------------------------------------------------

#[test]
fn rq_max_instance_id_bytes_round_trip() {
    let id = InstanceId::from_bytes([0xFF; 16]);
    let ts = make_test_timestamp(0);
    let key = encode_instance_index_key(InstanceStatus::Cancelled, ts, &id).unwrap();

    assert_eq!(&key[9..25], &[0xFF; 16]);

    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.instance_id, id);
}

// ===========================================================================
// DIMENSION 3: Key Encoding Attacks (Prefix Scan Confusion)
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-KE01: Max Pending key must NOT appear in Running scan
// ---------------------------------------------------------------------------

#[test]
fn rq_max_pending_key_does_not_leak_into_running_scan() {
    let (_dir, keyspace) = make_test_keyspace();

    // Construct the maximum possible Pending key:
    // status=0x01, created_at=u64::MAX, instance_id=[0xFF; 16]
    let id_max = InstanceId::from_bytes([0xFF; 16]);
    let ts_max = make_test_timestamp(u64::MAX);

    seed_instance(&keyspace, &id_max, InstanceStatus::Pending, ts_max);

    // This key is [0x01, 0xFF, 0xFF, ..., 0xFF] — the highest possible key in the Pending bucket.
    // If prefix scan is poorly implemented, it could bleed into the Running (0x02) range.
    let running = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Running));
    assert_eq!(
        running.len(),
        0,
        "Max Pending key must NOT appear in Running scan"
    );

    let pending = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Pending));
    assert_eq!(
        pending.len(),
        1,
        "Max Pending key must appear in Pending scan"
    );
}

// ---------------------------------------------------------------------------
// RQ-KE02: Min Running key must NOT appear in Pending scan
// ---------------------------------------------------------------------------

#[test]
fn rq_min_running_key_does_not_leak_into_pending_scan() {
    let (_dir, keyspace) = make_test_keyspace();

    // Minimum Running key: status=0x02, created_at=0, instance_id=[0x01; 16]
    let id_min = InstanceId::from_bytes([0x01; 16]);
    let ts_zero = make_test_timestamp(0);

    seed_instance(&keyspace, &id_min, InstanceStatus::Running, ts_zero);

    let pending = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Pending));
    assert_eq!(
        pending.len(),
        0,
        "Min Running key must NOT appear in Pending scan"
    );

    let running = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Running));
    assert_eq!(
        running.len(),
        1,
        "Min Running key must appear in Running scan"
    );
}

// ---------------------------------------------------------------------------
// RQ-KE03: Adjacent status boundaries — max of each bucket vs min of next
// ---------------------------------------------------------------------------

#[test]
fn rq_adjacent_status_boundaries_do_not_cross_contaminate() {
    let (_dir, keyspace) = make_test_keyspace();
    let _id = InstanceId::from_bytes([0xFF; 16]);
    let ts_max = make_test_timestamp(u64::MAX);
    let ts_zero = make_test_timestamp(0);
    let _id_min = InstanceId::from_bytes([0x01; 16]);

    // Insert max key for each status and min key for the next status
    // This tests ALL 5 adjacent boundaries
    let statuses = InstanceStatus::all_variants();

    (0..statuses.len() - 1).into_iter().for_each(|i| {
        let current = statuses[i];
        let next = statuses[i + 1];

        // Use unique IDs per boundary pair to avoid key collisions
        let max_id = InstanceId::from_bytes({
            let mut b = [0xFF; 16];
            b[0] = (i as u8) * 2 + 1;
            b
        });
        let min_id = InstanceId::from_bytes({
            let mut b = [0x01; 16];
            b[0] = (i as u8) * 2 + 2;
            b
        });

        seed_instance(&keyspace, &max_id, current, ts_max);
        seed_instance(&keyspace, &min_id, next, ts_zero);
    });

    // Verify isolation: each status scan returns only its own entries
    InstanceStatus::all_variants()
        .into_iter()
        .for_each(|status| {
            let entries = collect_scan_ok(scan_by_status(&keyspace, *status));
            (&entries).into_iter().for_each(|entry| {
                assert_eq!(
                    entry.status, *status,
                    "Scan for {:?} returned entry with status {:?}",
                    status, entry.status
                );
            });
        });
}

// ---------------------------------------------------------------------------
// RQ-KE04: Manually injected key at boundary 0x01FF...FF does not bleed
// ---------------------------------------------------------------------------

#[test]
fn rq_manually_injected_boundary_key_stays_in_correct_prefix_range() {
    let (_dir, keyspace) = make_test_keyspace();
    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();

    // Max Pending key: [0x01, 0xFF x 24]
    let max_pending_key = {
        let mut k = [0xFF; 25];
        k[0] = 0x01;
        k
    };
    partition.insert(max_pending_key, &[] as &[u8]).unwrap();

    // Min Running key: [0x02, 0x00 x 24]
    let min_running_key = {
        let mut k = [0x00; 25];
        k[0] = 0x02;
        k
    };
    partition.insert(min_running_key, &[] as &[u8]).unwrap();

    // Pending scan should find exactly the max Pending key
    let pending: Vec<_> = scan_by_status(&keyspace, InstanceStatus::Pending).collect();
    assert_eq!(pending.len(), 1, "Pending scan should find exactly 1 entry");
    assert_eq!(pending[0].as_ref().unwrap().status, InstanceStatus::Pending);

    // Running scan should find exactly the min Running key
    let running: Vec<_> = scan_by_status(&keyspace, InstanceStatus::Running).collect();
    assert_eq!(running.len(), 1, "Running scan should find exactly 1 entry");
    // Min Running key has ts=0, id=[0x00; 16] which is nil — decode may succeed or CorruptKey
    // depending on whether from_bytes([0x00;16]) then to_bytes() in round-trip works.
    // Either way, it must NOT appear in Pending scan.
}

// ---------------------------------------------------------------------------
// RQ-KE05: Key with status byte 0x00 (below valid range) not in any status scan
// ---------------------------------------------------------------------------

#[test]
fn rq_key_with_zero_status_byte_not_returned_by_any_valid_status_scan() {
    let (_dir, keyspace) = make_test_keyspace();
    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();

    // Inject key with status byte 0x00 (invalid, below valid range)
    let zero_status_key = [0x00u8; 25];
    partition.insert(zero_status_key, &[] as &[u8]).unwrap();

    // No valid status scan should find it
    InstanceStatus::all_variants()
        .into_iter()
        .for_each(|status| {
            let entries: Vec<_> = scan_by_status(&keyspace, *status).collect();
            assert_eq!(
                entries.len(),
                0,
                "Status {:?} scan should not return key with 0x00 status byte",
                status
            );
        });

    // But scan_all_instances uses prefix([]) so it SHOULD find it and yield CorruptKey
    let all: Vec<_> = scan_all_instances(&keyspace).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(
        all[0],
        Err(StorageError::CorruptKey),
        "scan_all should yield CorruptKey for 0x00 status byte"
    );
}

// ---------------------------------------------------------------------------
// RQ-KE06: Key with status byte 0x07 (above valid range) not in Cancelled scan
// ---------------------------------------------------------------------------

#[test]
fn rq_key_with_0x07_status_byte_not_returned_by_cancelled_scan() {
    let (_dir, keyspace) = make_test_keyspace();
    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();

    // Inject key with status byte 0x07 (one above Cancelled=0x06)
    let mut key_above = [0x01u8; 25];
    key_above[0] = 0x07;
    partition.insert(key_above, &[] as &[u8]).unwrap();

    let cancelled: Vec<_> = scan_by_status(&keyspace, InstanceStatus::Cancelled).collect();
    assert_eq!(
        cancelled.len(),
        0,
        "Cancelled scan should not return key with 0x07 status byte"
    );

    // scan_all should yield CorruptKey
    let all: Vec<_> = scan_all_instances(&keyspace).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0], Err(StorageError::CorruptKey));
}

// ===========================================================================
// DIMENSION 4: Invariant Violations Under Stress
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-IV01: Phantom entry detection — multiple keys for same instance_id
// ---------------------------------------------------------------------------

#[test]
fn rq_phantom_entries_detectable_via_scan_count() {
    let (_dir, keyspace) = make_test_keyspace();
    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();

    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    // Manually inject the same instance_id under TWO different statuses
    // This violates INV-001 (at most one key per InstanceId)
    let key_pending = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let key_running = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();

    partition.insert(key_pending, &[] as &[u8]).unwrap();
    partition.insert(key_running, &[] as &[u8]).unwrap();

    // Full scan reveals the violation: 2 entries with same instance_id
    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(all.len(), 2, "Should detect 2 phantom entries");

    let ids: Vec<_> = all.iter().map(|e| &e.instance_id).collect();
    assert_eq!(
        ids[0], ids[1],
        "Both entries should have the same instance_id"
    );
    assert_ne!(
        all[0].status, all[1].status,
        "But different statuses (phantom)"
    );
}

// ---------------------------------------------------------------------------
// RQ-IV02: Large volume — 1000 instances across all statuses
// ---------------------------------------------------------------------------

#[test]
fn rq_1000_instances_across_all_statuses_scan_returns_correct_counts() {
    let (_dir, keyspace) = make_test_keyspace();
    let statuses = InstanceStatus::all_variants();

    // Insert 1000 instances: ~166-167 per status
    (0u16..1000).for_each(|i| {
        let id = make_unique_instance_id(i);
        let status = statuses[(i as usize) % statuses.len()];
        let ts = make_test_timestamp(u64::from(i));
        seed_instance(&keyspace, &id, status, ts);
    });

    // Verify total
    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(all.len(), 1000, "Total should be 1000");

    // Verify per-status counts
    let mut per_status_total = 0usize;
    (statuses.iter().enumerate())
        .into_iter()
        .for_each(|(idx, status)| {
            let entries = collect_scan_ok(scan_by_status(&keyspace, *status));
            let expected = if idx < 4 { 167 } else { 166 }; // 1000 / 6 = 166 r 4
            assert_eq!(
                entries.len(),
                expected,
                "Status {:?} should have {expected} entries, found {}",
                status,
                entries.len()
            );
            per_status_total += entries.len();

            // Verify all entries have correct status
            entries.iter().for_each(|entry| {
                assert_eq!(entry.status, *status);
            });

            // Verify ordering within status
            entries.windows(2).for_each(|pair| {
                assert!(
                    pair[0].created_at.as_u64() <= pair[1].created_at.as_u64(),
                    "Within {:?}: entries not ordered by created_at",
                    status
                );
            });
        });

    assert_eq!(
        per_status_total, 1000,
        "Sum of per-status counts should be 1000"
    );
}

// ---------------------------------------------------------------------------
// RQ-IV03: scan_all ordering — must be (status_byte, created_at) globally
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_returns_globally_ordered_by_status_byte_then_created_at() {
    let (_dir, keyspace) = make_test_keyspace();

    // Insert entries in random-ish order across statuses
    let data: &[(u16, InstanceStatus, u64)] = &[
        (10, InstanceStatus::Cancelled, 999),
        (1, InstanceStatus::Pending, 100),
        (5, InstanceStatus::Running, 50),
        (2, InstanceStatus::Pending, 200),
        (6, InstanceStatus::Paused, 300),
        (3, InstanceStatus::Running, 10),
        (9, InstanceStatus::Failed, 400),
        (7, InstanceStatus::Completed, 500),
        (4, InstanceStatus::Running, 75),
        (8, InstanceStatus::Completed, 250),
    ];

    (data).into_iter().for_each(|(idx, status, ts)| {
        let id = make_unique_instance_id(*idx);
        seed_instance(&keyspace, &id, *status, make_test_timestamp(*ts));
    });

    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(all.len(), data.len());

    // Verify global ordering: (status_byte, created_at) ascending
    all.windows(2).for_each(|pair| {
        let a_status = pair[0].status.to_byte();
        let b_status = pair[1].status.to_byte();
        let a_ts = pair[0].created_at.as_u64();
        let b_ts = pair[1].created_at.as_u64();

        assert!(
            (a_status, a_ts) <= (b_status, b_ts),
            "Global ordering violated: ({a_status:#04x}, {a_ts}) > ({b_status:#04x}, {b_ts})"
        );
    });
}

// ---------------------------------------------------------------------------
// RQ-IV04: Rapid transitions on many instances — no orphaned keys
// ---------------------------------------------------------------------------

#[test]
fn rq_rapid_transitions_on_50_instances_leave_exactly_50_keys() {
    let (_dir, keyspace) = make_test_keyspace();

    // Insert 50 instances as Pending
    (0u16..50).for_each(|i| {
        let id = make_unique_instance_id(i);
        seed_instance(
            &keyspace,
            &id,
            InstanceStatus::Pending,
            make_test_timestamp(u64::from(i)),
        );
    });

    // Rapid transitions: each instance goes Pending → Running → Completed
    (0u16..50).for_each(|i| {
        let id = make_unique_instance_id(i);
        let ts = make_test_timestamp(u64::from(i));

        instance_index_upsert(
            &keyspace,
            &id,
            InstanceStatus::Running,
            ts,
            Some(InstanceStatus::Pending),
        )
        .unwrap();
        instance_index_upsert(
            &keyspace,
            &id,
            InstanceStatus::Completed,
            ts,
            Some(InstanceStatus::Running),
        )
        .unwrap();
    });

    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(
        all.len(),
        50,
        "After rapid transitions, exactly 50 keys should exist"
    );

    // All should be Completed
    all.iter().for_each(|entry| {
        assert_eq!(entry.status, InstanceStatus::Completed);
    });

    // Pending and Running should be empty
    assert_eq!(
        collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Pending)).len(),
        0
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Running)).len(),
        0
    );
}

// ---------------------------------------------------------------------------
// RQ-IV05: Interleaved transitions across instances — no cross-contamination
// ---------------------------------------------------------------------------

#[test]
fn rq_interleaved_transitions_do_not_cross_contaminate_instances() {
    let (_dir, keyspace) = make_test_keyspace();

    let id_a = make_unique_instance_id(1);
    let id_b = make_unique_instance_id(2);
    let ts_a = make_test_timestamp(100);
    let ts_b = make_test_timestamp(200);

    // Insert both as Pending
    seed_instance(&keyspace, &id_a, InstanceStatus::Pending, ts_a);
    seed_instance(&keyspace, &id_b, InstanceStatus::Pending, ts_b);

    // Interleaved: transition A, then B, then A again
    instance_index_upsert(
        &keyspace,
        &id_a,
        InstanceStatus::Running,
        ts_a,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    instance_index_upsert(
        &keyspace,
        &id_b,
        InstanceStatus::Failed,
        ts_b,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    instance_index_upsert(
        &keyspace,
        &id_a,
        InstanceStatus::Completed,
        ts_a,
        Some(InstanceStatus::Running),
    )
    .unwrap();

    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(all.len(), 2);

    let completed = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Completed));
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].instance_id, id_a);

    let failed = collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Failed));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].instance_id, id_b);

    // No orphaned entries
    assert_eq!(
        collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Pending)).len(),
        0
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&keyspace, InstanceStatus::Running)).len(),
        0
    );
}

// ===========================================================================
// DIMENSION 5: Encode/Decode Edge Cases
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-ED01: Every InstanceStatus variant encodes and decodes correctly
// ---------------------------------------------------------------------------

#[test]
fn rq_all_six_status_variants_encode_decode_round_trip() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(12345);

    (InstanceStatus::all_variants())
        .into_iter()
        .for_each(|status| {
            let key = encode_instance_index_key(*status, ts, &id).unwrap();
            let entry = decode_instance_index_key(&key).unwrap();
            assert_eq!(
                entry.status, *status,
                "Status {:?} failed round-trip",
                status
            );
            assert_eq!(entry.created_at, ts);
            assert_eq!(entry.instance_id, id);
        });
}

// ---------------------------------------------------------------------------
// RQ-ED02: Key bytes are exactly the contract-specified layout
// ---------------------------------------------------------------------------

#[test]
fn rq_key_layout_matches_contract_specification_for_all_statuses() {
    let id_bytes = [0xAB; 16];
    let id = InstanceId::from_bytes(id_bytes);
    let ts_value = 0x0102030405060708u64;
    let ts = make_test_timestamp(ts_value);

    (InstanceStatus::all_variants())
        .into_iter()
        .for_each(|status| {
            let key = encode_instance_index_key(*status, ts, &id).unwrap();

            // Byte 0: status byte
            assert_eq!(key[0], status.to_byte());
            // Bytes 1..9: created_at as big-endian u64
            assert_eq!(&key[1..9], &ts_value.to_be_bytes());
            // Bytes 9..25: instance_id bytes
            assert_eq!(&key[9..25], &id_bytes);
        });
}

// ---------------------------------------------------------------------------
// RQ-ED03: Encode key with different InstanceIds produces different keys
// ---------------------------------------------------------------------------

#[test]
fn rq_different_instance_ids_same_status_and_ts_produce_different_keys() {
    let id1 = InstanceId::from_bytes([0x01; 16]);
    let id2 = InstanceId::from_bytes([0x02; 16]);
    let ts = make_test_timestamp(1000);

    let key1 = encode_instance_index_key(InstanceStatus::Pending, ts, &id1).unwrap();
    let key2 = encode_instance_index_key(InstanceStatus::Pending, ts, &id2).unwrap();

    assert_ne!(key1, key2, "Different IDs must produce different keys");
    // They should differ only at bytes 9..25
    assert_eq!(
        &key1[0..9],
        &key2[0..9],
        "Status and timestamp should be identical"
    );
    assert_ne!(
        &key1[9..25],
        &key2[9..25],
        "Instance ID portion should differ"
    );
}

// ---------------------------------------------------------------------------
// RQ-ED04: Decode handles crafted key with valid status but extreme values
// ---------------------------------------------------------------------------

#[test]
fn rq_decode_handles_extreme_timestamp_and_id_values() {
    // Max everything: status=0x06, ts=u64::MAX, id=[0xFF; 16]
    let mut max_key = [0xFF; 25];
    max_key[0] = 0x06;
    let entry = decode_instance_index_key(&max_key).unwrap();
    assert_eq!(entry.status, InstanceStatus::Cancelled);
    assert_eq!(entry.created_at, make_test_timestamp(u64::MAX));

    // Min everything: status=0x01, ts=0, id=[0x00; 16]
    let mut min_key = [0x00; 25];
    min_key[0] = 0x01;
    let entry = decode_instance_index_key(&min_key).unwrap();
    assert_eq!(entry.status, InstanceStatus::Pending);
    assert_eq!(entry.created_at, make_test_timestamp(0));
}

// ===========================================================================
// DIMENSION 6: Value Slot Verification
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-VS01: After status transition, value remains empty (POST-009)
// ---------------------------------------------------------------------------

#[test]
fn rq_value_is_empty_after_status_transition() {
    let (_dir, keyspace) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    // Initial insert
    seed_instance(&keyspace, &id, InstanceStatus::Pending, ts);

    // Transition
    instance_index_upsert(
        &keyspace,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();
    let new_key = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();
    let raw_value = partition.get(new_key).unwrap().expect("key should exist");
    assert_eq!(
        raw_value.len(),
        0,
        "Value should remain empty after transition"
    );
}

// ---------------------------------------------------------------------------
// RQ-VS02: After idempotent upsert, value remains empty
// ---------------------------------------------------------------------------

#[test]
fn rq_value_is_empty_after_idempotent_upsert() {
    let (_dir, keyspace) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    seed_instance(&keyspace, &id, InstanceStatus::Pending, ts);

    // Idempotent re-insert
    instance_index_upsert(
        &keyspace,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let raw_value = partition.get(key).unwrap().expect("key should exist");
    assert_eq!(
        raw_value.len(),
        0,
        "Value should remain empty after idempotent upsert"
    );
}

// ===========================================================================
// DIMENSION 7: scan_all_instances with prefix([]) correctness
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-SA01: scan_all finds keys with every valid status byte
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_finds_entries_across_all_six_status_buckets() {
    let (_dir, keyspace) = make_test_keyspace();

    (InstanceStatus::all_variants().iter().enumerate())
        .into_iter()
        .for_each(|(i, status)| {
            let id = make_unique_instance_id(i as u16);
            seed_instance(&keyspace, &id, *status, make_test_timestamp(i as u64));
        });

    let all = collect_scan_ok(scan_all_instances(&keyspace));
    assert_eq!(
        all.len(),
        6,
        "scan_all should find entries in all 6 status buckets"
    );

    // Verify all 6 statuses are represented
    let found_statuses: Vec<_> = all.iter().map(|e| e.status).collect();
    (InstanceStatus::all_variants())
        .into_iter()
        .for_each(|status| {
            assert!(
                found_statuses.contains(status),
                "scan_all missing status {:?}",
                status
            );
        });
}

// ---------------------------------------------------------------------------
// RQ-SA02: scan_all with mixed corrupt and valid keys yields correct results
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_with_mixed_corrupt_and_valid_keys_yields_errors_and_entries() {
    let (_dir, keyspace) = make_test_keyspace();
    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .unwrap();

    // Inject: 1 key with invalid status (0x00), 1 valid key, 1 key with invalid length
    let corrupt_status_key = [0x00u8; 25];
    partition.insert(corrupt_status_key, &[] as &[u8]).unwrap();

    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);
    seed_instance(&keyspace, &id, InstanceStatus::Pending, ts);

    // Short key with prefix 0x03 (Paused range)
    let short_key = [0x03u8; 10];
    partition.insert(short_key, &[] as &[u8]).unwrap();

    let results: Vec<_> = scan_all_instances(&keyspace).collect();
    assert_eq!(
        results.len(),
        3,
        "Should yield 3 items (2 corrupt + 1 valid)"
    );

    let corrupt_count = results
        .iter()
        .filter(|r| matches!(r, Err(StorageError::CorruptKey)))
        .count();
    let ok_count = results.len() - corrupt_count;

    assert_eq!(corrupt_count, 2, "Should have 2 corrupt entries");
    assert_eq!(ok_count, 1, "Should have 1 valid entry");
}

// ===========================================================================
// DIMENSION 8: Proptest — Adversarial Random Inputs
// ===========================================================================

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
        #![proptest_config(ProptestConfig::with_cases(50))]

        // RQ-PT01: Arbitrary key bytes of length != 25 always rejected
        #[test]
        fn rq_proptest_arbitrary_length_rejected(
            len in (0usize..100).prop_filter("not 25", |l| *l != 25),
            fill in proptest::num::u8::ANY,
        ) {
            let key = vec![fill; len];
            prop_assert_eq!(decode_instance_index_key(&key), Err(StorageError::CorruptKey));
        }

        // RQ-PT02: Upsert then scan yields exactly 1 entry per unique instance
        #[test]
        fn rq_proptest_upsert_then_scan_yields_one_entry(
            status in arb_instance_status(),
            ts in proptest::num::u64::ANY,
            id_bytes in arb_instance_id_bytes(),
        ) {
            let (_dir, keyspace) = make_test_keyspace();
            let id = InstanceId::from_bytes(id_bytes);
            let timestamp = TimestampMs::try_from(ts).unwrap();

            instance_index_upsert(&keyspace, &id, status, timestamp, None).unwrap();

            let all = collect_scan_ok(scan_all_instances(&keyspace));
            prop_assert_eq!(all.len(), 1);
            prop_assert_eq!(all[0].status, status);
            prop_assert_eq!(all[0].created_at, timestamp);
            prop_assert_eq!(all[0].instance_id.clone(), id);
        }

        // RQ-PT03: Status transition always leaves exactly 1 key
        #[test]
        fn rq_proptest_status_transition_leaves_one_key(
            old_status in arb_instance_status(),
            new_status in arb_instance_status(),
            ts in proptest::num::u64::ANY,
            id_bytes in arb_instance_id_bytes(),
        ) {
            let (_dir, keyspace) = make_test_keyspace();
            let id = InstanceId::from_bytes(id_bytes);
            let timestamp = TimestampMs::try_from(ts).unwrap();

            // Insert with old status
            instance_index_upsert(&keyspace, &id, old_status, timestamp, None).unwrap();

            // Transition to new status
            instance_index_upsert(&keyspace, &id, new_status, timestamp, Some(old_status)).unwrap();

            let all = collect_scan_ok(scan_all_instances(&keyspace));
            prop_assert_eq!(all.len(), 1, "After transition, exactly 1 key should exist");
            prop_assert_eq!(all[0].status, new_status);
        }

        // RQ-PT04: Lexicographic key order = chronological order within same status
        #[test]
        fn rq_proptest_key_ordering_within_status(
            status in arb_instance_status(),
            t1 in 0u64..u64::MAX,
            id_bytes in arb_instance_id_bytes(),
        ) {
            let t2 = t1 + 1;
            let id = InstanceId::from_bytes(id_bytes);
            let ts1 = TimestampMs::try_from(t1).unwrap();
            let ts2 = TimestampMs::try_from(t2).unwrap();

            let key1 = encode_instance_index_key(status, ts1, &id).unwrap();
            let key2 = encode_instance_index_key(status, ts2, &id).unwrap();

            prop_assert!(key1 < key2, "key(t1={t1}) should be < key(t2={t2})");
        }

        // RQ-PT05: Different statuses produce different first bytes
        #[test]
        fn rq_proptest_different_statuses_different_first_byte(
            s1 in arb_instance_status(),
            s2 in arb_instance_status(),
            ts in proptest::num::u64::ANY,
            id_bytes in arb_instance_id_bytes(),
        ) {
            prop_assume!(s1 != s2);
            let id = InstanceId::from_bytes(id_bytes);
            let timestamp = TimestampMs::try_from(ts).unwrap();

            let key1 = encode_instance_index_key(s1, timestamp, &id).unwrap();
            let key2 = encode_instance_index_key(s2, timestamp, &id).unwrap();

            prop_assert_ne!(key1[0], key2[0], "Different statuses must produce different first bytes");
        }
    }
}
