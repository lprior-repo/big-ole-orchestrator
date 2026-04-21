use crate::error::IpcError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

pub const MAX_PAYLOAD_SIZE: u32 = 10_485_760;

/// The envelope sent from Engine to Child over FD3.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Fd3Envelope {
    pub version: u8,
    pub instance_id: String,
    pub node_id: String,
    pub input: serde_json::Value,
    pub secrets: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
}

/// The envelope sent from Child to Engine over FD4.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fd4Envelope {
    pub version: u8,
    pub instance_id: String,
    pub node_id: String,
    pub result: TaskResult,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskResult {
    Success { output: serde_json::Value },
    Failure { error: TaskError },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

// --- Actions ---

/// Serializes and writes an envelope to the provided writer with a length prefix.
///
/// # Errors
/// * `IpcError::PayloadTooLarge` - if the serialized envelope exceeds 10MB.
/// * `IpcError::InvalidJson` - if serialization fails.
/// * `IpcError::IoError` - if writing to the writer fails.
pub fn write_envelope<T: Serialize>(writer: &mut impl Write, envelope: &T) -> Result<(), IpcError> {
    let payload = serialize_envelope(envelope)?;
    let len = payload.len();

    let Ok(len_u32) = u32::try_from(len) else {
        return Err(IpcError::PayloadTooLarge(u32::MAX));
    };

    if len_u32 > MAX_PAYLOAD_SIZE {
        return Err(IpcError::PayloadTooLarge(len_u32));
    }

    writer.write_all(&len_u32.to_be_bytes())?;
    writer.write_all(&payload)?;
    Ok(())
}

/// Reads a length prefix and then deserializes the envelope from the provided reader.
///
/// # Errors
/// * `IpcError::IncompleteRead` - if the reader ends before reading the full envelope.
/// * `IpcError::PayloadTooLarge` - if the length prefix exceeds 10MB.
/// * `IpcError::SchemaViolation` - if the length prefix is invalid or schema validation fails.
/// * `IpcError::InvalidJson` - if deserialization fails.
pub fn read_envelope<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, IpcError> {
    let mut header = Vec::with_capacity(4);
    let n_header = reader.by_ref().take(4).read_to_end(&mut header)?;

    if n_header < 4 {
        return Err(IpcError::IncompleteRead {
            expected: 4,
            actual: n_header,
        });
    }

    let len_buf: [u8; 4] = match header.try_into() {
        Ok(buf) => buf,
        Err(_) => return Err(IpcError::SchemaViolation("Invalid header".into())),
    };

    let len = u32::from_be_bytes(len_buf);

    if len > MAX_PAYLOAD_SIZE {
        return Err(IpcError::PayloadTooLarge(len));
    }

    let mut payload = Vec::with_capacity(len as usize);
    let n_payload = reader
        .by_ref()
        .take(u64::from(len))
        .read_to_end(&mut payload)?;

    if n_payload < len as usize {
        return Err(IpcError::IncompleteRead {
            expected: len as usize,
            actual: n_payload,
        });
    }

    deserialize_and_validate(&payload)
}

/// High-level engine function to read an envelope and validate its identity context.
///
/// # Errors
/// * `IpcError::IdentityMismatch` - if the `instance_id` or `node_id` doesn't match expected values.
/// * Also returns errors from `read_envelope`.
pub fn engine_receive_envelope(
    reader: &mut impl Read,
    expected_instance_id: &str,
    expected_node_id: &str,
) -> Result<Fd4Envelope, IpcError> {
    let envelope: Fd4Envelope = read_envelope(reader)?;
    validate_identity(&envelope, expected_instance_id, expected_node_id)?;
    Ok(envelope)
}

// --- Calculations ---

fn serialize_envelope<T: Serialize>(envelope: &T) -> Result<Vec<u8>, IpcError> {
    serde_json::to_vec(envelope).map_err(|e| IpcError::InvalidJson(e.to_string()))
}

fn deserialize_and_validate<T: DeserializeOwned>(payload: &[u8]) -> Result<T, IpcError> {
    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|e| IpcError::InvalidJson(e.to_string()))?;

    validate_json_schema(&value)?;

    serde_json::from_value(value).map_err(|e| {
        if e.is_data() {
            IpcError::SchemaViolation(e.to_string())
        } else {
            IpcError::InvalidJson(e.to_string())
        }
    })
}

fn validate_json_schema(value: &serde_json::Value) -> Result<(), IpcError> {
    let Some(obj) = value.as_object() else {
        return Ok(());
    };

    // Version check
    if let Some(v) = obj.get("version") {
        let version = match v.as_u64() {
            Some(n) => u8::try_from(n).map_err(|_| IpcError::VersionMismatch(255))?,
            None => {
                return Err(IpcError::SchemaViolation(
                    "version must be an integer".into(),
                ))
            }
        };
        if version != 1 {
            return Err(IpcError::VersionMismatch(version));
        }
    }

    // ID checks
    validate_id_field(obj, "instance_id")?;
    validate_id_field(obj, "node_id")?;

    Ok(())
}

fn validate_id_field(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<(), IpcError> {
    if let Some(val) = obj.get(field) {
        let Some(id) = val.as_str() else {
            return Err(IpcError::SchemaViolation(format!(
                "{field} must be a string"
            )));
        };
        if id.is_empty() {
            return Err(IpcError::SchemaViolation(format!(
                "{field} cannot be empty"
            )));
        }
        if !id.chars().all(char::is_alphanumeric) {
            return Err(IpcError::SchemaViolation(format!(
                "{field} contains invalid characters"
            )));
        }
    }
    Ok(())
}

/// Validates that the envelope matches the expected instance and node IDs.
///
/// # Errors
/// * `IpcError::IdentityMismatch` - if the `instance_id` or `node_id` doesn't match expected values.
pub fn validate_identity(
    envelope: &Fd4Envelope,
    expected_instance_id: &str,
    expected_node_id: &str,
) -> Result<(), IpcError> {
    if envelope.instance_id != expected_instance_id || envelope.node_id != expected_node_id {
        return Err(IpcError::IdentityMismatch {
            expected_instance: expected_instance_id.to_string(),
            expected_node: expected_node_id.to_string(),
            actual_instance: envelope.instance_id.clone(),
            actual_node: envelope.node_id.clone(),
        });
    }
    Ok(())
}
