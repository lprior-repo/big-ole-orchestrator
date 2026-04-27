#![allow(clippy::unwrap_used)]

use vo_storage::codec::StorageError;
use vo_storage::instance_index::{scan_all_instances, scan_by_status};

use super::helpers::*;

#[test]
fn rq_scan_all_finds_entries_across_all_six_status_buckets() {
    let (_dir, database) = make_test_keyspace();

    (InstanceStatus::all_variants().iter().enumerate())
        .into_iter()
        .for_each(|(i, status)| {
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