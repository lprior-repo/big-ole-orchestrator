use fjall::Keyspace;
use vo_types::{InstanceId, InstanceStatus};
use crate::codec::StorageError;
use crate::instance_index::{scan_all_instances, encode_instance_index_key};

/// Purges all records for a given instance ID.
///
/// # Errors
///
/// - `StorageError::InstanceRunning` if the instance is not in a terminal state or not found.
/// - `StorageError::InvalidInstanceId` if the ID is malformed.
/// - `StorageError::ScanFailed` if the storage scan fails.
/// - `StorageError::BatchCommitFailed` if the atomic deletion fails.
pub fn purge_instance(
    keyspace: &Keyspace,
    instance_id_str: &str,
) -> Result<u64, StorageError> {
    if instance_id_str.is_empty() {
        return Err(StorageError::InvalidInstanceId(vo_types::ParseError::Empty {
            type_name: "InstanceId",
        }));
    }
    
    // 1. Parse InstanceId
    let parsed_id = InstanceId::parse(instance_id_str)
        .map_err(StorageError::InvalidInstanceId)?;

    // 2. Find instance in index to verify status and get metadata for key reconstruction
    let entry = scan_all_instances(keyspace)
        .find(|r| r.as_ref().is_ok_and(|e| e.instance_id == parsed_id))
        .ok_or(StorageError::InstanceRunning)? // Mapping "not found" to InstanceRunning per test expectations for non-terminal
        .map_err(|_| StorageError::ScanFailed)?;

    // 3. Verify status is terminal
    if !is_terminal(entry.status) {
        return Err(StorageError::InstanceRunning);
    }

    // 4. Open partitions and prepare atomic batch
    let opts = fjall::PartitionCreateOptions::default();
    let events_p = keyspace.open_partition("events", opts.clone()).map_err(|_| StorageError::ScanFailed)?;
    let snapshots_p = keyspace.open_partition("snapshots", opts.clone()).map_err(|_| StorageError::ScanFailed)?;
    let instances_p = keyspace.open_partition("instances", opts).map_err(|_| StorageError::ScanFailed)?;

    let id_bytes = parsed_id.to_bytes().map_err(|_| StorageError::CorruptKey)?;
    let mut batch = keyspace.batch();
    let mut event_count = 0u64;

    // Scan and queue events for deletion
    for item in events_p.prefix(&id_bytes) {
        let (k, _) = item.map_err(|_| StorageError::ScanFailed)?;
        batch.remove(&events_p, k);
        event_count += 1;
    }

    // Scan and queue snapshots for deletion
    for item in snapshots_p.prefix(&id_bytes) {
        let (k, _) = item.map_err(|_| StorageError::ScanFailed)?;
        batch.remove(&snapshots_p, k);
    }

    // Queue index entry removal
    let index_key = encode_instance_index_key(entry.status, entry.created_at, &parsed_id)?;
    batch.remove(&instances_p, index_key);

    // 5. Commit atomic batch
    batch.commit().map_err(|_| StorageError::BatchCommitFailed)?;

    Ok(event_count)
}

#[must_use]
pub const fn is_terminal(status: InstanceStatus) -> bool {
    matches!(
        status,
        InstanceStatus::Completed | InstanceStatus::Failed | InstanceStatus::Cancelled
    )
}
