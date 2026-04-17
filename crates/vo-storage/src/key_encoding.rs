//! Canonical key encoding utilities for storage partitions (ADR-020).
//!
//! All Fjall keys follow two rules:
//! 1. **Numeric components use fixed-width, big-endian binary encoding.**
//! 2. **Variable-length identifiers are length-prefixed.**
//!
//! ## Key Formats
//!
//! - **Events**: `[instance_id(16)][sequence_u64_be]` (26 bytes with length prefix)
//! - **Timers**: `[timestamp_u64_be][instance_id_len_u16_be][instance_id_bytes]`
//! - **Leases**: `[instance_id(16)][step_id_len_u16_be][step_id_bytes]`
//! - **Instances**: `[status_byte][created_at_u64_be][instance_id(16)]`
//! - **Dedupe**: `[idempotency_key_len_u16_be][idempotency_key_bytes]`
//! - **Effects**: `[instance_id(16)][sequence_u64_be][effect_marker]`

use vo_types::{InstanceId, ParseError, SequenceNumber, StepId};

#[cfg(test)]
mod tests;

<<<<<<< HEAD
#[cfg(test)]
mod red_queen_adversarial;
<<<<<<< HEAD
#[cfg(test)]
mod red_queen_tests;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
=======

#[derive(Debug, Clone, PartialEq, Eq)]
>>>>>>> origin/vo-worker-tests
pub enum KeyEncodingError {
    #[error("instance ID encoding error: {0}")]
    InstanceId(#[from] ParseError),
    #[error("step ID encoding error: {0}")]
    StepId(ParseError),
    #[error("invalid length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("field cannot be empty")]
    EmptyField,
}

impl From<std::str::Utf8Error> for KeyEncodingError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::StepId(ParseError::InvalidFormat {
            type_name: "Utf8",
            reason: e.to_string(),
        })
=======
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEncodingError {
    InstanceId(ParseError),
    StepId(ParseError),
    InvalidLength { expected: usize, actual: usize },
    EmptyField,
}

impl std::fmt::Display for KeyEncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstanceId(e) => write!(f, "instance ID encoding error: {e}"),
            Self::StepId(e) => write!(f, "step ID encoding error: {e}"),
            Self::InvalidLength { expected, actual } => {
                write!(f, "invalid length: expected {expected}, got {actual}")
            }
            Self::EmptyField => write!(f, "field cannot be empty"),
        }
    }
}

impl std::error::Error for KeyEncodingError {}

impl From<ParseError> for KeyEncodingError {
    fn from(e: ParseError) -> Self {
        Self::InstanceId(e)
>>>>>>> origin/polecat/synth-mnw6kj8v
    }
}

pub const PARTITION_EVENTS: &[u8] = b"events";
pub const PARTITION_TIMERS: &[u8] = b"timers";
pub const PARTITION_LEASES: &[u8] = b"leases";
pub const PARTITION_INSTANCES: &[u8] = b"instances";
pub const PARTITION_DEDUPE: &[u8] = b"dedupe";
pub const PARTITION_EFFECTS: &[u8] = b"effects";

#[must_use]
<<<<<<< HEAD
pub const fn encode_u64_be(value: u64) -> [u8; 8] {
=======
pub fn encode_u64_be(value: u64) -> [u8; 8] {
>>>>>>> origin/polecat/synth-mnw6kj8v
    value.to_be_bytes()
}

#[must_use]
<<<<<<< HEAD
pub const fn encode_u16_be(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

/// Decode a big-endian 8-byte slice into a u64.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 8 bytes.
=======
pub fn encode_u16_be(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

>>>>>>> origin/polecat/synth-mnw6kj8v
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

<<<<<<< HEAD
/// Decode a big-endian 2-byte slice into a u16.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 2 bytes.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
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

#[must_use]
pub fn encode_length_prefixed(value: &[u8]) -> Vec<u8> {
<<<<<<< HEAD
    let len = u16::try_from(value.len()).unwrap_or(u16::MAX);
=======
    let len = value.len() as u16;
>>>>>>> origin/polecat/synth-mnw6kj8v
    let mut result = Vec::with_capacity(2 + value.len());
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(value);
    result
}

<<<<<<< HEAD
/// Decode a length-prefixed byte slice.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is too short.
pub fn decode_length_prefixed(bytes: &[u8]) -> Result<(&[u8], &[u8]), KeyEncodingError> {
=======
pub fn decode_length_prefixed<'a>(
    bytes: &'a [u8],
) -> Result<(&'a [u8], &'a [u8]), KeyEncodingError> {
>>>>>>> origin/polecat/synth-mnw6kj8v
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

<<<<<<< HEAD
/// Encode an `InstanceId` as its 16-byte ULID representation.
///
/// # Errors
///
/// Returns `KeyEncodingError::InstanceId` if the instance ID cannot be serialized.
=======
#[must_use]
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn encode_instance_id(instance_id: &InstanceId) -> Result<[u8; 16], KeyEncodingError> {
    instance_id.to_bytes().map_err(KeyEncodingError::from)
}

<<<<<<< HEAD
/// Decode a 16-byte slice into an `InstanceId`.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the slice is not exactly 16 bytes.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn decode_instance_id(bytes: &[u8]) -> Result<InstanceId, KeyEncodingError> {
    let arr: [u8; 16] = bytes
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 16,
            actual: bytes.len(),
        })?;
    Ok(InstanceId::from_bytes(arr))
}

<<<<<<< HEAD
/// Encode a `StepId` as a length-prefixed byte string.
///
/// # Panics
///
/// Panics if the step ID exceeds 65535 bytes.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
#[must_use]
pub fn encode_step_id(step_id: &StepId) -> Vec<u8> {
    encode_length_prefixed(step_id.as_str().as_bytes())
}

<<<<<<< HEAD
/// Decode a length-prefixed byte string into a `StepId`.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the data is too short.
/// Returns `KeyEncodingError::StepId` if the decoded string is not a valid `StepId`.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
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

<<<<<<< HEAD
/// Encode a `SequenceNumber` as big-endian bytes.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
#[must_use]
pub fn encode_sequence_number(seq: SequenceNumber) -> [u8; 8] {
    encode_u64_be(seq.as_u64())
}

<<<<<<< HEAD
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
=======
pub fn decode_sequence_number(bytes: &[u8]) -> Result<SequenceNumber, KeyEncodingError> {
    let val = decode_u64_be(bytes)?;
    SequenceNumber::try_from(val).map_err(|e| KeyEncodingError::InstanceId(e))
}

>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn encode_event_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(16 + 8);
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key
}

<<<<<<< HEAD
<<<<<<< HEAD
/// Decode an event key into instance ID and sequence number.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 24 bytes.
///
/// # Panics
///
/// Panics if the key is not exactly 24 bytes.
=======
#[expect(clippy::unwrap_used)]
>>>>>>> origin/vo-worker-tests
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn decode_event_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() != 24 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 24,
            actual: bytes.len(),
        });
    }
<<<<<<< HEAD
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let seq_bytes: [u8; 8] = bytes[16..24].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(KeyEncodingError::InstanceId)?;
=======
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    let seq_bytes: [u8; 8] = bytes[16..24].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(|e| KeyEncodingError::InstanceId(e))?;
>>>>>>> origin/polecat/synth-mnw6kj8v
    Ok((instance_id, sequence))
}

#[must_use]
pub fn encode_timer_key(fire_at_ms: u64, instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut key = Vec::with_capacity(8 + 16);
    key.extend_from_slice(&fire_at_ms.to_be_bytes());
    key.extend_from_slice(&iid_bytes);
    key
}

<<<<<<< HEAD
<<<<<<< HEAD
/// Decode a timer key into fire-at timestamp and instance ID.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 24 bytes.
///
/// # Panics
///
/// Panics if the key is not exactly 24 bytes.
=======
#[expect(clippy::unwrap_used)]
>>>>>>> origin/vo-worker-tests
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn decode_timer_key(bytes: &[u8]) -> Result<(u64, InstanceId), KeyEncodingError> {
    if bytes.len() != 24 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 24,
            actual: bytes.len(),
        });
    }
<<<<<<< HEAD
    #[expect(clippy::unwrap_used)]
    let ts_bytes: [u8; 8] = bytes[..8].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
=======
    let ts_bytes: [u8; 8] = bytes[..8].try_into().unwrap();
>>>>>>> origin/polecat/synth-mnw6kj8v
    let iid_bytes: [u8; 16] = bytes[8..24].try_into().unwrap();
    Ok((
        u64::from_be_bytes(ts_bytes),
        InstanceId::from_bytes(iid_bytes),
    ))
}

<<<<<<< HEAD
/// Encode a lease key from instance ID and step ID.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
#[must_use]
pub fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
    format!("{instance_id}::{step_id}").into_bytes()
}

<<<<<<< HEAD
/// Decode a lease key into instance ID and step ID.
///
/// # Errors
///
/// Returns `KeyEncodingError::StepId` if the key is not valid UTF-8 or missing the `::` delimiter.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn decode_lease_key(bytes: &[u8]) -> Result<(InstanceId, StepId), KeyEncodingError> {
    let s = std::str::from_utf8(bytes).map_err(|e| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "LeaseKey",
            reason: e.to_string(),
        })
    })?;
    let (iid_str, sid_str) = s.split_once("::").ok_or_else(|| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "LeaseKey",
            reason: "missing :: delimiter".to_string(),
        })
    })?;
    let instance_id = InstanceId::parse(iid_str).map_err(KeyEncodingError::from)?;
    let step_id = StepId::parse(sid_str).map_err(KeyEncodingError::from)?;
    Ok((instance_id, step_id))
}

<<<<<<< HEAD
/// Encode a dedupe key as length-prefixed bytes.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
#[must_use]
pub fn encode_dedupe_key(idempotency_key: &str) -> Vec<u8> {
    encode_length_prefixed(idempotency_key.as_bytes())
}

<<<<<<< HEAD
/// Decode a length-prefixed dedupe key.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the data is too short.
/// Returns `KeyEncodingError::StepId` if the decoded bytes are not valid UTF-8.
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn decode_dedupe_key(bytes: &[u8]) -> Result<String, KeyEncodingError> {
    let (key_bytes, _) = decode_length_prefixed(bytes)?;
    String::from_utf8(key_bytes.to_vec()).map_err(|e| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "IdempotencyKey",
            reason: e.to_string(),
        })
    })
}

<<<<<<< HEAD
/// Encode an instance index key with status byte and creation time.
#[must_use]
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
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

<<<<<<< HEAD
/// Encode an effect key from instance ID and sequence number with effect marker.
#[must_use]
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn encode_effect_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(16 + 8 + 1);
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key.push(0xFF);
    key
}

<<<<<<< HEAD
<<<<<<< HEAD
/// Decode an effect key into instance ID and sequence number.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 25 bytes.
///
/// # Panics
///
/// Panics if the key is not exactly 25 bytes.
=======
#[expect(clippy::unwrap_used)]
>>>>>>> origin/vo-worker-tests
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn decode_effect_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() != 25 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 25,
            actual: bytes.len(),
        });
    }
    if bytes[24] != 0xFF {
        return Err(KeyEncodingError::InvalidLength {
            expected: 25,
            actual: bytes.len(),
        });
    }
<<<<<<< HEAD
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let seq_bytes: [u8; 8] = bytes[16..24].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(KeyEncodingError::InstanceId)?;
    Ok((instance_id, sequence))
}

/// Get the key prefix for all events of a given instance.
#[must_use]
=======
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    let seq_bytes: [u8; 8] = bytes[16..24].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(|e| KeyEncodingError::InstanceId(e))?;
    Ok((instance_id, sequence))
}

>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn get_event_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&iid_bytes);
    prefix
}

<<<<<<< HEAD
/// Get the key prefix for timers at or after a given timestamp.
#[must_use]
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn get_timer_key_prefix_for_time(fire_at_ms: u64) -> Vec<u8> {
    fire_at_ms.to_be_bytes().to_vec()
}

<<<<<<< HEAD
/// Get the key prefix for all lease entries of a given instance.
#[must_use]
pub fn get_lease_key_prefix_for_instance(instance_id: &InstanceId) -> Vec<u8> {
    format!("{instance_id}::").into_bytes()
}

/// Get the key prefix for a dedupe key.
#[must_use]
=======
pub fn get_lease_key_prefix_for_instance(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    iid_bytes.to_vec()
}

>>>>>>> origin/polecat/synth-mnw6kj8v
pub fn get_dedupe_key_prefix(idempotency_key: &str) -> Vec<u8> {
    encode_length_prefixed(idempotency_key.as_bytes())
}
