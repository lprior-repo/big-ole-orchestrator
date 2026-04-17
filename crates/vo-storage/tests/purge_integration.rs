use vo_storage::codec::encode_event_key;
use vo_storage::codec::StorageError;
use vo_storage::instance_index::instance_index_upsert;
use vo_storage::purge::{is_terminal, purge_instance};
use vo_types::{InstanceId, InstanceStatus, SequenceNumber, TimestampMs};

#[test]
fn purge_terminal_instance_deletes_all_records() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = ulid::Ulid::new().to_string();
    let instance_id = InstanceId::parse(&instance_id_str).unwrap();
    let ts = TimestampMs::try_from(1000u64).unwrap();

    instance_index_upsert(&db, &instance_id, InstanceStatus::Completed, ts, None).unwrap();

    let events_p = db
        .keyspace("events", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let snapshots_p = db
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
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

    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let key1 = encode_event_key(&instance_id, &seq1).unwrap();
    snapshots_p.insert(key1, b"snapshot-data").unwrap();

    let seq2 = SequenceNumber::try_from(2u64).unwrap();
    let key2 = encode_event_key(&instance_id, &seq2).unwrap();
    snapshots_p.insert(key2, b"snapshot-data").unwrap();

    let result = purge_instance(&db, &instance_id_str);

    assert_eq!(result, Ok(3));

    assert_eq!(events_p.prefix(instance_id.to_bytes().unwrap()).count(), 0);
    assert_eq!(
        snapshots_p.prefix(instance_id.to_bytes().unwrap()).count(),
        0
    );

    let instances_p = db
        .keyspace("instances", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    assert_eq!(instances_p.prefix([]).count(), 0);
}

#[test]
fn purge_running_instance_fails() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = ulid::Ulid::new().to_string();
    let instance_id = InstanceId::parse(&instance_id_str).unwrap();
    let ts = TimestampMs::try_from(1000u64).unwrap();

    instance_index_upsert(&db, &instance_id, InstanceStatus::Running, ts, None).unwrap();

    let result = purge_instance(&db, &instance_id_str);

    assert_eq!(result, Err(StorageError::InstanceRunning));
}

#[test]
fn purge_instance_returns_invalid_instance_id_when_input_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();
    let result = purge_instance(&db, "");
    assert_eq!(
        result,
        Err(StorageError::InvalidInstanceId(
            vo_types::ParseError::Empty {
                type_name: "InstanceId",
            }
        ))
    );
}

#[test]
fn purge_instance_returns_invalid_instance_id_when_input_is_malformed() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();
    let invalid_id = "not-a-ulid";
    let result = purge_instance(&db, invalid_id);
    assert_eq!(
        result,
        Err(StorageError::InvalidInstanceId(
            InstanceId::parse(invalid_id).unwrap_err()
        ))
    );
}

#[test]
fn purge_instance_returns_instance_running_when_instance_is_absent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();
    let instance_id = ulid::Ulid::new().to_string();
    let result = purge_instance(&db, &instance_id);
    assert_eq!(result, Err(StorageError::InstanceRunning));
}

#[test]
fn purge_terminal_instance_returns_zero_when_only_index_entry_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(temp_dir.path()).open().unwrap();

    let instance_id_str = ulid::Ulid::new().to_string();
    let instance_id = InstanceId::parse(&instance_id_str).unwrap();
    let timestamp = TimestampMs::try_from(2000u64).unwrap();

    instance_index_upsert(&db, &instance_id, InstanceStatus::Failed, timestamp, None).unwrap();

    let result = purge_instance(&db, &instance_id_str);

    assert_eq!(result, Ok(0));
}

#[test]
fn is_terminal_returns_true_for_terminal_statuses() {
    assert_eq!(
        (
            is_terminal(InstanceStatus::Completed),
            is_terminal(InstanceStatus::Failed),
            is_terminal(InstanceStatus::Cancelled)
        ),
        (true, true, true)
    );
}

#[test]
fn is_terminal_returns_false_for_non_terminal_statuses() {
    assert_eq!(
        (
            is_terminal(InstanceStatus::Pending),
            is_terminal(InstanceStatus::Running),
            is_terminal(InstanceStatus::Paused)
        ),
        (false, false, false)
    );
}
