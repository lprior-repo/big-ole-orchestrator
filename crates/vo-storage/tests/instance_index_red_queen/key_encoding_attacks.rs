#![allow(clippy::unwrap_used)]

use vo_storage::codec::StorageError;
use vo_storage::instance_index::{scan_all_instances, scan_by_status};

use super::helpers::*;

#[test]
fn rq_max_pending_key_does_not_leak_into_running_scan() {
    let (_dir, database) = make_test_keyspace();

    let id_max = InstanceId::from_bytes([0xFF; 16]);
    let ts_max = make_test_timestamp(u64::MAX);

    seed_instance(&database, &id_max, InstanceStatus::Pending, ts_max);

    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(running.len(), 0, "Max Pending key must NOT appear in Running scan");

    let pending = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert_eq!(pending.len(), 1, "Max Pending key must appear in Pending scan");
}

#[test]
fn rq_min_running_key_does_not_leak_into_pending_scan() {
    let (_dir, database) = make_test_keyspace();

    let id_min = InstanceId::from_bytes([0x01; 16]);
    let ts_zero = make_test_timestamp(0);

    seed_instance(&database, &id_min, InstanceStatus::Running, ts_zero);

    let pending = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert_eq!(pending.len(), 0, "Min Running key must NOT appear in Pending scan");

    let running = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(running.len(), 1, "Min Running key must appear in Running scan");
}

#[test]
fn rq_adjacent_status_boundaries_do_not_cross_contaminate() {
    let (_dir, database) = make_test_keyspace();
    let _id = InstanceId::from_bytes([0xFF; 16]);
    let ts_max = make_test_timestamp(u64::MAX);
    let ts_zero = make_test_timestamp(0);
    let _id_min = InstanceId::from_bytes([0x01; 16]);

    let statuses = InstanceStatus::all_variants();

    (0..statuses.len() - 1).into_iter().for_each(|i| {
        let current = statuses[i];
        let next = statuses[i + 1];

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

        seed_instance(&database, &max_id, current, ts_max);
        seed_instance(&database, &min_id, next, ts_zero);
    });

    InstanceStatus::all_variants()
        .into_iter()
        .for_each(|status| {
            let entries = collect_scan_ok(scan_by_status(&database, *status));
            (&entries).into_iter().for_each(|entry| {
                assert_eq!(
                    entry.status, *status,
                    "Scan for {:?} returned entry with status {:?}",
                    status, entry.status
                );
            });
        });
}

#[test]
fn rq_manually_injected_boundary_key_stays_in_correct_prefix_range() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let max_pending_key = {
        let mut k = [0xFF; 25];
        k[0] = 0x01;
        k
    };
    partition.insert(max_pending_key, &[] as &[u8]).unwrap();

    let min_running_key = {
        let mut k = [0x00; 25];
        k[0] = 0x02;
        k
    };
    partition.insert(min_running_key, &[] as &[u8]).unwrap();

    let pending: Vec<_> = scan_by_status(&database, InstanceStatus::Pending).collect();
    assert_eq!(pending.len(), 1, "Pending scan should find exactly 1 entry");
    assert_eq!(pending[0].as_ref().unwrap().status, InstanceStatus::Pending);

    let running: Vec<_> = scan_by_status(&database, InstanceStatus::Running).collect();
    assert_eq!(running.len(), 1, "Running scan should find exactly 1 entry");
}

#[test]
fn rq_key_with_zero_status_byte_not_returned_by_any_valid_status_scan() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let zero_status_key = [0x00u8; 25];
    partition.insert(zero_status_key, &[] as &[u8]).unwrap();

    InstanceStatus::all_variants()
        .into_iter()
        .for_each(|status| {
            let entries: Vec<_> = scan_by_status(&database, *status).collect();
            assert_eq!(
                entries.len(),
                0,
                "Status {:?} scan should not return key with 0x00 status byte",
                status
            );
        });

    let all: Vec<_> = scan_all_instances(&database).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(
        all[0],
        Err(StorageError::CorruptKey),
        "scan_all should yield CorruptKey for 0x00 status byte"
    );
}

#[test]
fn rq_key_with_0x07_status_byte_not_returned_by_cancelled_scan() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let mut key_above = [0x01u8; 25];
    key_above[0] = 0x07;
    partition.insert(key_above, &[] as &[u8]).unwrap();

    let cancelled: Vec<_> = scan_by_status(&database, InstanceStatus::Cancelled).collect();
    assert_eq!(
        cancelled.len(),
        0,
        "Cancelled scan should not return key with 0x07 status byte"
    );

    let all: Vec<_> = scan_all_instances(&database).collect();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0], Err(StorageError::CorruptKey));
}