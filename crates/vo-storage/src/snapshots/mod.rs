use crate::codec::StorageError;
use serde::{Deserialize, Serialize};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

pub const CURRENT_SNAPSHOT_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotHeader {
    pub version: u16,
    pub sequence_number: u64,
    pub instance_id: InstanceId,
    pub checksum: u32,
}

impl SnapshotHeader {
    #[must_use]
    pub const fn new(instance_id: InstanceId, sequence_number: u64, checksum: u32) -> Self {
        Self {
            version: CURRENT_SNAPSHOT_VERSION,
            sequence_number,
            instance_id,
            checksum,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPolicy {
    EveryNEvents(u64),
    Disabled,
}

impl Default for SnapshotPolicy {
    fn default() -> Self {
        Self::EveryNEvents(100)
    }
}

impl SnapshotPolicy {
    #[must_use]
    pub const fn should_snapshot(&self, current_sequence: u64) -> bool {
        match self {
            Self::EveryNEvents(n) => current_sequence > 0 && current_sequence.is_multiple_of(*n),
            Self::Disabled => false,
        }
    }
}

pub struct AtomicSnapshotWriter<'a> {
    db: &'a fjall::Database,
    snapshot_partition: fjall::Keyspace,
}

impl<'a> AtomicSnapshotWriter<'a> {
    /// Creates a new `AtomicSnapshotWriter`.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::Storage` if the snapshots partition cannot be opened.
    pub fn new(db: &'a fjall::Database) -> Result<Self, StorageError> {
        let snapshot_partition = db
            .keyspace("snapshots", || fjall::KeyspaceCreateOptions::default())
            .map_err(|_| StorageError::Storage)?;
        Ok(Self {
            db,
            snapshot_partition,
        })
    }

    /// Adds a snapshot write to the given batch.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
    /// Returns `StorageError::SerializationFailed` if serialization fails.
    pub fn write_snapshot(
        &self,
        batch: &mut fjall::OwnedWriteBatch,
        instance_id: InstanceId,
        sequence: u64,
        state: &InstanceState,
    ) -> Result<(), StorageError> {
        let key = encode_snapshot_key(&instance_id, sequence)?;
        let state_json =
            serde_json::to_vec(state).map_err(|_| StorageError::SerializationFailed)?;
        let checksum = crc32fast::hash(&state_json);
        let header = SnapshotHeader::new(instance_id, sequence, checksum);
        let header_bytes =
            serde_json::to_vec(&header).map_err(|_| StorageError::SerializationFailed)?;
        let mut value = header_bytes;
        value.push(b'|');
        value.extend_from_slice(&state_json);
        batch.insert(&self.snapshot_partition, key, &value);
        Ok(())
    }

    /// Writes a snapshot atomically using a dedicated batch.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
    /// Returns `StorageError::SerializationFailed` if serialization fails.
    /// Returns `StorageError::BatchCommitFailed` if the batch commit fails.
    pub fn write_snapshot_atomic(
        &self,
        instance_id: InstanceId,
        sequence: u64,
        state: &InstanceState,
    ) -> Result<(), StorageError> {
        let mut batch = self.db.batch();
        self.write_snapshot(&mut batch, instance_id, sequence, state)?;
        batch.commit().map_err(|_| StorageError::BatchCommitFailed)
    }
}

#[derive(Debug, Clone)]
pub struct RecoveryThrottleConfig {
    pub batch_size: usize,
    pub delay_between_batches_ms: u64,
}

impl Default for RecoveryThrottleConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            delay_between_batches_ms: 50,
        }
    }
}

pub struct RecoveryThrottle {
    config: RecoveryThrottleConfig,
    processed_count: usize,
}

impl RecoveryThrottle {
    #[must_use]
    pub const fn new(config: RecoveryThrottleConfig) -> Self {
        Self {
            config,
            processed_count: 0,
        }
    }

    #[must_use]
    pub const fn should_process(&self) -> bool {
        self.processed_count < self.config.batch_size
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn mark_processed(&mut self) {
        self.processed_count += 1;
    }

    #[must_use]
    pub const fn delay_ms(&self) -> Option<u64> {
        if self.processed_count >= self.config.batch_size {
            Some(self.config.delay_between_batches_ms)
        } else {
            None
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn reset(&mut self) {
        self.processed_count = 0;
    }
}

/// Compacts snapshots for an instance, keeping only the last N snapshots.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
/// Returns `StorageError::FjallError` if the storage engine fails.
/// Returns `StorageError::InvalidKey` if a stored key is not exactly 24 bytes.
pub fn compact_snapshots(
    partition: &fjall::Keyspace,
    instance_id: &InstanceId,
    keep_last_n: u64,
) -> Result<u64, StorageError> {
    let prefix = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;
    let mut snapshots: Vec<(u64, Vec<u8>)> = Vec::new();
    for item in partition.prefix(&prefix) {
        let (key, value) = item.into_inner().map_err(|_| StorageError::FjallError)?;
        let (_, seq) = decode_snapshot_key(&key).map_err(|_| StorageError::InvalidKey)?;
        snapshots.push((seq, value.to_vec()));
    }
    if snapshots.len() <= usize::try_from(keep_last_n).unwrap_or(usize::MAX) {
        return Ok(0);
    }
    snapshots.sort_by_key(|b| std::cmp::Reverse(b.0));
    let to_delete = &snapshots[usize::try_from(keep_last_n).unwrap_or(usize::MAX)..];
    let mut deleted = 0u64;
    for (seq, _) in to_delete {
        let key = encode_snapshot_key(instance_id, *seq)?;
        if partition.remove(key).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Returns all snapshot sequence numbers for an instance, sorted descending.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
/// Returns `StorageError::FjallError` if the storage engine fails.
/// Returns `StorageError::InvalidKey` if a stored key is not exactly 24 bytes.
pub fn get_all_snapshot_sequences(
    partition: &fjall::Keyspace,
    instance_id: &InstanceId,
) -> Result<Vec<u64>, StorageError> {
    let prefix = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;
    let mut sequences = Vec::new();
    for item in partition.prefix(&prefix) {
        let (key, _) = item.into_inner().map_err(|_| StorageError::FjallError)?;
        let (_, seq) = decode_snapshot_key(&key).map_err(|_| StorageError::InvalidKey)?;
        sequences.push(seq);
    }
    sequences.sort_by_key(|b| std::cmp::Reverse(*b));
    Ok(sequences)
}

/// Writes a snapshot of `state` at the given `sequence` for `instance_id`.
///
/// Stores raw state JSON.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
/// Returns `StorageError::SerializationFailed` if serialization fails.
/// Returns `StorageError::FjallError` if the storage engine fails.
#[allow(clippy::needless_pass_by_value)]
pub fn snapshot_write(
    partition: &fjall::Keyspace,
    instance_id: InstanceId,
    sequence: u64,
    state: &InstanceState,
) -> Result<(), StorageError> {
    let key = encode_snapshot_key(&instance_id, sequence)?;

    // Serialize state to JSON
    let state_json = serde_json::to_vec(state).map_err(|_| StorageError::SerializationFailed)?;

    partition
        .insert(key, state_json)
        .map_err(|_| StorageError::FjallError)
}

/// Loads the latest (highest-sequence) snapshot for `instance_id`.
///
/// Supports both formats:
/// - Header format: `header_json | state_json` (written by `AtomicSnapshotWriter`)
/// - Legacy format: direct `InstanceState` JSON (written by `snapshot_write`)
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
/// Returns `StorageError::FjallError` if the storage engine fails.
/// Returns `StorageError::InvalidKey` if a stored key is not exactly 24 bytes.
/// Returns `StorageError::DeserializationFailed` if the stored value is not valid JSON
/// or checksum verification fails.
pub fn snapshot_load_latest(
    partition: &fjall::Keyspace,
    instance_id: &InstanceId,
) -> Result<Option<(u64, InstanceState)>, StorageError> {
    let prefix = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;

    partition
        .prefix(&prefix)
        .next_back()
        .map_or(Ok(None), |guard| {
            guard
                .into_inner()
                .map_err(|_| StorageError::FjallError)
                .and_then(|(key, value)| {
                    decode_snapshot_key(&key).and_then(|(_, sequence)| {
                        deserialize_snapshot_value(&value).map(|state| Some((sequence, state)))
                    })
                })
        })
}

fn deserialize_snapshot_value(value: &[u8]) -> Result<InstanceState, StorageError> {
    if let Some(pos) = value.iter().position(|&b| b == b'|') {
        let (header_bytes, state_json) = value.split_at(pos);
        let state_json = &state_json[1..];
        let header: SnapshotHeader = serde_json::from_slice(header_bytes)
            .map_err(|_| StorageError::DeserializationFailed)?;
        let computed_checksum = crc32fast::hash(state_json);
        if computed_checksum != header.checksum {
            return Err(StorageError::DeserializationFailed);
        }
        serde_json::from_slice(state_json).map_err(|_| StorageError::DeserializationFailed)
    } else {
        serde_json::from_slice(value).map_err(|_| StorageError::DeserializationFailed)
    }
}

/// Encodes an `(InstanceId, u64)` pair into a 24-byte snapshot key.
///
/// Layout: `[instance_id_16_bytes | sequence_u64_be_8_bytes]`.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the `InstanceId` cannot be converted to bytes.
pub fn encode_snapshot_key(
    instance_id: &InstanceId,
    sequence: u64,
) -> Result<[u8; 24], StorageError> {
    let id_bytes = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;
    let seq_bytes = sequence.to_be_bytes();
    let mut key = [0u8; 24];
    key[..16].copy_from_slice(&id_bytes);
    key[16..].copy_from_slice(&seq_bytes);
    Ok(key)
}

/// Decodes a 24-byte snapshot key into an `(InstanceId, u64)` pair.
///
/// # Errors
///
/// Returns `StorageError::InvalidKey` if `key` is not exactly 24 bytes.
pub fn decode_snapshot_key(key: &[u8]) -> Result<(InstanceId, u64), StorageError> {
    let array: &[u8; 24] = key.try_into().map_err(|_| StorageError::InvalidKey)?;

    let id_bytes: [u8; 16] = array[..16]
        .try_into()
        .map_err(|_| StorageError::InvalidKey)?;
    let instance_id = InstanceId::from_bytes(id_bytes);

    let seq_bytes: [u8; 8] = array[16..]
        .try_into()
        .map_err(|_| StorageError::InvalidKey)?;
    let sequence = u64::from_be_bytes(seq_bytes);

    Ok((instance_id, sequence))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests_property.rs"]
mod tests_property;

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn encode_snapshot_key_bounds() {
        let seq: u64 = kani::any();
        let id_bytes: [u8; 16] = kani::any();
        let id = InstanceId::from_bytes(id_bytes);

        if let Ok(result) = encode_snapshot_key(&id, seq) {
            assert!(result.len() == 24);
        }
    }
}
