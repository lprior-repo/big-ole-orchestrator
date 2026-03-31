//! Instance index partition — secondary index keyed by status + `created_at` + `instance_id`.
//!
//! Architecture: Data (`InstanceIndexEntry`) → Calc (`encode_instance_index_key`,
//! `decode_instance_index_key`) → Actions (`instance_index_upsert`, `scan_by_status`,
//! `scan_all_instances`).

use crate::codec::StorageError;
use vo_types::{InstanceId, InstanceStatus, TimestampMs};

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests_encoding;
#[cfg(test)]
mod tests_iterator;

// ---------------------------------------------------------------------------
// Data layer — entry struct
// ---------------------------------------------------------------------------

/// Decoded row from the instances partition.
///
/// Invariant: Every field was decoded from a valid 25-byte composite key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceIndexEntry {
    pub instance_id: InstanceId,
    pub status: InstanceStatus,
    pub created_at: TimestampMs,
}

// ---------------------------------------------------------------------------
// Calc layer — pure key encoding/decoding
// ---------------------------------------------------------------------------

/// Encode an instance index key as a 25-byte big-endian composite key.
///
/// Layout: `[status_byte(1)][created_at_u64_be(8)][instance_id_bytes(16)]` = 25 bytes.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if `instance_id.to_bytes()` fails
/// (should never happen for a properly constructed `InstanceId`).
pub fn encode_instance_index_key(
    status: InstanceStatus,
    created_at: TimestampMs,
    instance_id: &InstanceId,
) -> Result<[u8; 25], StorageError> {
    let id_bytes = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;
    let mut key = [0u8; 25];
    key[0] = status.to_byte();
    key[1..9].copy_from_slice(&created_at.as_u64().to_be_bytes());
    key[9..25].copy_from_slice(&id_bytes);
    Ok(key)
}

/// Decode a 25-byte composite key into its components.
///
/// # Errors
///
/// - `StorageError::CorruptKey` if `bytes.len() != 25`
/// - `StorageError::CorruptKey` if the status byte is not in `[0x01..=0x06]`
pub fn decode_instance_index_key(bytes: &[u8]) -> Result<InstanceIndexEntry, StorageError> {
    if bytes.len() != 25 {
        return Err(StorageError::CorruptKey);
    }
    let status = InstanceStatus::from_byte(bytes[0]).ok_or(StorageError::CorruptKey)?;
    let ts_bytes: [u8; 8] = bytes[1..9]
        .try_into()
        .map_err(|_| StorageError::CorruptKey)?;
    let created_at = TimestampMs::try_from(u64::from_be_bytes(ts_bytes))
        .map_err(|_| StorageError::CorruptKey)?;
    let id_bytes: [u8; 16] = bytes[9..25]
        .try_into()
        .map_err(|_| StorageError::CorruptKey)?;
    let instance_id = InstanceId::from_bytes(id_bytes);
    Ok(InstanceIndexEntry {
        instance_id,
        status,
        created_at,
    })
}

// ---------------------------------------------------------------------------
// Actions layer — side-effecting functions
// ---------------------------------------------------------------------------

/// Upsert an instance into the instances index partition.
///
/// If `previous_status` is `Some(old_status)` and `old_status != status`,
/// deletes the old key and inserts the new key atomically via `fjall::Batch`.
///
/// If `previous_status` is `None` or `previous_status == Some(status)`,
/// performs a simple insert (overwrite is idempotent for identical arguments).
///
/// # Errors
///
/// - `StorageError::Storage` if the partition cannot be opened.
/// - `StorageError::Storage` if the batch commit fails.
/// - `StorageError::CorruptKey` if key encoding fails.
pub fn instance_index_upsert(
    keyspace: &fjall::Keyspace,
    instance_id: &InstanceId,
    status: InstanceStatus,
    created_at: TimestampMs,
    previous_status: Option<InstanceStatus>,
) -> Result<(), StorageError> {
    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .map_err(|_| StorageError::Storage)?;

    let new_key = encode_instance_index_key(status, created_at, instance_id)?;

    match previous_status {
        Some(old_status) if old_status != status => {
            let old_key = encode_instance_index_key(old_status, created_at, instance_id)?;
            atomic_status_transition(keyspace, &partition, &old_key, &new_key)
        }
        _ => partition
            .insert(new_key, &[] as &[u8])
            .map_err(|_| StorageError::Storage),
    }
}

/// Atomically delete the old status key and insert the new one via `fjall::Batch`.
fn atomic_status_transition(
    keyspace: &fjall::Keyspace,
    partition: &fjall::PartitionHandle,
    old_key: &[u8; 25],
    new_key: &[u8; 25],
) -> Result<(), StorageError> {
    let mut batch = keyspace.batch();
    batch.remove(partition, *old_key);
    batch.insert(partition, *new_key, &[] as &[u8]);
    batch.commit().map_err(|_| StorageError::Storage)
}

/// Prefix-scan by status. Returns an iterator over all instances with the given status,
/// ordered by `created_at` ascending (oldest first).
///
/// # Errors (per item)
///
/// - `StorageError::CorruptKey` if a stored key cannot be decoded.
/// - `StorageError::Storage` if the underlying Fjall iterator encounters an I/O error.
///
/// # Errors (construction)
///
/// If the partition cannot be opened, the first `.next()` call returns
/// `Some(Err(StorageError::Storage))`.
pub fn scan_by_status(
    keyspace: &fjall::Keyspace,
    status: InstanceStatus,
) -> impl Iterator<Item = Result<InstanceIndexEntry, StorageError>> {
    let partition_result =
        keyspace.open_partition("instances", fjall::PartitionCreateOptions::default());

    let Ok(partition) = partition_result else {
        return ScanIterator {
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };

    let prefix = [status.to_byte()];
    let iter = partition.prefix(prefix);

    ScanIterator {
        inner: Some(Box::new(iter)),
        init_error: None,
    }
}

/// Full range scan over all instances across all statuses.
/// Returns entries ordered by `(status_byte, created_at)` ascending.
///
/// # Errors (per item)
///
/// - `StorageError::CorruptKey` if a stored key cannot be decoded.
/// - `StorageError::Storage` if the underlying Fjall iterator encounters an I/O error.
///
/// # Errors (construction)
///
/// If the partition cannot be opened, the first `.next()` call returns
/// `Some(Err(StorageError::Storage))`.
pub fn scan_all_instances(
    keyspace: &fjall::Keyspace,
) -> impl Iterator<Item = Result<InstanceIndexEntry, StorageError>> {
    let partition_result =
        keyspace.open_partition("instances", fjall::PartitionCreateOptions::default());

    let Ok(partition) = partition_result else {
        return ScanIterator {
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };

    let iter = partition.prefix([]);

    ScanIterator {
        inner: Some(Box::new(iter)),
        init_error: None,
    }
}

// ---------------------------------------------------------------------------
// Internal iterator adapter
// ---------------------------------------------------------------------------

pub(crate) struct ScanIterator {
    pub(crate) inner: Option<Box<dyn DoubleEndedIterator<Item = fjall::Result<fjall::KvPair>>>>,
    pub(crate) init_error: Option<StorageError>,
}

impl Iterator for ScanIterator {
    type Item = Result<InstanceIndexEntry, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.init_error.take() {
            return Some(Err(err));
        }
        let inner = self.inner.as_mut()?;
        match inner.next() {
            Some(Ok((k_bytes, _v_bytes))) => Some(decode_instance_index_key(&k_bytes)),
            Some(Err(_)) => {
                self.inner = None;
                Some(Err(StorageError::Storage))
            }
            None => None,
        }
    }
}
