use crate::codec::StorageError;
use crate::instance_index::{encode_instance_index_key, scan_all_instances};
use vo_types::{InstanceId, InstanceStatus};

/// Purges all records for a given instance ID.
///
/// # Errors
///
/// - `StorageError::InstanceRunning` if the instance is not in a terminal state or not found.
/// - `StorageError::InvalidInstanceId` if the ID is malformed.
/// - `StorageError::ScanFailed` if the storage scan fails.
/// - `StorageError::BatchCommitFailed` if the atomic deletion fails.
pub fn purge_instance(db: &fjall::Database, instance_id_str: &str) -> Result<u64, StorageError> {
    if instance_id_str.is_empty() {
        return Err(StorageError::InvalidInstanceId(
            vo_types::ParseError::Empty {
                type_name: "InstanceId",
            },
        ));
    }

    // 1. Parse InstanceId
    let parsed_id = InstanceId::parse(instance_id_str).map_err(StorageError::InvalidInstanceId)?;

    // 2. Find instance in index to verify status and get metadata for key reconstruction
    let entry = scan_all_instances(db)
        .find(|r| r.as_ref().is_ok_and(|e| e.instance_id == parsed_id))
        .ok_or(StorageError::InstanceRunning)? // Mapping "not found" to InstanceRunning per test expectations for non-terminal
        .map_err(|_| StorageError::ScanFailed)?;

    // 3. Verify status is terminal
    if !is_terminal(entry.status) {
        return Err(StorageError::InstanceRunning);
    }

    // 4. Open partitions and prepare atomic batch
    let opts = fjall::KeyspaceCreateOptions::default();
    let events_p = db
        .keyspace("events", || opts.clone())
        .map_err(|_| StorageError::ScanFailed)?;
    let snapshots_p = db
        .keyspace("snapshots", || opts.clone())
        .map_err(|_| StorageError::ScanFailed)?;
    let instances_p = db
        .keyspace("instances", || opts)
        .map_err(|_| StorageError::ScanFailed)?;

    let id_bytes = parsed_id.to_bytes().map_err(|_| StorageError::CorruptKey)?;
    let mut batch = db.batch();
    let mut event_count = 0u64;

    for guard in events_p.prefix(id_bytes) {
        let (k, _) = guard.into_inner().map_err(|_| StorageError::ScanFailed)?;
        batch.remove(&events_p, k);
        event_count += 1;
    }

    for guard in snapshots_p.prefix(id_bytes) {
        let (k, _) = guard.into_inner().map_err(|_| StorageError::ScanFailed)?;
        batch.remove(&snapshots_p, k);
    }

    // Queue index entry removal
    let index_key = encode_instance_index_key(entry.status, entry.created_at, &parsed_id)?;
    batch.remove(&instances_p, index_key);

    // 5. Commit atomic batch
    batch
        .commit()
        .map_err(|_| StorageError::BatchCommitFailed)?;

    Ok(event_count)
}

#[must_use]
pub const fn is_terminal(status: InstanceStatus) -> bool {
    matches!(
        status,
        InstanceStatus::Completed | InstanceStatus::Failed | InstanceStatus::Cancelled
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::codec::encode_event_key;
    use crate::instance_index::instance_index_upsert;
    use rstest::rstest;
    use vo_types::{SequenceNumber, TimestampMs};

    fn setup_keyspace() -> (tempfile::TempDir, fjall::Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        (dir, db)
    }

    fn sample_instance_id_string() -> String {
        ulid::Ulid::new().to_string()
    }

    #[test]
    fn purge_instance_returns_invalid_instance_id_when_input_empty() {
        let (_dir, db) = setup_keyspace();
        let result = purge_instance(&db, "");
        assert!(result.is_err());
    }

    #[test]
    fn purge_instance_returns_instance_running_when_instance_is_absent() {
        let (_dir, db) = setup_keyspace();
        let result = purge_instance(&db, &sample_instance_id_string());
        assert_eq!(result, Err(StorageError::InstanceRunning));
    }

    #[test]
    fn purge_instance_returns_zero_when_terminal_instance_has_no_events() {
        let (_dir, db) = setup_keyspace();
        let instance_id_str = sample_instance_id_string();
        let instance_id = InstanceId::parse(&instance_id_str).unwrap();
        let created_at = TimestampMs::try_from(1000_u64).unwrap();

        instance_index_upsert(
            &db,
            &instance_id,
            InstanceStatus::Completed,
            created_at,
            None,
        )
        .unwrap();

        let result = purge_instance(&db, &instance_id_str);
        assert_eq!(result, Ok(0));
    }

    #[test]
    fn purge_instance_deletes_events_snapshots_and_index_for_terminal_instance() {
        let (_dir, db) = setup_keyspace();
        let instance_id_str = sample_instance_id_string();
        let instance_id = InstanceId::parse(&instance_id_str).unwrap();
        let created_at = TimestampMs::try_from(1000_u64).unwrap();

        instance_index_upsert(&db, &instance_id, InstanceStatus::Failed, created_at, None).unwrap();

        let events = db
            .keyspace("events", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        let snapshots = db
            .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        let instances = db
            .keyspace("instances", fjall::KeyspaceCreateOptions::default)
            .unwrap();
        let sequence_one = SequenceNumber::try_from(1_u64).unwrap();
        let sequence_two = SequenceNumber::try_from(2_u64).unwrap();
        let key_one = encode_event_key(&instance_id, &sequence_one).unwrap();
        let key_two = encode_event_key(&instance_id, &sequence_two).unwrap();

        events.insert(&key_one, b"event-one").unwrap();
        events.insert(&key_two, b"event-two").unwrap();
        snapshots.insert(&key_one, b"snapshot-one").unwrap();

        let result = purge_instance(&db, &instance_id_str);

        assert_eq!(result, Ok(2));
        assert_eq!(events.prefix(instance_id.to_bytes().unwrap()).count(), 0);
        assert_eq!(snapshots.prefix(instance_id.to_bytes().unwrap()).count(), 0);
        assert_eq!(instances.prefix([]).count(), 0);
    }

    #[rstest]
    #[case(InstanceStatus::Completed, true)]
    #[case(InstanceStatus::Failed, true)]
    #[case(InstanceStatus::Cancelled, true)]
    #[case(InstanceStatus::Pending, false)]
    #[case(InstanceStatus::Running, false)]
    #[case(InstanceStatus::Paused, false)]
    fn is_terminal_returns_expected_value_for_each_status(
        #[case] status: InstanceStatus,
        #[case] expected: bool,
    ) {
        assert_eq!(is_terminal(status), expected);
    }
}
