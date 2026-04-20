//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

use serde::{Deserialize, Serialize};
use std::fmt;

mod error;
pub use crate::error::VoError;

const MAX_ID_LENGTH: usize = 256;

/// Validates that an ID string is valid (non-empty, reasonable length, no control chars).
fn validate_id(id: &str, id_type: &str) -> Result<(), VoError> {
    if id.is_empty() {
        return Err(VoError::validation(format!("{} cannot be empty", id_type)));
    }
    if id.len() > MAX_ID_LENGTH {
        return Err(VoError::validation(format!(
            "{} cannot exceed {} characters (got {})",
            id_type,
            MAX_ID_LENGTH,
            id.len()
        )));
    }
    if id.chars().any(|c| c.is_control()) {
        return Err(VoError::validation(format!(
            "{} cannot contain control characters",
            id_type
        )));
    }
    Ok(())
}

/// Newtype wrapper for namespace identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamespaceId(String);

impl NamespaceId {
    /// Creates a new NamespaceId with validation.
    pub fn new(id: impl Into<String>) -> Result<Self, VoError> {
        let id = id.into();
        validate_id(&id, "NamespaceId")?;
        Ok(Self(id))
    }

    /// Creates a new NamespaceId without validation (unsafe - use with caution).
    pub unsafe fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the NamespaceId and returns the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for NamespaceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NamespaceId {
    fn from(s: &str) -> Self {
        Self::new(s).expect("NamespaceId validation failed")
    }
}

impl From<String> for NamespaceId {
    fn from(s: String) -> Self {
        Self::new(s).expect("NamespaceId validation failed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowEvent {
    TimerFired {
        timer_id: String,
        timestamp_ms: u64,
    },
    TaskCompleted {
        task_id: String,
        result_json: String,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    SignalReceived {
        signal_name: String,
        payload_json: String,
    },
    WorkflowStarted {
        workflow_id: String,
        input_json: String,
    },
    WorkflowCompleted {
        workflow_id: String,
        result_json: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_id_new_valid() {
        let id = NamespaceId::new("namespace-abc").unwrap();
        assert_eq!(id.as_str(), "namespace-abc");
    }

    #[test]
    fn namespace_id_new_empty_rejects() {
        let result = NamespaceId::new("");
        assert!(matches!(result, Err(VoError::Validation(msg)) if msg.contains("empty")));
    }

    #[test]
    fn namespace_id_new_too_long_rejects() {
        let long_id = "x".repeat(MAX_ID_LENGTH + 1);
        let result = NamespaceId::new(long_id);
        assert!(matches!(
            result,
            Err(VoError::Validation(msg)) if msg.contains("exceed")
        ));
    }

    #[test]
    fn namespace_id_new_with_control_char_rejects() {
        let result = NamespaceId::new("test\x00id");
        assert!(matches!(
            result,
            Err(VoError::Validation(msg)) if msg.contains("control")
        ));
    }

    #[test]
    fn namespace_id_display() {
        let id = NamespaceId::new("my-namespace").unwrap();
        assert_eq!(format!("{}", id), "my-namespace");
    }

    #[test]
    fn namespace_id_as_ref() {
        let id = NamespaceId::new("as-ref-ns").unwrap();
        assert_eq!(id.as_ref(), "as-ref-ns");
    }

    #[test]
    fn namespace_id_into_inner() {
        let id = NamespaceId::new("into-ns").unwrap();
        assert_eq!(id.into_inner(), "into-ns");
    }

    #[test]
    fn namespace_id_from_str() {
        let id: NamespaceId = "from-ns-str".into();
        assert_eq!(id.as_str(), "from-ns-str");
    }

    #[test]
    fn namespace_id_from_string() {
        let id: NamespaceId = "from-ns-string".to_string().into();
        assert_eq!(id.as_str(), "from-ns-string");
    }

    #[test]
    fn serde_roundtrip_namespace_id() {
        let id = NamespaceId::new("serde-ns-456").unwrap();
        let json = serde_json::to_string(&id).expect("should serialize");
        let deserialized: NamespaceId = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(id, deserialized);
    }

    #[test]
    fn serde_deserialization_validates() {
        // Empty string should fail even during deserialization
        let result: Result<NamespaceId, _> = serde_json::from_str("\"\"");
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn id_clone_preserves_data() {
        let id = NamespaceId::new("clone-test").unwrap();
        let cloned = id.clone();
        assert_eq!(id, cloned);
        assert_eq!(id.as_str(), cloned.as_str());
    }

    #[test]
    fn id_hash_and_eq() {
        use std::collections::HashSet;

        let id1 = NamespaceId::new("hash-test").unwrap();
        let id2 = NamespaceId::new("hash-test").unwrap();
        let id3 = NamespaceId::new("different").unwrap();

        let mut set = HashSet::new();
        set.insert(id1.clone());
        set.insert(id2.clone()); // Should be duplicate
        set.insert(id3);

        assert_eq!(set.len(), 2); // id1 and id2 are equal
    }
}
