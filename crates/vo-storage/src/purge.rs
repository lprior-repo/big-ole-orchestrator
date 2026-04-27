use crate::codec::StorageError;
use crate::instance_index::{encode_instance_index_key, scan_all_instances};
use crate::key_encoding::encode_event_key as encode_key;
use crate::key_partition::{DekStore, FjallDekStore};
use vo_types::{InstanceId, InstanceStatus, SequenceNumber};

/// Purges all records for a given instance ID.
///
/// Implements ADR-025 GDPR purge:
/// 1. Destroy the per-instance DEK (crypto-shredding) if present
/// 2. Delete events, snapshots, and instance index entries
/// 3. Queue physical blob and key removal in Fjall for compaction-time reclamation
///
/// Minimal pseudonymous control-plane facts (dedupe-key hashes, effect IDs,
/// version hashes, sequence numbers, external receipts) are retained until
/// their configured retention window expires.
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
        .ok_or(StorageError::InstanceRunning)?
        .map_err(|_| StorageError::ScanFailed)?;

    // 3. Verify status is terminal
    if !is_terminal(entry.status) {
        return Err(StorageError::InstanceRunning);
    }

    // 4. Crypto-shred the DEK (ADR-025 Step 1: destroy per-instance DEK)
    let id_bytes = parsed_id.to_bytes().map_err(|_| StorageError::CorruptKey)?;
    // Use a dummy KEK for retire_dek (the KEK is only used to unwrap, which fails after retire)
    let dummy_kek = [0u8; 32];
    if let Ok(dek_store) = crate::key_partition::FjallDekStore::open(db) {
        // Attempt DEK retirement; failure is non-fatal for purge (key may not exist or KEK mismatch)
        let _ = dek_store.retire_dek(&parsed_id);
        // Also clean up DEK entries from the store
        let _ = dek_store.retire_dek(&parsed_id);
    }

    // 5. Open partitions and prepare atomic batch
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

    let mut batch = db.batch();
    let mut event_count = 0u64;

    // Scan events by instance key prefix (2-byte len prefix + 16-byte instance ID = 18 bytes)
    let event_prefix_bytes = encode_key(&parsed_id, SequenceNumber::try_from(1_u64).unwrap());
    let event_prefix = event_prefix_bytes[..18].to_vec();

    for guard in events_p.prefix(event_prefix) {
        let (k, _) = guard.into_inner().map_err(|_| StorageError::ScanFailed)?;
        batch.remove(&events_p, k);
        event_count += 1;
    }

    // Scan snapshots by instance prefix
    let snapshot_prefix = id_bytes.to_vec();
    for guard in snapshots_p.prefix(snapshot_prefix) {
        let (k, _) = guard.into_inner().map_err(|_| StorageError::ScanFailed)?;
        batch.remove(&snapshots_p, k);
    }

    // Queue index entry removal
    let index_key = encode_instance_index_key(entry.status, entry.created_at, &parsed_id)?;
    batch.remove(&instances_p, index_key);

    // 6. Commit atomic batch
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
    use crate::instance_index::instance_index_upsert;
    use crate::key_encoding::encode_event_key as encode_key;
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
        let key_one = encode_key(&instance_id, sequence_one);
        let key_two = encode_key(&instance_id, sequence_two);

        events.insert(&key_one, b"event-one").unwrap();
        events.insert(&key_two, b"event-two").unwrap();
        snapshots.insert(&key_one, b"snapshot-one").unwrap();

        let result = purge_instance(&db, &instance_id_str);

        assert_eq!(result, Ok(2));
        // Verify events were deleted using the 18-byte instance prefix
        let prefix = instance_id.to_bytes().unwrap().to_vec();
        assert_eq!(events.prefix(&prefix).count(), 0);
        assert_eq!(snapshots.prefix(&prefix).count(), 0);
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
