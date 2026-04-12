use crate::codec::StorageError;
use fjall::PartitionHandle;
use vo_types::state::InstanceState;
use vo_types::InstanceId;

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
    partition: &PartitionHandle,
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
/// Supports legacy format (direct `InstanceState` JSON).
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the instance ID cannot be serialized.
/// Returns `StorageError::FjallError` if the storage engine fails.
/// Returns `StorageError::InvalidKey` if a stored key is not exactly 24 bytes.
/// Returns `StorageError::DeserializationFailed` if the stored value is not valid JSON.
pub fn snapshot_load_latest(
    partition: &PartitionHandle,
    instance_id: &InstanceId,
) -> Result<Option<(u64, InstanceState)>, StorageError> {
    let prefix = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;

    partition
        .prefix(&prefix)
        .next_back()
        .map_or(Ok(None), |result| {
            result
                .map_err(|_| StorageError::FjallError)
                .and_then(|(key, value)| {
                    decode_snapshot_key(&key).and_then(|(_, sequence)| {
                        // Deserialize InstanceState directly
                        let state = serde_json::from_slice(&value)
                            .map_err(|_| StorageError::DeserializationFailed)?;
                        Ok(Some((sequence, state)))
                    })
                })
        })
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
