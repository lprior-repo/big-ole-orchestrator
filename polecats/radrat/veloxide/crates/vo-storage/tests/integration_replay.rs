//! Integration tests for the event replay query engine.
//!
//! Tests exercise `replay_events` with a real fjall keyspace, verifying
//! sequential replay, gap detection, corrupt payloads, and boundary conditions.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::pedantic)]

use fjall::{Config, PartitionCreateOptions};
use vo_storage::codec::StorageError;
use vo_storage::query::epoch_prefix_generator;
use vo_storage::query::lineage_prefix_generator;
use vo_storage::query::optimizer::{
    OptimizedReplayIterator, Projection, QueryOptimizer, QuerySpec,
};
use vo_storage::query::replay_events;
use vo_storage::query::replay_events_for_lineage;
use vo_storage::query::LineageQuery;
use vo_storage::query::LINEAGE_ID_NULL_BYTE;
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

fn make_unsupported_version_envelope_json(instance_id: &str) -> Vec<u8> {
    serde_json::json!({
        "version": 99,
        "instance_id": instance_id,
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
    let instance_id_string = ulid::Ulid::new().to_string();
    let instance_id = parse_instance_id(&instance_id_string);
    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert!(results.is_empty());
}

#[test]
fn replay_events_returns_single_event_in_order() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
    let bad_value = make_unsupported_version_envelope_json(instance_id_str);
    insert_event(&partition, instance_id_str, 1, &bad_value);

    let iter = replay_events(&keyspace, &instance_id);
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err(StorageError::UnsupportedVersion));
}

#[test]
fn replay_events_isolates_different_instances() {
    let (_dir, keyspace) = setup_keyspace();
    let id_a_string = ulid::Ulid::new().to_string();
    let id_a = id_a_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
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
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();
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

fn make_envelope_json_with_version(seq: u64, instance_id: &str, version: u8) -> Vec<u8> {
    serde_json::json!({
        "version": version,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": 1000 + seq,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1"},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

fn make_envelope_json_with_timestamp(seq: u64, instance_id: &str, timestamp_ms: u64) -> Vec<u8> {
    serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": timestamp_ms,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1"},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

#[test]
fn optimized_replay_iterator_with_limit() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=10u64 {
        let value = make_envelope_json(seq, instance_id_str);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![],
        projection: Projection::Full,
        limit: Some(3),
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 3);
}

#[test]
fn optimized_replay_iterator_with_offset() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=10u64 {
        let value = make_envelope_json(seq, instance_id_str);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![],
        projection: Projection::Full,
        limit: None,
        offset: 7,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].as_ref().unwrap().sequence, 8);
    assert_eq!(results[1].as_ref().unwrap().sequence, 9);
    assert_eq!(results[2].as_ref().unwrap().sequence, 10);
}

#[test]
fn optimized_replay_iterator_with_sequence_range() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=20u64 {
        let value = make_envelope_json(seq, instance_id_str);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    use vo_storage::query::optimizer::Predicate;
    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![Predicate::SequenceRange { min: 5, max: 15 }],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    assert!(
        plan.scan_range_start.is_some(),
        "SequenceRange predicate should produce scan_range_start"
    );
    assert!(
        plan.scan_range_end.is_some(),
        "SequenceRange predicate should produce scan_range_end"
    );
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 11, "Should return sequences 5-15 inclusive");
    for (i, result) in results.iter().enumerate() {
        let env = result.as_ref().unwrap();
        assert_eq!(
            env.sequence,
            (5 + i) as u64,
            "Sequence {} should be at index {}",
            5 + i,
            i
        );
    }
}

#[test]
fn optimized_replay_iterator_with_event_type_predicate() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    let event_types = [
        "WorkflowStarted",
        "StepCompleted",
        "WorkflowStarted",
        "StepCompleted",
        "WorkflowStarted",
    ];
    for (i, event_type) in event_types.iter().enumerate() {
        let seq = i as u64 + 1;
        let value = serde_json::json!({
            "version": 1,
            "instance_id": instance_id_str,
            "sequence": seq,
            "timestamp_ms": 1000 + seq,
            "payload": {"type": event_type, "workflow_id": "wf-1"},
            "metadata": {}
        })
        .to_string()
        .into_bytes();
        insert_event(&partition, instance_id_str, seq, &value);
    }

    use vo_storage::query::optimizer::Predicate;
    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![Predicate::EventType("WorkflowStarted".to_string())],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 3);
    for result in &results {
        let env = result.as_ref().unwrap();
        assert_eq!(
            env.payload.get("type").unwrap().as_str().unwrap(),
            "WorkflowStarted"
        );
    }
}

#[test]
fn optimized_replay_iterator_empty_when_no_matching_events() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=5u64 {
        let value = make_envelope_json(seq, instance_id_str);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    use vo_storage::query::optimizer::Predicate;
    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![Predicate::EventType("NonExistentEvent".to_string())],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert!(results.is_empty());
}

#[test]
fn optimized_replay_iterator_with_schema_version_predicate() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=5u64 {
        let version = if seq % 2 == 0 { 1 } else { 0 };
        let value = make_envelope_json_with_version(seq, instance_id_str, version);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    use vo_storage::query::optimizer::Predicate;
    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![Predicate::SchemaVersion(1)],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(
        results.len(),
        2,
        "Expected 2 events with schema_version=1, got {}",
        results.len()
    );
    for result in &results {
        let env = result.as_ref().unwrap();
        assert_eq!(env.schema_version, 1);
    }
}

#[test]
fn optimized_replay_iterator_with_timestamp_range_predicate() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(&id_string);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=10u64 {
        let timestamp_ms = 1000 + (seq * 100);
        let value = make_envelope_json_with_timestamp(seq, instance_id_str, timestamp_ms);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    use vo_storage::query::optimizer::Predicate;
    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![Predicate::TimestampRange {
            min_ms: 1100,
            max_ms: 1300,
        }],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(
        results.len(),
        3,
        "Expected 3 events with timestamp 1100-1300, got {}",
        results.len()
    );
    for result in &results {
        let env = result.as_ref().unwrap();
        assert!(env.timestamp_ms >= 1100);
        assert!(env.timestamp_ms <= 1300);
    }
}

#[test]
fn optimized_replay_iterator_combined_predicates_and_limit() {
    let (_dir, keyspace) = setup_keyspace();
    let id_string = ulid::Ulid::new().to_string();
    let instance_id_str = id_string.as_str();
    let instance_id = parse_instance_id(instance_id_str);
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    for seq in 1..=20u64 {
        let value = make_envelope_json(seq, instance_id_str);
        insert_event(&partition, instance_id_str, seq, &value);
    }

    use vo_storage::query::optimizer::Predicate;
    let spec = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id),
        predicates: vec![Predicate::EventType("WorkflowStarted".to_string())],
        projection: Projection::Full,
        limit: Some(5),
        offset: 0,
    };
    let plan = QueryOptimizer::optimize(spec);
    let iter = OptimizedReplayIterator::from_plan(&plan, &keyspace).expect("valid plan");
    let results: Vec<_> = iter.collect();
    assert_eq!(results.len(), 5);
}

#[test]
fn optimized_replay_iterator_isolates_different_instances() {
    let (_dir, keyspace) = setup_keyspace();
    let id_a_string = ulid::Ulid::new().to_string();
    let id_a = id_a_string.as_str();
    let id_b = "01H5JYV4XHGSR2F8KZ9BWNRFMB";
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .expect("partition");
    let a1 = make_envelope_json(1, id_a);
    let a2 = make_envelope_json(2, id_a);
    let b1 = make_envelope_json(1, id_b);
    let b2 = make_envelope_json(2, id_b);
    insert_event(&partition, id_a, 1, &a1);
    insert_event(&partition, id_a, 2, &a2);
    insert_event(&partition, id_b, 1, &b1);
    insert_event(&partition, id_b, 2, &b2);

    let instance_id_a = parse_instance_id(id_a);
    let spec_a = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id_a),
        predicates: vec![],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan_a = QueryOptimizer::optimize(spec_a);
    let iter_a = OptimizedReplayIterator::from_plan(&plan_a, &keyspace).expect("valid plan");
    let results_a: Vec<_> = iter_a.collect();
    assert_eq!(results_a.len(), 2);

    let instance_id_b = parse_instance_id(id_b);
    let spec_b = QuerySpec {
        lineage_query: LineageQuery::InstanceId(&instance_id_b),
        predicates: vec![],
        projection: Projection::Full,
        limit: None,
        offset: 0,
    };
    let plan_b = QueryOptimizer::optimize(spec_b);
    let iter_b = OptimizedReplayIterator::from_plan(&plan_b, &keyspace).expect("valid plan");
    let results_b: Vec<_> = iter_b.collect();
    assert_eq!(results_b.len(), 2);
}

fn insert_lineage_event(
    partition: &fjall::PartitionHandle,
    lineage_id: &str,
    epoch: u64,
    seq: u64,
    value: &[u8],
) {
    let lineage_prefix = lineage_prefix_generator(lineage_id).unwrap();
    let epoch_bytes = epoch.to_be_bytes();
    let mut key = lineage_prefix;
    key.extend_from_slice(&epoch_bytes);
    key.extend_from_slice(&seq.to_be_bytes());
    partition.insert(&key, value).unwrap();
}

#[test]
fn lineage_wide_query_returns_events_across_all_epochs() {
    let (_dir, keyspace) = setup_keyspace();
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();

    let lineage_id = "wf-lineage-42";
    let instance_id_str = ulid::Ulid::new().to_string();

    insert_lineage_event(
        &partition,
        lineage_id,
        1,
        1,
        &make_envelope_json(1, &instance_id_str),
    );
    insert_lineage_event(
        &partition,
        lineage_id,
        1,
        2,
        &make_envelope_json(2, &instance_id_str),
    );
    insert_lineage_event(
        &partition,
        lineage_id,
        2,
        1,
        &make_envelope_json(1, &instance_id_str),
    );

    let query = LineageQuery::LineageWide { lineage_id };
    let iter = replay_events_for_lineage(&keyspace, &query);
    let results: Vec<_> = iter.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn epoch_specific_query_returns_events_only_for_target_epoch() {
    let (_dir, keyspace) = setup_keyspace();
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();

    let lineage_id = "wf-lineage-99";
    let instance_id_str = ulid::Ulid::new().to_string();

    insert_lineage_event(
        &partition,
        lineage_id,
        1,
        1,
        &make_envelope_json(1, &instance_id_str),
    );
    insert_lineage_event(
        &partition,
        lineage_id,
        1,
        2,
        &make_envelope_json(2, &instance_id_str),
    );
    insert_lineage_event(
        &partition,
        lineage_id,
        2,
        1,
        &make_envelope_json(1, &instance_id_str),
    );

    let query = LineageQuery::EpochSpecific {
        lineage_id,
        epoch: vo_types::Epoch::new(1),
    };
    let iter = replay_events_for_lineage(&keyspace, &query);
    let results: Vec<_> = iter.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].sequence, 1);
    assert_eq!(results[1].sequence, 2);
}

#[test]
fn lineage_wide_query_returns_empty_for_nonexistent_lineage() {
    let (_dir, keyspace) = setup_keyspace();

    let query = LineageQuery::LineageWide {
        lineage_id: "no-such-lineage",
    };
    let iter = replay_events_for_lineage(&keyspace, &query);
    let results: Vec<_> = iter.collect();
    assert!(results.is_empty());
}

#[test]
fn epoch_specific_query_returns_empty_for_nonexistent_epoch() {
    let (_dir, keyspace) = setup_keyspace();
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();

    let lineage_id = "wf-lineage-empty";
    let instance_id_str = ulid::Ulid::new().to_string();
    insert_lineage_event(
        &partition,
        lineage_id,
        1,
        1,
        &make_envelope_json(1, &instance_id_str),
    );

    let query = LineageQuery::EpochSpecific {
        lineage_id,
        epoch: vo_types::Epoch::new(99),
    };
    let iter = replay_events_for_lineage(&keyspace, &query);
    let results: Vec<_> = iter.collect();
    assert!(results.is_empty());
}

#[test]
fn lineage_wide_query_does_not_return_instance_id_events() {
    let (_dir, keyspace) = setup_keyspace();
    let partition = keyspace
        .open_partition("events", PartitionCreateOptions::default())
        .unwrap();

    let instance_id_str = ulid::Ulid::new().to_string();
    insert_event(
        &partition,
        &instance_id_str,
        1,
        &make_envelope_json(1, &instance_id_str),
    );

    let query = LineageQuery::LineageWide {
        lineage_id: "different-lineage",
    };
    let iter = replay_events_for_lineage(&keyspace, &query);
    let results: Vec<_> = iter.collect();
    assert!(results.is_empty());
}
