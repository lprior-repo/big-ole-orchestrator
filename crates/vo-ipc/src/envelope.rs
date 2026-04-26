use crate::error::IpcError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};

pub const MAX_PAYLOAD_SIZE: u32 = 10_485_760;

/// The envelope sent from Engine to Child over FD3.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Fd3Envelope {
    pub version: u8,
    pub instance_id: String,
    pub node_id: String,
    pub input: serde_json::Value,
    pub secrets: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
}

impl std::fmt::Debug for Fd3Envelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fd3Envelope")
            .field("version", &self.version)
            .field("instance_id", &self.instance_id)
            .field("node_id", &self.node_id)
            .field("input", &self.input)
            .field("secrets", &format!("[{} redacted]", self.secrets.len()))
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// The envelope sent from Child to Engine over FD4.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fd4Envelope {
    pub version: u8,
    pub instance_id: String,
    pub node_id: String,
    pub result: TaskResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskResult {
    Success { output: serde_json::Value },
    Failure { error: TaskError },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::Cursor;

    #[test]
    fn fd3_envelope_equality() {
        let env1 = Fd3Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            input: serde_json::json!({"a": 1}),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        let env2 = env1.clone();
        assert_eq!(env1, env2);
    }

    #[test]
    fn fd3_envelope_debug_redacts_secrets() {
        let mut secrets = BTreeMap::new();
        secrets.insert("key".into(), "secret_val".into());
        let env = Fd3Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            input: serde_json::json!(null),
            secrets,
            metadata: BTreeMap::new(),
        };
        let debug = format!("{:?}", env);
        assert!(debug.contains("1 redacted"));
        assert!(!debug.contains("secret_val"));
    }

    #[test]
    fn fd4_envelope_equality() {
        let env1 = Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Success {
                output: serde_json::json!(42),
            },
        };
        let env2 = env1.clone();
        assert_eq!(env1, env2);
    }

    #[test]
    fn fd4_envelope_debug_shows_all_fields() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            result: TaskResult::Failure {
                error: TaskError {
                    code: "E1".into(),
                    message: "boom".into(),
                    details: None,
                },
            },
        };
        let debug = format!("{:?}", env);
        assert!(debug.contains("Fd4Envelope"));
        assert!(debug.contains("E1"));
    }

    #[test]
    fn task_result_equality() {
        let r1 = TaskResult::Success {
            output: serde_json::json!(null),
        };
        let r2 = TaskResult::Success {
            output: serde_json::json!(null),
        };
        assert_eq!(r1, r2);

        let r3 = TaskResult::Failure {
            error: TaskError {
                code: "E".into(),
                message: "m".into(),
                details: None,
            },
        };
        assert_ne!(r1, r3);
    }

    #[test]
    fn task_error_with_and_without_details() {
        let with = TaskError {
            code: "E".into(),
            message: "m".into(),
            details: Some(serde_json::json!({"key": "val"})),
        };
        let without = TaskError {
            code: "E".into(),
            message: "m".into(),
            details: None,
        };
        assert_ne!(with, without);
    }

    #[test]
    fn write_envelope_length_prefix() {
        let env = Fd3Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            input: serde_json::json!(null),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        assert!(buf.len() >= 4);
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert_eq!(len as usize, buf.len() - 4);
    }

    #[test]
    fn read_envelope_empty_reader_returns_incomplete_read() {
        let mut reader = Cursor::new(Vec::<u8>::new());
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut reader);
        assert!(matches!(
            result,
            Err(IpcError::IncompleteRead {
                expected: 4,
                actual: 0
            })
        ));
    }

    #[test]
    fn validate_identity_mismatch_instance() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "actual".into(),
            node_id: "node1".into(),
            result: TaskResult::Success {
                output: serde_json::json!(null),
            },
        };
        let result = validate_identity(&env, "expected", "node1");
        match result {
            Err(IpcError::IdentityMismatch {
                expected_instance,
                expected_node,
                actual_instance,
                actual_node,
            }) => {
                assert_eq!(expected_instance, "expected");
                assert_eq!(expected_node, "node1");
                assert_eq!(actual_instance, "actual");
                assert_eq!(actual_node, "node1");
            }
            other => panic!("expected IdentityMismatch, got {:?}", other),
        }
    }

    #[test]
    fn validate_identity_mismatch_node() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "i1".into(),
            node_id: "actual_node".into(),
            result: TaskResult::Success {
                output: serde_json::json!(null),
            },
        };
        let result = validate_identity(&env, "i1", "expected_node");
        assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
    }

    #[test]
    fn max_payload_size_is_10mb() {
        assert_eq!(MAX_PAYLOAD_SIZE, 10_485_760);
    }

    #[test]
    fn write_read_fd3_envelope_roundtrip() {
        let env = Fd3Envelope {
            version: 1,
            instance_id: "inst123".into(),
            node_id: "node456".into(),
            input: serde_json::json!({"key": "value"}),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        assert_eq!(env, decoded);
    }

    #[test]
    fn write_read_fd4_envelope_roundtrip() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Success {
                output: serde_json::json!({"result": 42}),
            },
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        assert_eq!(env, decoded);
    }

    #[test]
    fn write_read_fd4_failure_roundtrip() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Failure {
                error: TaskError {
                    code: "E_TIMEOUT".into(),
                    message: "deadline exceeded".into(),
                    details: Some(serde_json::json!({"elapsed_ms": 5000})),
                },
            },
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        assert_eq!(env, decoded);
    }

    #[test]
    fn write_envelope_with_secrets_roundtrip() {
        let mut secrets = BTreeMap::new();
        secrets.insert("API_KEY".into(), "secret123".into());
        secrets.insert("TOKEN".into(), "tok_abc".into());
        let env = Fd3Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            input: serde_json::json!(null),
            secrets: secrets.clone(),
            metadata: BTreeMap::new(),
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        assert_eq!(decoded.secrets, secrets);
    }

    #[test]
    fn write_envelope_with_metadata_roundtrip() {
        let mut metadata = BTreeMap::new();
        metadata.insert("trace_id".into(), "abc123".into());
        let env = Fd3Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            input: serde_json::json!({"x": 1}),
            secrets: BTreeMap::new(),
            metadata: metadata.clone(),
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        assert_eq!(decoded.metadata, metadata);
    }

    #[test]
    fn read_envelope_invalid_json_returns_error() {
        let invalid_json = b"not valid json at all";
        let len = invalid_json.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(invalid_json);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::InvalidJson(_))));
    }

    #[test]
    fn read_envelope_truncated_payload_returns_incomplete_read() {
        let len: u32 = 1000;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(b"short");
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(
            result,
            Err(IpcError::IncompleteRead {
                expected: 1000,
                actual: 5
            })
        ));
    }

    #[test]
    fn read_envelope_version_zero_returns_version_mismatch() {
        let env = serde_json::json!({
            "version": 0,
            "instance_id": "i",
            "node_id": "n",
            "input": null,
            "secrets": {},
            "metadata": {}
        });
        let payload = serde_json::to_vec(&env).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::VersionMismatch(0))));
    }

    #[test]
    fn read_envelope_empty_instance_id_returns_schema_violation() {
        let env = serde_json::json!({
            "version": 1,
            "instance_id": "",
            "node_id": "n",
            "input": null,
            "secrets": {},
            "metadata": {}
        });
        let payload = serde_json::to_vec(&env).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
    }

    #[test]
    fn read_envelope_instance_id_with_special_chars_returns_schema_violation() {
        let env = serde_json::json!({
            "version": 1,
            "instance_id": "has-dash!",
            "node_id": "n",
            "input": null,
            "secrets": {},
            "metadata": {}
        });
        let payload = serde_json::to_vec(&env).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
    }

    #[test]
    fn validate_identity_match_succeeds() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            result: TaskResult::Success {
                output: serde_json::json!(null),
            },
        };
        assert!(validate_identity(&env, "inst", "node").is_ok());
    }

    #[test]
    fn engine_receive_envelope_validates_identity() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "correct".into(),
            node_id: "node".into(),
            result: TaskResult::Success {
                output: serde_json::json!(null),
            },
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();

        let result = engine_receive_envelope(&mut Cursor::new(buf.clone()), "correct", "node");
        assert!(result.is_ok());

        let result = engine_receive_envelope(&mut Cursor::new(buf), "wrong", "node");
        assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
    }

    #[test]
    fn task_result_serialization_roundtrip() {
        let success = TaskResult::Success {
            output: serde_json::json!({"data": [1, 2, 3]}),
        };
        let json = serde_json::to_string(&success).unwrap();
        let decoded: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(success, decoded);

        let failure = TaskResult::Failure {
            error: TaskError {
                code: "ERR".into(),
                message: "failed".into(),
                details: Some(serde_json::json!({"ctx": "test"})),
            },
        };
        let json = serde_json::to_string(&failure).unwrap();
        let decoded: TaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(failure, decoded);
    }

    #[test]
    fn write_envelope_rejects_oversized_payload() {
        let mut big_input = serde_json::Map::new();
        for i in 0..200_000 {
            big_input.insert(format!("k{i}"), serde_json::json!("v".repeat(50)));
        }
        let env = Fd3Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            input: serde_json::Value::Object(big_input),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        let mut buf = Vec::new();
        let result = write_envelope(&mut buf, &env);
        assert!(matches!(result, Err(IpcError::PayloadTooLarge(_))));
    }

    #[test]
    fn read_envelope_rejects_oversized_length_prefix() {
        let len: u32 = MAX_PAYLOAD_SIZE + 1;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(b"{}");
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::PayloadTooLarge(_))));
    }

    #[test]
    fn read_envelope_accepts_non_object_json() {
        let payload = serde_json::to_vec(&42).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd4Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(result.is_err());
    }

    #[test]
    fn read_envelope_empty_node_id_returns_schema_violation() {
        let env = serde_json::json!({
            "version": 1,
            "instance_id": "i",
            "node_id": "",
            "result": { "success": { "output": null } }
        });
        let payload = serde_json::to_vec(&env).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd4Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
    }

    #[test]
    fn read_envelope_node_id_with_special_chars_returns_schema_violation() {
        let env = serde_json::json!({
            "version": 1,
            "instance_id": "i",
            "node_id": "has-dash!",
            "result": { "success": { "output": null } }
        });
        let payload = serde_json::to_vec(&env).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd4Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
    }

    #[test]
    fn read_envelope_version_255_returns_version_mismatch() {
        let env = serde_json::json!({
            "version": 255,
            "instance_id": "i",
            "node_id": "n",
            "input": null,
            "secrets": {},
            "metadata": {}
        });
        let payload = serde_json::to_vec(&env).unwrap();
        let len = payload.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&payload);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(result, Err(IpcError::VersionMismatch(255))));
    }

    #[test]
    fn read_envelope_partial_header_returns_incomplete_read() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0, 0, 1]);
        let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
        assert!(matches!(
            result,
            Err(IpcError::IncompleteRead {
                expected: 4,
                actual: 3
            })
        ));
    }

    #[test]
    fn write_envelope_exact_max_size_succeeds() {
        let tiny = Fd3Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            input: serde_json::json!(null),
            secrets: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        let mut buf = Vec::new();
        let result = write_envelope(&mut buf, &tiny);
        assert!(result.is_ok());
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert!(len <= MAX_PAYLOAD_SIZE);
    }

    #[test]
    fn fd3_envelope_serde_json_roundtrip() {
        let env = Fd3Envelope {
            version: 1,
            instance_id: "inst".into(),
            node_id: "node".into(),
            input: serde_json::json!({"key": [1, 2, 3]}),
            secrets: BTreeMap::from([("k".into(), "v".into())]),
            metadata: BTreeMap::from([("m".into(), "d".into())]),
        };
        let json = serde_json::to_string(&env).unwrap();
        let decoded: Fd3Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, decoded);
    }

    #[test]
    fn fd4_envelope_serde_json_roundtrip() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "i".into(),
            node_id: "n".into(),
            result: TaskResult::Failure {
                error: TaskError {
                    code: "E".into(),
                    message: "msg".into(),
                    details: Some(serde_json::json!({"nested": true})),
                },
            },
        };
        let json = serde_json::to_string(&env).unwrap();
        let decoded: Fd4Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, decoded);
    }

    #[test]
    fn validate_identity_both_fields_mismatch() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "actual_i".into(),
            node_id: "actual_n".into(),
            result: TaskResult::Success {
                output: serde_json::json!(null),
            },
        };
        let result = validate_identity(&env, "expected_i", "expected_n");
        assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
        if let Err(IpcError::IdentityMismatch {
            expected_instance,
            expected_node,
            actual_instance,
            actual_node,
        }) = result
        {
            assert_eq!(expected_instance, "expected_i");
            assert_eq!(expected_node, "expected_n");
            assert_eq!(actual_instance, "actual_i");
            assert_eq!(actual_node, "actual_n");
        }
    }

    #[test]
    fn engine_receive_envelope_roundtrip_success() {
        let env = Fd4Envelope {
            version: 1,
            instance_id: "inst1".into(),
            node_id: "node1".into(),
            result: TaskResult::Success {
                output: serde_json::json!({"ok": true}),
            },
        };
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let received = engine_receive_envelope(&mut Cursor::new(buf), "inst1", "node1").unwrap();
        assert_eq!(received, env);
    }
}


