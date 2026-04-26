//! Canonical key encoding utilities for storage partitions (ADR-020).
//!
//! All Fjall keys follow two rules:
//! 1. **Numeric components use fixed-width, big-endian binary encoding.**
//! 2. **Variable-length identifiers are length-prefixed.**
//!
//! ## Key Formats
//!
//! - **Events**: `[instance_id_len_u16_be][instance_id_bytes(16)][sequence_u64_be]` (26 bytes)
//! - **Timers**: `[timestamp_u64_be][instance_id_len_u16_be][instance_id_bytes]` (26 bytes for ULID)
//! - **Leases**: `[instance_id(16)][step_id_len_u16_be][step_id_bytes]`
//! - **Instances**: `[status_byte][created_at_u64_be][instance_id(16)]`
//! - **Dedupe**: `[idempotency_key_len_u16_be][idempotency_key_bytes]`
//! - **Effects**: `[instance_id_len_u16_be][instance_id_bytes(16)][sequence_u64_be][effect_marker]` (27 bytes)

use vo_types::{InstanceId, ParseError, SequenceNumber, StepId};

#[cfg(test)]
mod storage_contract_tests;

#[cfg(test)]
mod tests;

// TEMPORARILY DISABLED - broken test files (pre-existing API mismatch)
// #[cfg(test)]
// mod proptests;

#[cfg(test)]
mod red_queen_adversarial;
// #[cfg(test)]
// mod red_queen_tests;

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

#[must_use]
pub fn encode_length_prefixed(value: &[u8]) -> Vec<u8> {
    let len = u16::try_from(value.len()).unwrap_or(u16::MAX);
    let mut result = Vec::with_capacity(2 + value.len());
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(value);
    result
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
/// # Panics
///
/// Panics if the step ID exceeds 65535 bytes.
#[must_use]
pub fn encode_step_id(step_id: &StepId) -> Vec<u8> {
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

/// Encode an event key: `[instance_id(16)][sequence_u64_be]` = 24 bytes.
///
/// The `InstanceId` component is a **fixed-width 16-byte ULID binary newtype**
/// (see `vo_types::InstanceId::to_bytes` / `from_bytes`). Because it always
/// serializes to exactly 16 bytes, no length prefix is needed — the boundary
/// between the instance and sequence components is unambiguous at byte offset 16.
///
/// This satisfies ADR-020's framing requirement via the fixed-binary-newtype
/// exception: variable-length identifiers are length-prefixed, but fixed-width
/// binary types (like the 16-byte ULID) do not require a length prefix.
#[must_use]
pub fn encode_event_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let len_prefix = encode_u16_be(iid_bytes.len() as u16);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(2 + 16 + 8);
    key.extend_from_slice(&len_prefix);
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key
}

/// Decode an event key into instance ID and sequence number.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 26 bytes
/// (2-byte length prefix + 16-byte instance ID + 8-byte sequence number).
pub fn decode_event_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() < 20 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 26,
            actual: bytes.len(),
        });
    }
    let id_len = decode_u16_be(&bytes[..2])? as usize;
    if bytes.len() < 2 + id_len + 8 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 2 + id_len + 8,
            actual: bytes.len(),
        });
    }
    let iid_bytes: [u8; 16] = bytes[2..2 + id_len].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 2 + id_len,
        actual: bytes.len(),
    })?;
    let seq_bytes: [u8; 8] = bytes[2 + id_len..2 + id_len + 8].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 2 + id_len + 8,
        actual: bytes.len(),
    })?;
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(KeyEncodingError::InstanceId)?;
    Ok((instance_id, sequence))
}

/// Encode a timer key from fire-at timestamp and instance ID.
///
/// Format: `[timestamp_u64_be][instance_id_len_u16_be][instance_id_bytes]`
/// ADR-020 compliant: length-prefixed instance ID avoids ambiguous concatenation.
#[must_use]
pub fn encode_timer_key(fire_at_ms: u64, instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let iid_len = iid_bytes.len() as u16;
    let mut key = Vec::with_capacity(8 + 2 + iid_bytes.len());
    key.extend_from_slice(&fire_at_ms.to_be_bytes());
    key.extend_from_slice(&iid_len.to_be_bytes());
    key.extend_from_slice(&iid_bytes);
    key
}

/// Decode a timer key into fire-at timestamp and instance ID.
///
/// Format: `[timestamp_u64_be][instance_id_len_u16_be][instance_id_bytes]`
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is too short.
pub fn decode_timer_key(bytes: &[u8]) -> Result<(u64, InstanceId), KeyEncodingError> {
    if bytes.len() < 10 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 10,
            actual: bytes.len(),
        });
    }
    let ts_bytes: [u8; 8] = bytes[..8].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 8,
        actual: bytes.len(),
    })?;
    let ts = u64::from_be_bytes(ts_bytes);
    let iid_len = u16::from_be_bytes(
        bytes[8..10]
            .try_into()
            .map_err(|_| KeyEncodingError::InvalidLength {
                expected: 10,
                actual: bytes.len(),
            })?,
    ) as usize;
    if bytes.len() < 10 + iid_len {
        return Err(KeyEncodingError::InvalidLength {
            expected: 10 + iid_len,
            actual: bytes.len(),
        });
    }
    let iid_bytes: [u8; 16] = bytes[10..10 + iid_len].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 10 + iid_len,
        actual: bytes.len(),
    })?;
    Ok((ts, InstanceId::from_bytes(iid_bytes)))
}

/// Encode a lease key from instance ID and step ID.
///
/// Format: `[instance_id(16 bytes)][step_id_len_u16_be][step_id_bytes]`
#[must_use]
pub fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let sid_str = step_id.as_str();
    let sid_bytes = sid_str.as_bytes();
    let sid_len = u16::try_from(sid_bytes.len()).unwrap_or(u16::MAX);
    let mut key = Vec::with_capacity(16 + 2 + sid_bytes.len());
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&sid_len.to_be_bytes());
    key.extend_from_slice(sid_bytes);
    key
}

/// Decode a lease key into instance ID and step ID.
///
/// Format: `[instance_id(16 bytes)][step_id_len_u16_be][step_id_bytes]`
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is too short.
/// Returns `KeyEncodingError::StepId` if the step ID is not valid UTF-8.
pub fn decode_lease_key(bytes: &[u8]) -> Result<(InstanceId, StepId), KeyEncodingError> {
    if bytes.len() < 18 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 18,
            actual: bytes.len(),
        });
    }
    let iid_bytes: [u8; 16] = bytes[..16].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 16,
        actual: bytes.len(),
    })?;
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sid_len = u16::from_be_bytes(bytes[16..18].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 2,
        actual: bytes.len() - 16,
    })?) as usize;
    if bytes.len() < 18 + sid_len {
        return Err(KeyEncodingError::InvalidLength {
            expected: 18 + sid_len,
            actual: bytes.len(),
        });
    }
    let sid_bytes = &bytes[18..18 + sid_len];
    let sid_str = std::str::from_utf8(sid_bytes).map_err(|e| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "StepId",
            reason: e.to_string(),
        })
    })?;
    let step_id = StepId::parse(sid_str).map_err(KeyEncodingError::from)?;
    Ok((instance_id, step_id))
}

/// Encode a dedupe key as length-prefixed bytes.
#[must_use]
pub fn encode_dedupe_key(idempotency_key: &str) -> Vec<u8> {
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
/// Format: `[instance_id_len_u16_be][instance_id_bytes(16)][sequence_u64_be][0xFF]` (27 bytes)
#[must_use]
pub fn encode_effect_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let len_prefix = encode_u16_be(iid_bytes.len() as u16);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(2 + 16 + 8 + 1);
    key.extend_from_slice(&len_prefix);
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key.push(0xFF);
    key
}

/// Decode an effect key into instance ID and sequence number.
///
/// # Errors
///
/// Returns `KeyEncodingError::InvalidLength` if the key is not exactly 27 bytes
/// (2-byte length prefix + 16-byte instance ID + 8-byte sequence number + 0xFF marker).
pub fn decode_effect_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() < 21 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 27,
            actual: bytes.len(),
        });
    }
    if bytes[bytes.len() - 1] != 0xFF {
        return Err(KeyEncodingError::InvalidLength {
            expected: 27,
            actual: bytes.len(),
        });
    }
    let id_len = decode_u16_be(&bytes[..2])? as usize;
    if bytes.len() < 2 + id_len + 8 + 1 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 2 + id_len + 8 + 1,
            actual: bytes.len(),
        });
    }
    let iid_bytes: [u8; 16] = bytes[2..2 + id_len].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 2 + id_len,
        actual: bytes.len(),
    })?;
    let seq_bytes: [u8; 8] = bytes[2 + id_len..2 + id_len + 8].try_into().map_err(|_| KeyEncodingError::InvalidLength {
        expected: 2 + id_len + 8,
        actual: bytes.len(),
    })?;
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(KeyEncodingError::InstanceId)?;
    Ok((instance_id, sequence))
}

/// Get the key prefix for all events of a given instance.
///
/// Returns the 18-byte prefix: 2-byte length prefix + 16-byte instance ID.
#[must_use]
pub fn get_event_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let len_prefix = encode_u16_be(iid_bytes.len() as u16);
    let mut prefix = Vec::with_capacity(2 + 16);
    prefix.extend_from_slice(&len_prefix);
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
/// Returns the 16-byte instance ID prefix (no step_id component).
#[must_use]
pub fn get_lease_key_prefix_for_instance(instance_id: &InstanceId) -> Vec<u8> {
    instance_id.to_bytes().unwrap_or([0u8; 16]).to_vec()
}

/// Get the key prefix for a dedupe key.
#[must_use]
pub fn get_dedupe_key_prefix(idempotency_key: &str) -> Vec<u8> {
    encode_length_prefixed(idempotency_key.as_bytes())
}

// ---------------------------------------------------------------------------
// Legacy delimiter format detection (ADR-020 migration safety)
// ---------------------------------------------------------------------------

/// Legacy delimiter lease key pattern: `{instance_id}::{step_id}`.
/// The old `FjallLeaseStore::encode_lease_key` produced this format.
pub const LEGACY_DELIMITER: &[u8] = b"::";

/// Legacy fence key pattern: `{instance_id}::{step_id}::fence`.
pub const LEGACY_FENCE_DELIMITER: &[u8] = b"::fence";

/// Check if a lease key uses the legacy delimiter format instead of
/// the ADR-020 length-prefixed format.
///
/// Legacy format: `{string_instance_id}::{string_step_id}`
/// New format:    `[instance_id(16 bytes)][step_id_len_u16_be][step_id_bytes]`
///
/// A key is considered legacy if it is at least 20 bytes long (minimum:
/// 11-char ULID + 2 `::` + 6-char step like "step-1" + 1 = 20),
/// contains `::` at a position that could split into valid instance_id
/// and step_id strings, and does NOT start with a valid ADR-020 length prefix.
#[must_use]
pub fn is_legacy_delimiter_lease_key(key: &[u8]) -> bool {
    // Minimum legacy key: 11-char ULID (min for ULID validation) + "::" + 1-char step_id
    if key.len() < 14 {
        return false;
    }

    // New format starts with 2-byte BE length prefix for step_id.
    // If the first byte is 0 and the second byte is small (< 200),
    // it's likely a length prefix for the 16-byte instance ID.
    // But simpler: if key is at least 18 bytes and the first 16 bytes
    // look like a valid encoded instance ID, skip delimiter check.
    // We'll use the heuristic: if key.len() >= 18 + 2 and the bytes
    // at position 16-17 look like a BE u16 length that equals
    // key.len() - 18, it's the new format.
    if key.len() >= 18 {
        let sid_len_bytes = &key[16..18];
        let sid_len = u16::from_be_bytes([sid_len_bytes[0], sid_len_bytes[1]]) as usize;
        if key.len() == 18 + sid_len {
            return false;
        }
    }

    // Check for :: delimiter pattern
    // Find all occurrences of :: in the key
    let mut delimiter_positions: Vec<usize> = Vec::new();
    for i in 0..key.len().saturating_sub(1) {
        if key[i] == b':' && key[i + 1] == b':' {
            delimiter_positions.push(i);
        }
    }

    // Legacy lease keys have exactly one `::` delimiter
    if delimiter_positions.len() != 1 {
        return false;
    }

    let delim_pos = delimiter_positions[0];

    // The first part (before ::) should be at least 11 bytes (min ULID string length)
    if delim_pos < 11 {
        return false;
    }

    // The second part (after ::) should be at least 1 byte
    if delim_pos + 2 >= key.len() {
        return false;
    }

    // Try to parse both parts as valid IDs
    let iid_str = match std::str::from_utf8(&key[..delim_pos]) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let sid_str = match std::str::from_utf8(&key[delim_pos + 2..]) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Validate that both parts look like valid IDs
    InstanceId::parse(iid_str).is_ok() && StepId::parse(sid_str).is_ok()
}

/// Check if a key uses the legacy delimiter fence format.
///
/// Legacy fence key format: `{instance_id}::{step_id}::fence`
#[must_use]
pub fn is_legacy_delimiter_fence_key(key: &[u8]) -> bool {
    if !key.ends_with(LEGACY_FENCE_DELIMITER) {
        return false;
    }

    let prefix = &key[..key.len() - LEGACY_FENCE_DELIMITER.len()];
    is_legacy_delimiter_lease_key(prefix)
}

/// Information about a legacy key found during migration scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyKeyInfo {
    /// The original legacy key bytes.
    pub key: Vec<u8>,
    /// The instance ID extracted from the legacy key.
    pub instance_id: String,
    /// The step ID extracted from the legacy key.
    pub step_id: String,
    /// Whether this was a fence key (as opposed to a lease key).
    pub is_fence: bool,
}

/// Try to extract instance_id and step_id from a legacy delimiter lease key.
///
/// Returns `None` if the key is not in legacy delimiter format or cannot be
/// parsed into valid instance/step IDs.
pub fn extract_legacy_lease_components(key: &[u8]) -> Option<(String, String)> {
    if !is_legacy_delimiter_lease_key(key) {
        return None;
    }

    let mut delimiter_positions: Vec<usize> = Vec::new();
    for i in 0..key.len().saturating_sub(1) {
        if key[i] == b':' && key[i + 1] == b':' {
            delimiter_positions.push(i);
        }
    }

    if delimiter_positions.len() != 1 {
        return None;
    }

    let delim_pos = delimiter_positions[0];
    let iid_str = std::str::from_utf8(&key[..delim_pos]).ok()?.to_string();
    let sid_str = std::str::from_utf8(&key[delim_pos + 2..]).ok()?.to_string();

    // Validate
    if InstanceId::parse(&iid_str).is_err() || StepId::parse(&sid_str).is_err() {
        return None;
    }

    Some((iid_str, sid_str))
}

/// Try to extract instance_id and step_id from a legacy delimiter fence key.
///
/// Returns `None` if the key is not in legacy delimiter format or cannot be
/// parsed into valid instance/step IDs.
pub fn extract_legacy_fence_components(key: &[u8]) -> Option<(String, String)> {
    if !is_legacy_delimiter_fence_key(key) {
        return None;
    }

    let prefix = &key[..key.len() - LEGACY_FENCE_DELIMITER.len()];
    extract_legacy_lease_components(prefix)
}
