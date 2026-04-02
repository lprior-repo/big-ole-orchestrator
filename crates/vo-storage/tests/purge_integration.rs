use vo_storage::codec::encode_event_key;
use vo_storage::codec::StorageError;
use vo_storage::instance_index::instance_index_upsert;
use vo_storage::purge::purge_instance;
use vo_types::{InstanceId, InstanceStatus, SequenceNumber, TimestampMs};

#[test]
fn purge_terminal_instance_deletes_all_records() {
    let temp_dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(temp_dir.path()).open().unwrap();

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let ts = TimestampMs::try_from(1000u64).unwrap();

    // Setup: instances index entry
    instance_index_upsert(&keyspace, &instance_id, InstanceStatus::Completed, ts, None).unwrap();

    // Setup: 3 events
    let events_p = keyspace
        .open_partition("events", Default::default())
        .unwrap();
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let key1 = encode_event_key(&instance_id, &seq1).unwrap();
    events_p.insert(key1, b"event-data").unwrap();

    let seq2 = SequenceNumber::try_from(2u64).unwrap();
    let key2 = encode_event_key(&instance_id, &seq2).unwrap();
    events_p.insert(key2, b"event-data").unwrap();

    let seq3 = SequenceNumber::try_from(3u64).unwrap();
    let key3 = encode_event_key(&instance_id, &seq3).unwrap();
    events_p.insert(key3, b"event-data").unwrap();

    // Setup: 2 snapshots
    let snapshots_p = keyspace
        .open_partition("snapshots", Default::default())
        .unwrap();
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let key1 = encode_event_key(&instance_id, &seq1).unwrap();
    snapshots_p.insert(key1, b"snapshot-data").unwrap();

    let seq2 = SequenceNumber::try_from(2u64).unwrap();
    let key2 = encode_event_key(&instance_id, &seq2).unwrap();
    snapshots_p.insert(key2, b"snapshot-data").unwrap();

    // Execute
    let result = purge_instance(&keyspace, "01H5JYV4XHGSR2F8KZ9BWNRFMA");

    // Verify
    assert_eq!(result, Ok(3)); // 3 events purged

    // Verify everything is gone
    assert_eq!(events_p.prefix(instance_id.to_bytes().unwrap()).count(), 0);
    assert_eq!(
        snapshots_p.prefix(instance_id.to_bytes().unwrap()).count(),
        0
    );

    let instances_p = keyspace
        .open_partition("instances", Default::default())
        .unwrap();
    assert_eq!(instances_p.prefix([]).count(), 0);
}

#[test]
fn purge_running_instance_fails() {
    let temp_dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(temp_dir.path()).open().unwrap();

    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let ts = TimestampMs::try_from(1000u64).unwrap();

    // Setup: running instance
    instance_index_upsert(&keyspace, &instance_id, InstanceStatus::Running, ts, None).unwrap();

    // Execute
    let result = purge_instance(&keyspace, "01H5JYV4XHGSR2F8KZ9BWNRFMA");

    // Verify
    assert_eq!(result, Err(StorageError::InstanceRunning));
}
