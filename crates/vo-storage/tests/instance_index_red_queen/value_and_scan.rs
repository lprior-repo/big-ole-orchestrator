//! Red Queen tests: Value slot verification and scan_all correctness.

use vo_storage::codec::StorageError;
use vo_storage::instance_index::{scan_all_instances, scan_by_status, InstanceStatus};

use crate::instance_index_red_queen::helpers::*;

// ---------------------------------------------------------------------------
// RQ-VS01: After status transition, value remains empty (POST-009)
// ---------------------------------------------------------------------------

#[test]
fn rq_value_is_empty_after_status_transition() {
    use vo_storage::instance_index::{encode_instance_index_key, instance_index_upsert};

    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
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
    use vo_storage::instance_index::{encode_instance_index_key, instance_index_upsert};

    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();

    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let raw_value = partition.get(key).unwrap().expect("key should exist");
    assert_eq!(
        raw_value.len(),
        0,
        "Value should remain empty after idempotent upsert"
    );
}

// ---------------------------------------------------------------------------
// RQ-SA01: scan_all finds keys with every valid status byte
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_finds_entries_across_all_six_status_buckets() {
    let (_dir, database) = make_test_keyspace();

    (InstanceStatus::all_variants().iter().enumerate()).into_iter().for_each(|(i, status)| {
        let id = make_unique_instance_id(i as u16);
        seed_instance(&database, &id, *status, make_test_timestamp(i as u64));
    });

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(
        all.len(),
        6,
        "scan_all should find entries in all 6 status buckets"
    );

    let found_statuses: Vec<_> = all.iter().map(|e| e.status).collect();
    (InstanceStatus::all_variants()).into_iter().for_each(|status| {
        assert!(found_statuses.contains(&status), "scan_all missing status {:?}", status);
    });
}

// ---------------------------------------------------------------------------
// RQ-SA02: scan_all with mixed corrupt and valid keys yields correct results
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_with_mixed_corrupt_and_valid_keys_yields_errors_and_entries() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let corrupt_status_key = [0x00u8; 25];
    partition.insert(corrupt_status_key, &[] as &[u8]).unwrap();

    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);
    seed_instance(&database, &id, InstanceStatus::Pending, ts);

    let short_key = [0x03u8; 10];
    partition.insert(short_key, &[] as &[u8]).unwrap();

    let results: Vec<_> = scan_all_instances(&database).collect();
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
