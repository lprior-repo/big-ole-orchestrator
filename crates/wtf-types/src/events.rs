//! Domain events for the wtf-engine.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{
    AttemptNumber, BinaryHash, EventVersion, FireAtMs, IdempotencyKey, InstanceId, NodeName,
    SequenceNumber, TimerId, TimestampMs, WorkflowName,
};

pub const MAX_SUPPORTED_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum EventError {
    #[error("event envelope deserialization failed: {message}")]
    DeserializationFailed { message: String },

    #[error("unsupported event version {actual} (max supported: {max_supported})")]
    UnsupportedVersion { actual: u64, max_supported: u64 },

    #[error("event payload deserialization failed: {message}")]
    InvalidPayload { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: EventVersion,
    pub instance_id: InstanceId,
    pub sequence: SequenceNumber,
    pub timestamp_ms: TimestampMs,
    pub payload: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
}

impl EventEnvelope {
    /// Decode a JSON byte slice into a validated `EventEnvelope`.
    ///
    /// First-pass decode: deserializes the envelope struct but does NOT
    /// attempt to decode the payload into a typed `EventPayload`.
    ///
    /// # Errors
    /// Returns `EventError::DeserializationFailed` if the JSON is malformed
    /// or missing required fields (including when newtype validation fails,
    /// e.g. `version = 0` or `sequence = 0`).
    pub fn decode(json_bytes: &[u8]) -> Result<Self, EventError> {
        serde_json::from_slice(json_bytes).map_err(|e| EventError::DeserializationFailed {
            message: e.to_string(),
        })
    }

    /// Decode the `payload` field into a typed `EventPayload`.
    ///
    /// Second-pass decode, version-gated: if `self.version > MAX_SUPPORTED_VERSION`,
    /// returns `EventError::UnsupportedVersion` without inspecting the payload.
    ///
    /// # Errors
    /// - `EventError::UnsupportedVersion` when `self.version` exceeds the gate.
    /// - `EventError::InvalidPayload` when the payload JSON does not match any
    ///   known `EventPayload` variant.
    pub fn decode_payload(&self) -> Result<EventPayload, EventError> {
        let version_value = self.version.as_u64();
        if version_value > MAX_SUPPORTED_VERSION {
            return Err(EventError::UnsupportedVersion {
                actual: version_value,
                max_supported: MAX_SUPPORTED_VERSION,
            });
        }
        serde_json::from_value(self.payload.clone()).map_err(|e| EventError::InvalidPayload {
            message: e.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    WorkflowStarted {
        workflow_name: WorkflowName,
        binary_hash: BinaryHash,
    },

    WorkflowCompleted {
        result: Option<serde_json::Value>,
    },

    WorkflowFailed {
        error_message: String,
    },

    WorkflowCancelled {
        reason: String,
    },

    StepScheduled {
        node_name: NodeName,
        attempt: AttemptNumber,
    },

    StepStarted {
        node_name: NodeName,
        attempt: AttemptNumber,
        idempotency_key: IdempotencyKey,
        binary_hash: BinaryHash,
    },

    StepCompleted {
        node_name: NodeName,
        attempt: AttemptNumber,
        result: Option<serde_json::Value>,
    },

    StepFailed {
        node_name: NodeName,
        attempt: AttemptNumber,
        error_message: String,
        retryable: bool,
    },

    TimerSet {
        timer_id: TimerId,
        fire_at: FireAtMs,
    },

    TimerFired {
        timer_id: TimerId,
    },

    CancelRequested,

    InstanceResumed {
        previous_binary_hash: BinaryHash,
        resumed_binary_hash: BinaryHash,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_supported_version_equals_one_when_read() {
        assert_eq!(MAX_SUPPORTED_VERSION, 1);
    }

    #[test]
    fn event_error_deserialization_failed_displays_message_when_formatted() {
        let err = EventError::DeserializationFailed {
            message: "missing field `version`".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("event envelope deserialization failed"));
        assert!(msg.contains("missing field `version`"));
    }

    #[test]
    fn event_error_unsupported_version_displays_versions_when_formatted() {
        let err = EventError::UnsupportedVersion {
            actual: 5,
            max_supported: 1,
        };
        let msg = err.to_string();
        assert!(msg.contains("unsupported event version 5"));
        assert!(msg.contains("max supported: 1"));
    }

    #[test]
    fn event_error_invalid_payload_displays_message_when_formatted() {
        let err = EventError::InvalidPayload {
            message: "unknown variant `BogusVariant`".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("event payload deserialization failed"));
        assert!(msg.contains("unknown variant `BogusVariant`"));
    }

    #[test]
    fn event_payload_cancel_requested_is_unit_variant_when_inspected() {
        let payload = EventPayload::CancelRequested;
        assert!(matches!(payload, EventPayload::CancelRequested));
    }

    #[test]
    fn event_payload_cancel_requested_serializes_as_tag_only_when_serde_used() {
        let payload = EventPayload::CancelRequested;
        let json = serde_json::to_value(payload).expect("serialize");
        assert_eq!(json, serde_json::json!("CancelRequested"));
    }

    #[test]
    fn event_error_implements_std_error_trait_when_checked() {
        fn require_error<E: std::error::Error>(_: &E) {}
        let err = EventError::DeserializationFailed {
            message: "test".to_string(),
        };
        require_error(&err);
    }

    #[test]
    fn event_error_clone_equals_original_when_cloned() {
        let err = EventError::UnsupportedVersion {
            actual: 99,
            max_supported: 1,
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn event_envelope_debug_produces_output_when_formatted() {
        let env = EventEnvelope {
            version: EventVersion::new_unchecked(1),
            instance_id: InstanceId("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string()),
            sequence: SequenceNumber::new_unchecked(3),
            timestamp_ms: TimestampMs(1710000000000),
            payload: serde_json::json!({"WorkflowStarted": {"workflow_name": "test", "binary_hash": "abcdef01"}}),
            metadata: None,
        };
        let debug_str = format!("{env:?}");
        assert!(debug_str.contains("EventEnvelope"));
        assert!(debug_str.contains("version"));
    }

    #[test]
    fn event_payload_debug_produces_output_when_formatted() {
        let p = EventPayload::WorkflowFailed {
            error_message: "boom".to_string(),
        };
        let debug_str = format!("{p:?}");
        assert!(debug_str.contains("WorkflowFailed"));
    }

    #[test]
    fn event_envelope_clone_equals_original_when_cloned() {
        let env = EventEnvelope {
            version: EventVersion::new_unchecked(1),
            instance_id: InstanceId("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string()),
            sequence: SequenceNumber::new_unchecked(3),
            timestamp_ms: TimestampMs(1710000000000),
            payload: serde_json::json!({"StepStarted": {"node_name": "build"}}),
            metadata: None,
        };
        let cloned = env.clone();
        assert_eq!(env, cloned);
    }

    #[test]
    fn event_payload_clone_equals_original_when_cloned() {
        let p = EventPayload::WorkflowFailed {
            error_message: "boom".to_string(),
        };
        let cloned = p.clone();
        assert_eq!(p, cloned);
    }

    #[test]
    fn event_error_deserialization_failed_message_is_nonempty_when_constructed() {
        let err = EventError::DeserializationFailed {
            message: "some error".to_string(),
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn event_error_invalid_payload_message_is_nonempty_when_constructed() {
        let err = EventError::InvalidPayload {
            message: "bad payload".to_string(),
        };
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn event_payload_step_started_contains_idempotency_key_and_binary_hash_when_constructed() {
        let p = EventPayload::StepStarted {
            node_name: NodeName("build".to_string()),
            attempt: AttemptNumber::new_unchecked(1),
            idempotency_key: IdempotencyKey("key-123".to_string()),
            binary_hash: BinaryHash("abcdef0123456789".to_string()),
        };
        if let EventPayload::StepStarted {
            idempotency_key,
            binary_hash,
            ..
        } = &p
        {
            assert_eq!(idempotency_key.as_str(), "key-123");
            assert_eq!(binary_hash.as_str(), "abcdef0123456789");
        } else {
            panic!("Expected StepStarted variant");
        }
    }

    #[test]
    fn event_payload_instance_resumed_contains_both_hashes_when_constructed() {
        let p = EventPayload::InstanceResumed {
            previous_binary_hash: BinaryHash("aaaa0000".to_string()),
            resumed_binary_hash: BinaryHash("bbbb1111".to_string()),
        };
        if let EventPayload::InstanceResumed {
            previous_binary_hash,
            resumed_binary_hash,
        } = &p
        {
            assert_eq!(previous_binary_hash.as_str(), "aaaa0000");
            assert_eq!(resumed_binary_hash.as_str(), "bbbb1111");
        } else {
            panic!("Expected InstanceResumed variant");
        }
    }
}
