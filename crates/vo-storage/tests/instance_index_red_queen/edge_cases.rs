#![allow(clippy::unwrap_used)]

use vo_storage::instance_index::{encode_instance_index_key, scan_all_instances, scan_by_status};

use super::helpers::*;

#[test]
fn rq_u64_max_timestamp_round_trips_through_encode_decode() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(u64::MAX);
    let key = encode_instance_index_key(InstanceStatus::Failed, ts, &id).unwrap();

    assert_eq!(&key[1..9], &[0xFF; 8]);

    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.created_at, ts);
    assert_eq!(entry.instance_id, id);
}

#[test]
fn rq_zero_timestamp_round_trips_through_encode_decode() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(0);
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();

    assert_eq!(&key[1..9], &[0x00; 8]);

    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.created_at, ts);
}

#[test]
fn rq_u64_max_timestamp_sorts_after_all_other_timestamps_in_scan() {
    let (_dir, database) = make_test_keyspace();
    let id_early = make_unique_instance_id(1);
    let id_late = make_unique_instance_id(2);
    let id_max = make_unique_instance_id(3);

    seed_instance(
        &database,
        &id_early,
        InstanceStatus::Pending,
        make_test_timestamp(100),
    );
    seed_instance(
        &database,
        &id_late,
        InstanceStatus::Pending,
        make_test_timestamp(u64::MAX - 1),
    );
    seed_instance(
        &database,
        &id_max,
        InstanceStatus::Pending,
        make_test_timestamp(u64::MAX),
    );

    let entries = collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].created_at, make_test_timestamp(100));
    assert_eq!(entries[1].created_at, make_test_timestamp(u64::MAX - 1));
    assert_eq!(entries[2].created_at, make_test_timestamp(u64::MAX));
}

#[test]
fn rq_same_timestamp_different_ids_produce_deterministic_scan_order() {
    let (_dir, database) = make_test_keyspace();
    let ts = make_test_timestamp(5000);

    let id_low = InstanceId::from_bytes([0x01; 16]);
    let id_mid = InstanceId::from_bytes([0x80; 16]);
    let id_high = InstanceId::from_bytes([0xFF; 16]);

    seed_instance(&database, &id_high, InstanceStatus::Running, ts);
    seed_instance(&database, &id_low, InstanceStatus::Running, ts);
    seed_instance(&database, &id_mid, InstanceStatus::Running, ts);

    let entries = collect_scan_ok(scan_by_status(&database, InstanceStatus::Running));
    assert_eq!(entries.len(), 3);

    assert_eq!(entries[0].instance_id, id_low);
    assert_eq!(entries[1].instance_id, id_mid);
    assert_eq!(entries[2].instance_id, id_high);
}

#[test]
fn rq_all_instances_same_status_returns_all_in_status_scan_none_in_others() {
    let (_dir, database) = make_test_keyspace();

    (0u16..10).into_iter().for_each(|i| {
        let id = make_unique_instance_id(i);
        seed_instance(
            &database,
            &id,
            InstanceStatus::Paused,
            make_test_timestamp(u64::from(i) * 100),
        );
    });

    let paused = collect_scan_ok(scan_by_status(&database, InstanceStatus::Paused));
    assert_eq!(paused.len(), 10);

    InstanceStatus::all_variants()
        .into_iter()
        .for_each(|status| {
            if *status != InstanceStatus::Paused {
                let scan = collect_scan_ok(scan_by_status(&database, *status));
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

#[test]
fn rq_circular_status_transitions_leave_exactly_one_key() {
    let (_dir, database) = make_test_keyspace();
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    instance_index_upsert(&database, &id, InstanceStatus::Pending, ts, None).unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Pending,
        ts,
        Some(InstanceStatus::Running),
    )
    .unwrap();
    assert_eq!(collect_scan_ok(scan_all_instances(&database)).len(), 1);

    instance_index_upsert(
        &database,
        &id,
        InstanceStatus::Running,
        ts,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 1, "After circular transitions, exactly 1 key must exist");
    assert_eq!(all[0].status, InstanceStatus::Running);
}

#[test]
fn rq_max_instance_id_bytes_round_trip() {
    let id = InstanceId::from_bytes([0xFF; 16]);
    let ts = make_test_timestamp(0);
    let key = encode_instance_index_key(InstanceStatus::Cancelled, ts, &id).unwrap();

    assert_eq!(&key[9..25], &[0xFF; 16]);

    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.instance_id, id);
}