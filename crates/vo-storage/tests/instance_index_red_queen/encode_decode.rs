#![allow(clippy::unwrap_used)]

use vo_storage::instance_index::{
    decode_instance_index_key, encode_instance_index_key, scan_all_instances, scan_by_status,
};

use super::helpers::*;

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

            assert_eq!(key[0], status.to_byte());
            assert_eq!(&key[1..9], &ts_value.to_be_bytes());
            assert_eq!(&key[9..25], &id_bytes);
        });
}

#[test]
fn rq_different_instance_ids_same_status_and_ts_produce_different_keys() {
    let id1 = InstanceId::from_bytes([0x01; 16]);
    let id2 = InstanceId::from_bytes([0x02; 16]);
    let ts = make_test_timestamp(1000);

    let key1 = encode_instance_index_key(InstanceStatus::Pending, ts, &id1).unwrap();
    let key2 = encode_instance_index_key(InstanceStatus::Pending, ts, &id2).unwrap();

    assert_ne!(key1, key2, "Different IDs must produce different keys");
    assert_eq!(&key1[0..9], &key2[0..9], "Status and timestamp should be identical");
    assert_ne!(&key1[9..25], &key2[9..25], "Instance ID portion should differ");
}

#[test]
fn rq_decode_handles_extreme_timestamp_and_id_values() {
    let mut max_key = [0xFF; 25];
    max_key[0] = 0x06;
    let entry = decode_instance_index_key(&max_key).unwrap();
    assert_eq!(entry.status, InstanceStatus::Cancelled);
    assert_eq!(entry.created_at, make_test_timestamp(u64::MAX));

    let mut min_key = [0x00; 25];
    min_key[0] = 0x01;
    let entry = decode_instance_index_key(&min_key).unwrap();
    assert_eq!(entry.status, InstanceStatus::Pending);
    assert_eq!(entry.created_at, make_test_timestamp(0));
}

#[test]
fn rq_value_is_empty_after_status_transition() {
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
    assert_eq!(raw_value.len(), 0, "Value should remain empty after transition");
}

#[test]
fn rq_value_is_empty_after_idempotent_upsert() {
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
    assert_eq!(raw_value.len(), 0, "Value should remain empty after idempotent upsert");
}