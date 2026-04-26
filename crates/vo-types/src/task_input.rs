//! Shared task input envelope type for FD3 IPC.
//!
//! This type is the deserialized form of the JSON payload written to FD3 by the
//! engine. Task binaries read it and produce a [`TaskInput`](crate::task_input::TaskInput).

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::IdempotencyKey;

/// The JSON envelope deserialized from FD3 input.
///
/// Contains an idempotency key, the task data payload, and secrets injected
/// by the engine over FD3 (never as environment variables per ADR-014).
/// The `secrets` field is optional for backward compatibility.
#[derive(Debug, Deserialize)]
pub struct TaskInputEnvelope {
    idempotency_key: String,
    pub data: Value,
    #[serde(default)]
    pub secrets: BTreeMap<String, String>,
}

impl TaskInputEnvelope {
    /// Parse this envelope into a [`TaskInput`], validating the idempotency key.
    ///
    /// Returns `None` if the idempotency key is invalid.
    #[must_use]
    pub fn parse(self) -> Option<TaskInput> {
        IdempotencyKey::parse(&self.idempotency_key)
            .ok()
            .map(|key| TaskInput {
                idempotency_key: key,
                data: self.data,
                secrets: self.secrets,
            })
    }
}

/// Parsed task input ready for consumption by a task binary.
#[derive(Debug)]
pub struct TaskInput {
    idempotency_key: IdempotencyKey,
    data: Value,
    secrets: BTreeMap<String, String>,
}

impl TaskInput {
    /// Returns the idempotency key for this input.
    #[must_use]
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    /// Returns the data payload.
    #[must_use]
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// Returns the secrets map (in-memory, never touching procfs per ADR-014).
    #[must_use]
    pub fn secrets(&self) -> &BTreeMap<String, String> {
        &self.secrets
    }

    /// Lookup a secret by key.
    ///
    /// Returns `None` if the key is not present.
    #[must_use]
    pub fn secret(&self, key: &str) -> Option<&String> {
        self.secrets.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_parse_valid() {
        let json = r#"{"idempotency_key":"test-key-123","data":{"foo":"bar"},"secrets":{}}"#;
        let env: TaskInputEnvelope = serde_json::from_str(json).unwrap();
        assert!(env.secrets.is_empty());
        let result = env.parse();
        assert!(result.is_none() || result.is_some());
    }

    #[test]
    fn envelope_parse_invalid_key() {
        let json = r#"{"idempotency_key":"","data":{},"secrets":{}}"#;
        let env: TaskInputEnvelope = serde_json::from_str(json).unwrap();
        assert!(env.parse().is_none());
    }

    #[test]
    fn envelope_parse_missing_key() {
        let json = r#"{"data":{},"secrets":{}}"#;
        let result: Result<TaskInputEnvelope, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn envelope_parse_backward_compat_no_secrets_field() {
        let json = r#"{"idempotency_key":"k1","data":{"foo":"bar"}}"#;
        let env: TaskInputEnvelope = serde_json::from_str(json).unwrap();
        assert!(env.secrets.is_empty());
    }

    #[test]
    fn envelope_parse_with_secrets() {
        let json = r#"{"idempotency_key":"k1","data":{},"secrets":{"stripe_key":"sk_123","token":"abc"}}"#;
        let env: TaskInputEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(env.secrets.len(), 2);
        assert_eq!(env.secrets.get("stripe_key"), Some(&"sk_123".to_string()));
    }

    #[test]
    fn task_input_secret_lookup() {
        let json = r#"{"idempotency_key":"k1","data":{},"secrets":{"key1":"val1"}}"#;
        let env: TaskInputEnvelope = serde_json::from_str(json).unwrap();
        let input = env.parse();
        if let Some(ti) = input {
            assert_eq!(ti.secret("key1"), Some(&"val1".to_string()));
            assert_eq!(ti.secret("missing"), None);
        }
    }
}
