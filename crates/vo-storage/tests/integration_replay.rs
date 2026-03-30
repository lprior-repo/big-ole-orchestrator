//! Integration tests for the event replay query engine.
//!
//! Tests exercise `replay_events` with a real fjall keyspace, verifying
//! sequential replay, gap detection, corrupt payloads, and boundary conditions.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::pedantic)]

use fjall::{Config, PartitionCreateOptions};
use vo_storage::query::{replay_events, StorageError};
use vo_types::InstanceId;

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
    assert!(results[0].is_ok());
    assert_eq!(results[0].as_ref().unwrap().sequence, 1);
}

#[test]
fn replay_events_returns_multiple_events_in_sequence() {
    let (_dir, keyspace) = setup_keyspace();
    let instance_id_str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    for seq in 1..=5u64 {
        let value = make_envelope_json(seq, instance_id_str);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 5);
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok(), "Event {} should be Ok", i);
        assert_eq!(result.as_ref().unwrap().sequence, (i + 1) as u64);
    }
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
    assert!(results[0].is_ok());
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
    for seq in 1..=3u64 {
        insert_event(&partition, id_a, seq, &make_envelope_json(seq, id_a));
    }
    for seq in 1..=2u64 {
        insert_event(&partition, id_b, seq, &make_envelope_json(seq, id_b));
    }

    let instance_id_a = parse_instance_id(id_a);
    let iter_a = replay_events(&keyspace, &instance_id_a);
    let results_a: Vec<_> = iter_a.collect();
    assert_eq!(results_a.len(), 3);

    let instance_id_b = parse_instance_id(id_b);
    let iter_b = replay_events(&keyspace, &instance_id_b);
    let results_b: Vec<_> = iter_b.collect();
    assert_eq!(results_b.len(), 2);
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
    assert!(results[0].is_ok());
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
    for seq in 10..=12u64 {
        insert_event(
            &partition,
            instance_id_str,
            seq,
            &make_envelope_json(seq, instance_id_str),
        );
    }

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].as_ref().unwrap().sequence, 10);
    assert_eq!(results[1].as_ref().unwrap().sequence, 11);
    assert_eq!(results[2].as_ref().unwrap().sequence, 12);
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
    assert!(results[0].is_ok());
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
    for seq in seq_start..seq_start + 5 {
        insert_event(
            &partition,
            instance_id_str,
            seq,
            &make_envelope_json(seq, instance_id_str),
        );
    }

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 5);
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok());
        assert_eq!(result.as_ref().unwrap().sequence, seq_start + i as u64);
    }
}
