use crate::helpers::{make_test_instance_id, make_test_timestamp, seed_instance};
use vo_storage::instance_index::{
    encode_instance_index_key, scan_all_instances, scan_by_status, instance_index_upsert,
};

#[test]
fn rq_value_is_empty_after_status_transition() {
    let (_dir, database) = crate::helpers::make_test_keyspace();
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
    let (_dir, database) = crate::helpers::make_test_keyspace();
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