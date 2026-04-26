//! Canonical key encoding utilities for storage partitions (ADR-020).
//!
//! All Fjall keys follow two rules:
//! 1. **Numeric components use fixed-width, big-endian binary encoding.**
//! 2. **Variable-length identifiers are length-prefixed.**
//!
//! ## Key Formats
//!
//! - **Events**: `[instance_id_len_u16_be(2)][instance_id_bytes(16)][sequence_u64_be(8)]` (26 bytes)
//! - **Timers**: `[timestamp_u64_be(8)][instance_id_len_u16_be(2)][instance_id_bytes(16)]` (26 bytes)
//! - **Leases**: `[instance_id_bytes(16)][step_id_len_u16_be(2)][step_id_bytes]` (18+ bytes)
//! - **Instances**: `[status_byte(1)][created_at_u64_be(8)][instance_id_bytes(16)]` (25 bytes)
//! - **Dedupe**: `[idempotency_key_len_u16_be(2)][idempotency_key_bytes]` (4+ bytes)
//! - **Effects**: `[instance_id_len_u16_be(2)][instance_id_bytes(16)][sequence_u64_be(8)][effect_marker(1)]` (27 bytes)

use vo_types::{InstanceId, ParseError, SequenceNumber, StepId};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod red_queen_adversarial;
#[cfg(test)]
mod red_queen_tests;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyEncodingError {
    #[error("instance ID encoding error: {0}")]
    InstanceId(#[from] ParseError),
    #[error("step ID encoding error: {0}")]
    StepId(ParseError),
    #[error("invalid length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("field cannot be empty")]
    EmptyField,
    #[error("key component too long: max {max} bytes, got {actual} bytes")]
    KeyComponentTooLong { max: usize, actual: usize },
}

impl From<std::str::Utf8Error> for KeyEncodingError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::StepId(ParseError::InvalidFormat {
            type_name: "Utf8",
            reason: e.to_string(),
        })
    }
}

pub const PARTITION_EVENTS: &[u8] = b"events";
pub const PARTITION_TIMERS: &[u8] = b"timers";
pub const PARTITION_LEASES: &[u8] = b"leases";
pub const PARTITION_INSTANCES: &[u8] = b"instances";
pub const PARTITION_DEDUPE: &[u8] = b"dedupe";
pub const PARTITION_EFFECTS: &[u8] = b"effects";

#[must_use]
pub const fn encode_u64_be(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

#[must_use]
pub const fn encode_u16_be(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

/// Decode a big-endian 8-byte slice into a u64.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 8 bytes.
pub fn decode_u64_be(bytes: &[u8]) -> Result<u64, KeyEncodingError> {
    if bytes.len() != 8 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 8,
            actual: bytes.len(),
        });
    }
    let arr: [u8; 8] = bytes
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 8,
            actual: bytes.len(),
        })?;
    Ok(u64::from_be_bytes(arr))
}

/// Decode a big-endian 2-byte slice into a u16.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 2 bytes.
pub fn decode_u16_be(bytes: &[u8]) -> Result<u16, KeyEncodingError> {
    if bytes.len() != 2 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 2,
            actual: bytes.len(),
        });
    }
    let arr: [u8; 2] = bytes
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 2,
            actual: bytes.len(),
        })?;
    Ok(u16::from_be_bytes(arr))
}

/// Encode a byte slice as length-prefixed (u16 big-endian length + data).
///
/// # Errors
///
/// Returns `KeyEncodingError::KeyComponentTooLong` if `value.len() > u16::MAX` (65535 bytes).
/// Overflow must be explicit; no truncation.
pub fn encode_length_prefixed(value: &[u8]) -> Result<Vec<u8>, KeyEncodingError> {
    let len = u16::try_from(value.len()).map_err(|_| KeyEncodingError::KeyComponentTooLong {
        max: u16::MAX as usize,
        actual: value.len(),
    })?;
    let mut result = Vec::with_capacity(2 + value.len());
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(value);
    Ok(result)
}

/// Decode a length-prefixed byte slice.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is too short.
pub fn decode_length_prefixed(bytes: &[u8]) -> Result<(&[u8], &[u8]), KeyEncodingError> {
    if bytes.len() < 2 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 2,
            actual: bytes.len(),
        });
    }
    let len = decode_u16_be(&bytes[..2])? as usize;
    if bytes.len() < 2 + len {
        return Err(KeyEncodingError::InvalidLength {
            expected: 2 + len,
            actual: bytes.len(),
        });
    }
    Ok((&bytes[2..2 + len], &bytes[2 + len..]))
}

/// Encode an `InstanceId` as its 16-byte ULID representation.
///
/// # Errors
///
/// Returns `KeyEncodingError::InstanceId` if the instance ID cannot be serialized.
pub fn encode_instance_id(instance_id: &InstanceId) -> Result<[u8; 16], KeyEncodingError> {
    instance_id.to_bytes().map_err(KeyEncodingError::from)
}

/// Decode a 16-byte slice into an `InstanceId`.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 16 bytes.
pub fn decode_instance_id(bytes: &[u8]) -> Result<InstanceId, KeyEncodingError> {
    let arr: [u8; 16] = bytes
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 16,
            actual: bytes.len(),
        })?;
    Ok(InstanceId::from_bytes(arr))
}

/// Encode a `StepId` as a length-prefixed byte string.
///
/// # Errors
///
/// Returns `KeyEncodingError::KeyComponentTooLong` if the step ID exceeds 65535 bytes.
pub fn encode_step_id(step_id: &StepId) -> Result<Vec<u8>, KeyEncodingError> {
    encode_length_prefixed(step_id.as_str().as_bytes())
}

/// Decode a length-prefixed byte string into a `StepId`.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the data is too short.
/// Returns `KeyEncodingError::StepId` if the decoded string is not a valid `StepId`.
pub fn decode_step_id(bytes: &[u8]) -> Result<StepId, KeyEncodingError> {
    let (id_bytes, _) = decode_length_prefixed(bytes)?;
    let s = std::str::from_utf8(id_bytes).map_err(|e| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "StepId",
            reason: e.to_string(),
        })
    })?;
    StepId::parse(s).map_err(KeyEncodingError::from)
}

/// Encode a `SequenceNumber` as big-endian bytes.
#[must_use]
pub fn encode_sequence_number(seq: SequenceNumber) -> [u8; 8] {
    encode_u64_be(seq.as_u64())
}

/// Decode big-endian bytes into a `SequenceNumber`.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 8 bytes.
/// Returns `KeyEncodingError::InstanceId` if the decoded value is not a valid sequence.
///
/// # Panics
///
/// Panics if the slice is not exactly 8 bytes.
pub fn decode_sequence_number(bytes: &[u8]) -> Result<SequenceNumber, KeyEncodingError> {
    let val = decode_u64_be(bytes)?;
    SequenceNumber::try_from(val).map_err(KeyEncodingError::InstanceId)
}

#[must_use]
pub fn encode_event_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(2 + 16 + 8);
    key.extend_from_slice(&encode_u16_be(iid_bytes.len() as u16));
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key
}

/// Decode an event key into instance ID and sequence number.
///
/// Expected format: `[instance_id_len_u16_be(2)][instance_id_bytes(16)][sequence_u64_be(8)]`
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 26 bytes.
pub fn decode_event_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() != 26 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 26,
            actual: bytes.len(),
        });
    }
    let _len = decode_u16_be(&bytes[..2])?;
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[2..18].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let seq_bytes: [u8; 8] = bytes[18..26].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(KeyEncodingError::InstanceId)?;
    Ok((instance_id, sequence))
}

#[must_use]
pub fn encode_timer_key(fire_at_ms: u64, instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut key = Vec::with_capacity(8 + 2 + iid_bytes.len());
    key.extend_from_slice(&fire_at_ms.to_be_bytes());
    key.extend_from_slice(&encode_u16_be(iid_bytes.len() as u16));
    key.extend_from_slice(&iid_bytes);
    key
}

/// Decode a timer key into fire-at timestamp and instance ID.
///
/// Expected format: `[timestamp_u64_be(8)][instance_id_len_u16_be(2)][instance_id_bytes]`
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is too short for the declared length.
pub fn decode_timer_key(bytes: &[u8]) -> Result<(u64, InstanceId), KeyEncodingError> {
    if bytes.len() < 10 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 10,
            actual: bytes.len(),
        });
    }
    let ts_bytes: [u8; 8] = bytes[..8]
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 8,
            actual: bytes.len(),
        })?;
    let iid_len = decode_u16_be(&bytes[8..10])? as usize;
    let required = 10 + iid_len;
    if bytes.len() < required {
        return Err(KeyEncodingError::InvalidLength {
            expected: required,
            actual: bytes.len(),
        });
    }
    let iid_bytes: [u8; 16] = bytes[10..10 + iid_len]
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 16,
            actual: iid_len,
        })?;
    Ok((
        u64::from_be_bytes(ts_bytes),
        InstanceId::from_bytes(iid_bytes),
    ))
}

/// Encode a lease key from instance ID and step ID.
///
/// Format: `[instance_id_bytes(16)][step_id_len_u16_be(2)][step_id_bytes]`
#[must_use]
pub fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let step_bytes = step_id.as_str().as_bytes();
    let mut key = Vec::with_capacity(16 + 2 + step_bytes.len());
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&encode_u16_be(step_bytes.len() as u16));
    key.extend_from_slice(step_bytes);
    key
}

/// Decode a lease key into instance ID and step ID.
///
/// Expected format: `[instance_id_bytes(16)][step_id_len_u16_be(2)][step_id_bytes]`
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is too short, or if the step ID
/// component is invalid.
pub fn decode_lease_key(bytes: &[u8]) -> Result<(InstanceId, StepId), KeyEncodingError> {
    if bytes.len() < 18 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 18,
            actual: bytes.len(),
        });
    }
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let step_id = decode_step_id(&bytes[16..])?;
    Ok((instance_id, step_id))
}

/// Encode a dedupe key as length-prefixed bytes.
///
/// # Errors
///
/// Returns `KeyEncodingError::KeyComponentTooLong` if the idempotency key exceeds 65535 bytes.
pub fn encode_dedupe_key(idempotency_key: &str) -> Result<Vec<u8>, KeyEncodingError> {
    encode_length_prefixed(idempotency_key.as_bytes())
}

/// Decode a length-prefixed dedupe key.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the data is too short.
/// Returns `KeyEncodingError::StepId` if the decoded bytes are not valid UTF-8.
pub fn decode_dedupe_key(bytes: &[u8]) -> Result<String, KeyEncodingError> {
    let (key_bytes, _) = decode_length_prefixed(bytes)?;
    String::from_utf8(key_bytes.to_vec()).map_err(|e| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "IdempotencyKey",
            reason: e.to_string(),
        })
    })
}

/// Encode an instance index key with status byte and creation time.
#[must_use]
pub fn encode_instance_index_key_for_status(
    status_byte: u8,
    created_at: u64,
    instance_id: &InstanceId,
) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut key = Vec::with_capacity(1 + 8 + 16);
    key.push(status_byte);
    key.extend_from_slice(&created_at.to_be_bytes());
    key.extend_from_slice(&iid_bytes);
    key
}

/// Encode an effect key from instance ID and sequence number with effect marker.
///
/// Format: `[instance_id_len_u16_be(2)][instance_id_bytes(16)][sequence_u64_be(8)][effect_marker(1)]`
#[must_use]
pub fn encode_effect_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(2 + 16 + 8 + 1);
    key.extend_from_slice(&encode_u16_be(iid_bytes.len() as u16));
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key.push(0xFF);
    key
}

/// Decode an effect key into instance ID and sequence number.
///
/// Expected format: `[instance_id_len_u16_be(2)][instance_id_bytes(16)][sequence_u64_be(8)][effect_marker(1)]`
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 27 bytes or missing the 0xFF marker.
pub fn decode_effect_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() != 27 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 27,
            actual: bytes.len(),
        });
    }
    if bytes[26] != 0xFF {
        return Err(KeyEncodingError::InvalidLength {
            expected: 27,
            actual: bytes.len(),
        });
    }
    let _len = decode_u16_be(&bytes[..2])?;
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[2..18].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let seq_bytes: [u8; 8] = bytes[18..26].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(KeyEncodingError::InstanceId)?;
    Ok((instance_id, sequence))
}

/// Get the key prefix for all events of a given instance.
///
/// Returns the first 16 bytes of the encoded event key (length-prefixed instance ID +
/// instance ID bytes), sufficient for range scans.
#[must_use]
pub fn get_event_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&encode_u16_be(iid_bytes.len() as u16));
    prefix.extend_from_slice(&iid_bytes);
    prefix
}

/// Get the key prefix for timers at or after a given timestamp.
#[must_use]
pub fn get_timer_key_prefix_for_time(fire_at_ms: u64) -> Vec<u8> {
    fire_at_ms.to_be_bytes().to_vec()
}

/// Get the key prefix for all lease entries of a given instance.
///
/// Returns the first 16 bytes of the encoded lease key (raw instance ID bytes),
/// sufficient for range scans of all leases for a given instance.
#[must_use]
pub fn get_lease_key_prefix_for_instance(instance_id: &InstanceId) -> Vec<u8> {
    instance_id.to_bytes().unwrap_or([0u8; 16]).to_vec()
}

/// Get the key prefix for a dedupe key.
///
/// # Errors
///
/// Returns `KeyEncodingError::KeyComponentTooLong` if the idempotency key exceeds 65535 bytes.
pub fn get_dedupe_key_prefix(idempotency_key: &str) -> Result<Vec<u8>, KeyEncodingError> {
    encode_length_prefixed(idempotency_key.as_bytes())
}
