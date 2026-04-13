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
    }
}

pub const PARTITION_EVENTS: &[u8] = b"events";
pub const PARTITION_TIMERS: &[u8] = b"timers";
pub const PARTITION_LEASES: &[u8] = b"leases";
pub const PARTITION_INSTANCES: &[u8] = b"instances";
pub const PARTITION_DEDUPE: &[u8] = b"dedupe";
pub const PARTITION_EFFECTS: &[u8] = b"effects";

#[must_use]
pub fn encode_u64_be(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

#[must_use]
pub fn encode_u16_be(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

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
    let len = value.len() as u16;
    let mut result = Vec::with_capacity(2 + value.len());
    result.extend_from_slice(&len.to_be_bytes());
    result.extend_from_slice(value);
    result
}

pub fn decode_length_prefixed<'a>(
    bytes: &'a [u8],
) -> Result<(&'a [u8], &'a [u8]), KeyEncodingError> {
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

#[must_use]
pub fn encode_instance_id(instance_id: &InstanceId) -> Result<[u8; 16], KeyEncodingError> {
    instance_id.to_bytes().map_err(KeyEncodingError::from)
}

pub fn decode_instance_id(bytes: &[u8]) -> Result<InstanceId, KeyEncodingError> {
    let arr: [u8; 16] = bytes
        .try_into()
        .map_err(|_| KeyEncodingError::InvalidLength {
            expected: 16,
            actual: bytes.len(),
        })?;
    Ok(InstanceId::from_bytes(arr))
}

#[must_use]
pub fn encode_step_id(step_id: &StepId) -> Vec<u8> {
    encode_length_prefixed(step_id.as_str().as_bytes())
}

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

#[must_use]
pub fn encode_sequence_number(seq: SequenceNumber) -> [u8; 8] {
    encode_u64_be(seq.as_u64())
}

pub fn decode_sequence_number(bytes: &[u8]) -> Result<SequenceNumber, KeyEncodingError> {
    let val = decode_u64_be(bytes)?;
    SequenceNumber::try_from(val).map_err(|e| KeyEncodingError::InstanceId(e))
}

pub fn encode_event_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(16 + 8);
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key
}

#[expect(clippy::unwrap_used)]
pub fn decode_event_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), KeyEncodingError> {
    if bytes.len() != 24 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 24,
            actual: bytes.len(),
        });
    }
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let seq_bytes: [u8; 8] = bytes[16..24].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(|e| KeyEncodingError::InstanceId(e))?;
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

#[expect(clippy::unwrap_used)]
pub fn decode_timer_key(bytes: &[u8]) -> Result<(u64, InstanceId), KeyEncodingError> {
    if bytes.len() != 24 {
        return Err(KeyEncodingError::InvalidLength {
            expected: 24,
            actual: bytes.len(),
        });
    }
    #[expect(clippy::unwrap_used)]
    let ts_bytes: [u8; 8] = bytes[..8].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[8..24].try_into().unwrap();
    Ok((
        u64::from_be_bytes(ts_bytes),
        InstanceId::from_bytes(iid_bytes),
    ))
}

#[must_use]
pub fn encode_lease_key(instance_id: &InstanceId, step_id: &StepId) -> Vec<u8> {
    format!("{instance_id}::{step_id}").into_bytes()
}

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

#[must_use]
pub fn encode_dedupe_key(idempotency_key: &str) -> Vec<u8> {
    encode_length_prefixed(idempotency_key.as_bytes())
}

pub fn decode_dedupe_key(bytes: &[u8]) -> Result<String, KeyEncodingError> {
    let (key_bytes, _) = decode_length_prefixed(bytes)?;
    String::from_utf8(key_bytes.to_vec()).map_err(|e| {
        KeyEncodingError::StepId(ParseError::InvalidFormat {
            type_name: "IdempotencyKey",
            reason: e.to_string(),
        })
    })
}

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

pub fn encode_effect_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let seq_bytes = encode_sequence_number(sequence);
    let mut key = Vec::with_capacity(16 + 8 + 1);
    key.extend_from_slice(&iid_bytes);
    key.extend_from_slice(&seq_bytes);
    key.push(0xFF);
    key
}

#[expect(clippy::unwrap_used)]
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
    #[expect(clippy::unwrap_used)]
    let iid_bytes: [u8; 16] = bytes[..16].try_into().unwrap();
    #[expect(clippy::unwrap_used)]
    let seq_bytes: [u8; 8] = bytes[16..24].try_into().unwrap();
    let instance_id = InstanceId::from_bytes(iid_bytes);
    let sequence = SequenceNumber::try_from(u64::from_be_bytes(seq_bytes))
        .map_err(|e| KeyEncodingError::InstanceId(e))?;
    Ok((instance_id, sequence))
}

pub fn get_event_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&iid_bytes);
    prefix
}

pub fn get_timer_key_prefix_for_time(fire_at_ms: u64) -> Vec<u8> {
    fire_at_ms.to_be_bytes().to_vec()
}

pub fn get_lease_key_prefix_for_instance(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    iid_bytes.to_vec()
}

pub fn get_dedupe_key_prefix(idempotency_key: &str) -> Vec<u8> {
    encode_length_prefixed(idempotency_key.as_bytes())
}
