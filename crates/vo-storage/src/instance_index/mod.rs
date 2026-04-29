//! Instance index partition — secondary index keyed by status + `created_at` + `instance_id`.
//!
//! Architecture: Data (`InstanceIndexEntry`) → Calc (`encode_instance_index_key`,
//! `decode_instance_index_key`) → Actions (`instance_index_upsert`, `scan_by_status`,
//! `scan_all_instances`).

use std::sync::Arc;

use crate::codec::StorageError;
use crate::partitions::{get_partition_config, INSTANCES_PARTITION};
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
/// deletes the old key and inserts the new key atomically via `fjall::OwnedWriteBatch`.
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
    db: &fjall::Database,
    instance_id: &InstanceId,
    status: InstanceStatus,
    created_at: TimestampMs,
    previous_status: Option<InstanceStatus>,
) -> Result<(), StorageError> {
    let keyspace = db
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .map_err(|_| StorageError::Storage)?;

    let new_key = encode_instance_index_key(status, created_at, instance_id)?;

    match previous_status {
        Some(old_status) if old_status != status => {
            let old_key = encode_instance_index_key(old_status, created_at, instance_id)?;
            atomic_status_transition(db, &keyspace, &old_key, &new_key)
        }
        _ => keyspace
            .insert(new_key, &[] as &[u8])
            .map_err(|_| StorageError::Storage),
    }
}

/// Atomically delete the old status key and insert the new one via `fjall::OwnedWriteBatch`.
fn atomic_status_transition(
    db: &fjall::Database,
    keyspace: &fjall::Keyspace,
    old_key: &[u8; 25],
    new_key: &[u8; 25],
) -> Result<(), StorageError> {
    let mut batch = db.batch();
    batch.remove(keyspace, *old_key);
    batch.insert(keyspace, *new_key, &[] as &[u8]);
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
    db: &fjall::Database,
    status: InstanceStatus,
) -> impl Iterator<Item = Result<InstanceIndexEntry, StorageError>> {
    let keyspace_result = db.keyspace("instances", || fjall::KeyspaceCreateOptions::default());

    let Ok(keyspace) = keyspace_result else {
        return ScanIterator {
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };

    let prefix = [status.to_byte()];
    let iter = keyspace.prefix(prefix);

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
    db: &fjall::Database,
) -> impl Iterator<Item = Result<InstanceIndexEntry, StorageError>> {
    let keyspace_result = db.keyspace("instances", || fjall::KeyspaceCreateOptions::default());

    let Ok(keyspace) = keyspace_result else {
        return ScanIterator {
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };

    let iter = keyspace.prefix([]);

    ScanIterator {
        inner: Some(Box::new(iter)),
        init_error: None,
    }
}

// ---------------------------------------------------------------------------
// Internal iterator adapter
// ---------------------------------------------------------------------------

pub(crate) struct ScanIterator {
    pub(crate) inner: Option<Box<dyn DoubleEndedIterator<Item = fjall::Guard>>>,
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
            Some(guard) => {
                if let Ok((k_bytes, _v_bytes)) = guard.into_inner() {
                    Some(decode_instance_index_key(&k_bytes))
                } else {
                    self.inner = None;
                    Some(Err(StorageError::Storage))
                }
            }
            None => None,
        }
    }
}

// -----------------------------------------------------------------------------
// FjallInstanceIndex — persistent instance index store
// -----------------------------------------------------------------------------

/// Fjall-backed instance index store wrapping the instances partition.
///
/// Provides typed access to instance index operations with proper partition
/// configuration applied at keyspace creation time.
pub struct FjallInstanceIndex {
    db: Arc<fjall::Database>,
    partition: Arc<fjall::Keyspace>,
}

impl FjallInstanceIndex {
    /// Opens the instances partition with proper [`PartitionConfig`].
    ///
    /// # Errors
    ///
    /// Returns `StorageError::Storage` if the partition cannot be opened.
    pub fn open(db: &fjall::Database) -> Result<Self, StorageError> {
        let config = get_partition_config(INSTANCES_PARTITION);
        let partition = db
            .keyspace(INSTANCES_PARTITION, || config.to_fjall_options())
            .map_err(|e| StorageError::Storage)?;
        Ok(Self {
            db: Arc::new(db.clone()),
            partition: Arc::new(partition),
        })
    }

    /// Returns a reference to the underlying keyspace for use in batch operations.
    #[must_use]
    pub fn keyspace(&self) -> &fjall::Keyspace {
        &self.partition
    }
}
