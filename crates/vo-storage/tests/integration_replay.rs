//! Integration tests for the event replay query engine.
//!
//! Tests exercise `replay_events` with a real fjall keyspace, verifying
//! sequential replay, gap detection, corrupt payloads, and boundary conditions.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::pedantic)]

use fjall::{Config, PartitionCreateOptions};
use vo_storage::codec::StorageError;
use vo_storage::query::replay_events;
use vo_types::{EventEnvelope, InstanceId};

fn make_envelope_json(seq: u64, instance_id: &str) -> Vec<u8> {
    serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": 1000 + seq,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1"},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

fn make_bad_envelope_json() -> Vec<u8> {
    b"not valid json".to_vec()
}

fn make_unsupported_version_envelope_json() -> Vec<u8> {
    serde_json::json!({
        "version": 99,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

fn insert_event(partition: &fjall::PartitionHandle, instance_id: &str, seq: u64, value: &[u8]) {
    let mut key = instance_id.as_bytes().to_vec();
    key.extend_from_slice(&seq.to_be_bytes());
    partition.insert(&key, value).unwrap();
}

fn setup_keyspace() -> (tempfile::TempDir, fjall::Keyspace) {
    let folder = tempfile::tempdir().expect("temp dir");
    let keyspace = Config::new(folder.path()).open().expect("keyspace");
    keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    (folder, keyspace)
}

fn parse_instance_id(s: &str) -> InstanceId {
    InstanceId::parse(s).expect("valid instance ID")
}

fn parse_envelope(bytes: &[u8]) -> EventEnvelope {
    EventEnvelope::from_bytes(bytes).expect("valid test envelope")
}

#[test]
fn replay_events_returns_empty_iterator_when_no_events_exist() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id = parse_instance_id("01H5JYV4XHGSR2F8KZ9BWNRFMA");
    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert!(results.is_empty());
}

#[test]
fn replay_events_returns_single_event_in_order() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let value = make_envelope_json(1, instance_id_str);
    insert_event(&partition, instance_id_str, 1, &value);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Ok(parse_envelope(&value)));
}

#[test]
fn replay_events_returns_multiple_events_in_sequence() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let value_1 = make_envelope_json(1, instance_id_str);
    let value_2 = make_envelope_json(2, instance_id_str);
    let value_3 = make_envelope_json(3, instance_id_str);
    let value_4 = make_envelope_json(4, instance_id_str);
    let value_5 = make_envelope_json(5, instance_id_str);
    insert_event(&partition, instance_id_str, 1, &value_1);
    insert_event(&partition, instance_id_str, 2, &value_2);
    insert_event(&partition, instance_id_str, 3, &value_3);
    insert_event(&partition, instance_id_str, 4, &value_4);
    insert_event(&partition, instance_id_str, 5, &value_5);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0], Ok(parse_envelope(&value_1)));
    assert_eq!(results[1], Ok(parse_envelope(&value_2)));
    assert_eq!(results[2], Ok(parse_envelope(&value_3)));
    assert_eq!(results[3], Ok(parse_envelope(&value_4)));
    assert_eq!(results[4], Ok(parse_envelope(&value_5)));
}

#[test]
fn replay_events_detects_sequence_gap() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let v1 = make_envelope_json(1, instance_id_str);
    insert_event(&partition, instance_id_str, 1, &v1);
    // skip seq 2
    let v3 = make_envelope_json(3, instance_id_str);
    insert_event(&partition, instance_id_str, 3, &v3);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok(parse_envelope(&v1)));
    assert_eq!(results[1], Err(StorageError::SequenceGap));
}

#[test]
fn replay_events_handles_corrupt_payload() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let bad_value = make_bad_envelope_json();
    insert_event(&partition, instance_id_str, 1, &bad_value);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err(StorageError::CorruptEventPayload));
}

#[test]
fn replay_events_handles_unsupported_version() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let bad_value = make_unsupported_version_envelope_json();
    insert_event(&partition, instance_id_str, 1, &bad_value);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err(StorageError::UnsupportedVersion));
}

#[test]
fn replay_events_isolates_different_instances() {
    let (_dir, keyspace) = setup_keyspace();
    let id_a = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let id_b = "01H5JYV4XHGSR2F8KZ9BWNRFMB";
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let a1 = make_envelope_json(1, id_a);
    let a2 = make_envelope_json(2, id_a);
    let a3 = make_envelope_json(3, id_a);
    let b1 = make_envelope_json(1, id_b);
    let b2 = make_envelope_json(2, id_b);
    insert_event(&partition, id_a, 1, &a1);
    insert_event(&partition, id_a, 2, &a2);
    insert_event(&partition, id_a, 3, &a3);
    insert_event(&partition, id_b, 1, &b1);
    insert_event(&partition, id_b, 2, &b2);

    let instance_id_a = parse_instance_id(id_a);
    let iter_a = replay_events(&keyspace, &instance_id_a);
    let results_a: Vec<_> = iter_a.collect();
    assert_eq!(results_a.len(), 3);
    assert_eq!(results_a[0], Ok(parse_envelope(&a1)));
    assert_eq!(results_a[1], Ok(parse_envelope(&a2)));
    assert_eq!(results_a[2], Ok(parse_envelope(&a3)));

    let instance_id_b = parse_instance_id(id_b);
    let iter_b = replay_events(&keyspace, &instance_id_b);
    let results_b: Vec<_> = iter_b.collect();
    assert_eq!(results_b.len(), 2);
    assert_eq!(results_b[0], Ok(parse_envelope(&b1)));
    assert_eq!(results_b[1], Ok(parse_envelope(&b2)));
}

#[test]
fn replay_events_stops_after_first_error() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let v1 = make_envelope_json(1, instance_id_str);
    insert_event(&partition, instance_id_str, 1, &v1);
    // corrupt event at seq 2
    insert_event(&partition, instance_id_str, 2, &make_bad_envelope_json());
    // valid event at seq 3 that should NOT be reached
    let v3 = make_envelope_json(3, instance_id_str);
    insert_event(&partition, instance_id_str, 3, &v3);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    // First event ok, second corrupt, then iterator terminates
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], Ok(parse_envelope(&v1)));
    assert_eq!(results[1], Err(StorageError::CorruptEventPayload));
}

#[test]
fn replay_events_accepts_non_one_starting_sequence() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    // start from seq 10
    let value_10 = make_envelope_json(10, instance_id_str);
    let value_11 = make_envelope_json(11, instance_id_str);
    let value_12 = make_envelope_json(12, instance_id_str);
    insert_event(&partition, instance_id_str, 10, &value_10);
    insert_event(&partition, instance_id_str, 11, &value_11);
    insert_event(&partition, instance_id_str, 12, &value_12);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0], Ok(parse_envelope(&value_10)));
    assert_eq!(results[1], Ok(parse_envelope(&value_11)));
    assert_eq!(results[2], Ok(parse_envelope(&value_12)));
}

#[test]
fn replay_events_handles_gap_at_start() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    // Starting from seq 5 is fine — iterator accepts any first event
    insert_event(
        &partition,
        instance_id_str,
        5,
        &make_envelope_json(5, instance_id_str),
    );
    insert_event(
        &partition,
        instance_id_str,
        7,
        &make_envelope_json(7, instance_id_str),
    );

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0],
        Ok(parse_envelope(&make_envelope_json(5, instance_id_str)))
    );
    assert_eq!(results[1], Err(StorageError::SequenceGap));
}

#[test]
fn replay_events_handles_large_sequence_range() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    // Insert events with large sequence numbers
    let seq_start = 1_000_000u64;
    let value_1 = make_envelope_json(seq_start, instance_id_str);
    let value_2 = make_envelope_json(seq_start + 1, instance_id_str);
    let value_3 = make_envelope_json(seq_start + 2, instance_id_str);
    let value_4 = make_envelope_json(seq_start + 3, instance_id_str);
    let value_5 = make_envelope_json(seq_start + 4, instance_id_str);
    insert_event(&partition, instance_id_str, seq_start, &value_1);
    insert_event(&partition, instance_id_str, seq_start + 1, &value_2);
    insert_event(&partition, instance_id_str, seq_start + 2, &value_3);
    insert_event(&partition, instance_id_str, seq_start + 3, &value_4);
    insert_event(&partition, instance_id_str, seq_start + 4, &value_5);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0], Ok(parse_envelope(&value_1)));
    assert_eq!(results[1], Ok(parse_envelope(&value_2)));
    assert_eq!(results[2], Ok(parse_envelope(&value_3)));
    assert_eq!(results[3], Ok(parse_envelope(&value_4)));
    assert_eq!(results[4], Ok(parse_envelope(&value_5)));
}
