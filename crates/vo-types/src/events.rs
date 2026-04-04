//! Domain events for the vo-engine.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::payload_parser::{optional_u64, require_string, require_string_field, require_u64};

pub const MAX_SUPPORTED_VERSION: u8 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("Input bytes are not valid UTF-8")]
    InvalidInput,

    #[error("Envelope JSON is malformed")]
    InvalidEnvelopeFormat,

    #[error("Missing envelope field: {0}")]
    MissingEnvelopeField(String),

    #[error("Invalid envelope field: {0}")]
    InvalidEnvelopeField(String),

    #[error("Unsupported envelope version: {0}")]
    UnsupportedEnvelopeVersion(u8),

    #[error("Missing schema version")]
    MissingSchemaVersion,

    #[error("Invalid schema version format")]
    InvalidSchemaVersionFormat,

    #[error("Unsupported schema version: {0}")]
    UnsupportedSchemaVersion(u16),

    #[error("Payload JSON is malformed")]
    InvalidPayloadFormat,

    #[error("Missing payload field: {0}")]
    MissingPayloadField(String),

    #[error("Invalid payload field: {0}")]
    InvalidPayloadField(String),

    #[error("Unsupported payload version: {0}")]
    UnsupportedPayloadVersion(u8),

    #[error("Unknown payload type: {0}")]
    UnknownPayloadType(String),

    #[error("Envelope decode failed: {0}")]
    EnvelopeDecodeFailed(Box<Error>),

    #[error("Payload decode skipped due to unsupported envelope version")]
    PayloadDecodeSkipped,

    #[error("Payload decode failed: {0}")]
    PayloadDecodeFailed(Box<Error>),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub schema_version: u8,
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

impl EventEnvelope {
    /// Decode an `EventEnvelope` from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if the bytes are not valid UTF-8, or
    /// various envelope errors if the JSON is malformed or missing required fields.
    pub fn from_bytes(input: &[u8]) -> Result<Self, Error> {
        let json_str = std::str::from_utf8(input).map_err(|_| Error::InvalidInput)?;
        Self::from_str(json_str)
    }

    /// Decode an `EventEnvelope` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns envelope-level errors if the JSON is malformed, missing
    /// required fields, or contains an unsupported version.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &str) -> Result<Self, Error> {
        let value: serde_json::Value =
            serde_json::from_str(input).map_err(|_| Error::InvalidEnvelopeFormat)?;

        let obj = value.as_object().ok_or(Error::InvalidEnvelopeFormat)?;

        let version_u64 = envelope_u64(obj, "version")?;
        let version = u8::try_from(version_u64).map_err(|_| {
            Error::InvalidEnvelopeField("version exceeds maximum supported value".to_string())
        })?;
        let instance_id = envelope_string(obj, "instance_id")?;
        let sequence = envelope_u64(obj, "sequence")?;
        let timestamp_ms = envelope_u64(obj, "timestamp_ms")?;

        let payload = obj
            .get("payload")
            .ok_or_else(|| Error::MissingEnvelopeField("payload".to_string()))?;

        let metadata = obj
            .get("metadata")
            .ok_or_else(|| Error::MissingEnvelopeField("metadata".to_string()))?
            .as_object()
            .ok_or_else(|| Error::InvalidEnvelopeField("metadata must be an object".to_string()))?;

        if instance_id.is_empty() {
            return Err(Error::InvalidEnvelopeField(
                "instance_id cannot be empty".to_string(),
            ));
        }

        if sequence == 0 {
            return Err(Error::InvalidEnvelopeField(
                "sequence must be >= 1".to_string(),
            ));
        }

        if version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedEnvelopeVersion(version));
        }

        Ok(EventEnvelope {
            schema_version: version,
            instance_id,
            sequence,
            timestamp_ms,
            payload: payload.clone(),
            metadata: serde_json::Value::Object(metadata.clone()),
        })
    }

    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.schema_version <= MAX_SUPPORTED_VERSION
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    WorkflowStarted {
        workflow_id: String,
        dag_topology: serde_json::Value,
        binary_hash: String,
    },
    WorkflowCompleted {
        workflow_id: String,
        completion_time_ms: u64,
    },
    WorkflowFailed {
        workflow_id: String,
        failure_reason: String,
    },
    WorkflowCancelled {
        workflow_id: String,
        cancelled_by: String,
    },
    StepScheduled {
        workflow_id: String,
        step_id: String,
        attempt: u32,
        execution_id: String,
    },
    StepStarted {
        workflow_id: String,
        step_id: String,
        started_at_ms: u64,
    },
    StepCompleted {
        workflow_id: String,
        step_id: String,
        completed_at_ms: u64,
        output: serde_json::Value,
    },
    StepFailed {
        workflow_id: String,
        step_id: String,
        failure_reason: String,
        attempt: u32,
    },
    TimerSet {
        workflow_id: String,
        timer_id: String,
        fire_at_ms: u64,
    },
    TimerFired {
        workflow_id: String,
        timer_id: String,
        fired_at_ms: u64,
    },
    CancelRequested {
        workflow_id: String,
        requested_by: String,
    },
    InstanceResumed {
        workflow_id: String,
        resumed_at_ms: u64,
    },
    /// Emitted when a workflow continues-as-new to a new epoch (ADR-038).
    ContinuedAsNew {
        workflow_id: String,
        lineage_id: String,
        old_epoch: u64,
        new_epoch: u64,
    },
}

impl EventPayload {
    /// Decode an `EventPayload` from a JSON value.
    ///
    /// # Errors
    ///
    /// Returns payload-level errors if the JSON is not a valid object,
    /// missing required fields, or has an unsupported version/type.
    pub fn try_from_json(payload_json: &serde_json::Value) -> Result<Self, Error> {
        let obj = payload_json
            .as_object()
            .ok_or(Error::InvalidPayloadFormat)?;

        let payload_type = require_string(obj, "type")?;
        let payload_version_u64 = optional_u64(obj, "version", 0);
        let payload_version = u8::try_from(payload_version_u64).map_err(|_| {
            Error::InvalidPayloadField("version exceeds maximum supported value".to_string())
        })?;
        if payload_version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedPayloadVersion(payload_version));
        }

        match payload_type.as_str() {
            "WorkflowStarted" => Ok(EventPayload::WorkflowStarted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                dag_topology: obj
                    .get("dag_topology")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                binary_hash: require_string(obj, "binary_hash")?,
            }),
            "WorkflowCompleted" => Ok(EventPayload::WorkflowCompleted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                completion_time_ms: require_u64(obj, "completion_time_ms")?,
            }),
            "WorkflowFailed" => Ok(EventPayload::WorkflowFailed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                failure_reason: require_string(obj, "failure_reason")?,
            }),
            "WorkflowCancelled" => Ok(EventPayload::WorkflowCancelled {
                workflow_id: require_string_field(obj, "workflow_id")?,
                cancelled_by: require_string(obj, "cancelled_by")?,
            }),
            "StepScheduled" => Ok(EventPayload::StepScheduled {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                #[allow(clippy::cast_possible_truncation)]
                attempt: require_u64(obj, "attempt")? as u32,
                execution_id: require_string(obj, "execution_id")?,
            }),
            "StepStarted" => Ok(EventPayload::StepStarted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                started_at_ms: require_u64(obj, "started_at_ms")?,
            }),
            "StepCompleted" => Ok(EventPayload::StepCompleted {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                completed_at_ms: require_u64(obj, "completed_at_ms")?,
                output: obj
                    .get("output")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            }),
            "StepFailed" => Ok(EventPayload::StepFailed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                failure_reason: require_string(obj, "failure_reason")?,
                #[allow(clippy::cast_possible_truncation)]
                attempt: require_u64(obj, "attempt")? as u32,
            }),
            "TimerSet" => Ok(EventPayload::TimerSet {
                workflow_id: require_string_field(obj, "workflow_id")?,
                timer_id: require_string(obj, "timer_id")?,
                fire_at_ms: require_u64(obj, "fire_at_ms")?,
            }),
            "TimerFired" => Ok(EventPayload::TimerFired {
                workflow_id: require_string_field(obj, "workflow_id")?,
                timer_id: require_string(obj, "timer_id")?,
                fired_at_ms: require_u64(obj, "fired_at_ms")?,
            }),
            "CancelRequested" => Ok(EventPayload::CancelRequested {
                workflow_id: require_string_field(obj, "workflow_id")?,
                requested_by: require_string(obj, "requested_by")?,
            }),
            "InstanceResumed" => Ok(EventPayload::InstanceResumed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                resumed_at_ms: require_u64(obj, "resumed_at_ms")?,
            }),
            "ContinuedAsNew" => Ok(EventPayload::ContinuedAsNew {
                workflow_id: require_string_field(obj, "workflow_id")?,
                lineage_id: require_string(obj, "lineage_id")?,
                old_epoch: require_u64(obj, "old_epoch")?,
                new_epoch: require_u64(obj, "new_epoch")?,
            }),
            other => Err(Error::UnknownPayloadType(other.to_string())),
        }
    }

    #[must_use]
    pub fn is_version_supported(version: u8) -> bool {
        version <= MAX_SUPPORTED_VERSION
    }
}

/// Decode a full event (envelope + payload) from raw bytes.
///
/// # Errors
///
/// Returns `Error::PayloadDecodeSkipped` if the envelope version is unsupported,
/// `Error::EnvelopeDecodeFailed` on envelope parse failures, or
/// `Error::PayloadDecodeFailed` on payload parse failures.
pub fn decode_event(input: &[u8]) -> Result<(EventEnvelope, EventPayload), Error> {
    let envelope = match EventEnvelope::from_bytes(input) {
        Err(Error::UnsupportedEnvelopeVersion(_)) => {
            return Err(Error::PayloadDecodeSkipped);
        }
        Err(e) => {
            return Err(Error::EnvelopeDecodeFailed(Box::new(e)));
        }
        Ok(envelope) => envelope,
    };
    if !envelope.is_supported() {
        return Err(Error::PayloadDecodeSkipped);
    }
    let payload = EventPayload::try_from_json(&envelope.payload)
        .map_err(|e| Error::PayloadDecodeFailed(Box::new(e)))?;
    Ok((envelope, payload))
}

// ---------------------------------------------------------------------------
// Envelope field extraction helpers (EnvelopeError variant)
// ---------------------------------------------------------------------------

fn envelope_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, Error> {
    obj.get(field)
        .ok_or_else(|| Error::MissingEnvelopeField(field.to_string()))?
        .as_str()
        .ok_or_else(|| Error::InvalidEnvelopeField(format!("{field} must be a string")))
        .map(std::string::ToString::to_string)
}

fn envelope_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u64, Error> {
    obj.get(field)
        .ok_or_else(|| Error::MissingEnvelopeField(field.to_string()))?
        .as_u64()
        .ok_or_else(|| Error::InvalidEnvelopeField(format!("{field} must be an integer")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn envelope_from_bytes_returns_ok_when_input_is_valid_json() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        let envelope = result.unwrap();
        assert_eq!(envelope.schema_version, 1);
        assert_eq!(envelope.instance_id, "wf-123");
        assert_eq!(envelope.sequence, 1);
        assert_eq!(envelope.timestamp_ms, 1000);
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_format_when_json_is_malformed() {
        let json = r#"{"version": 1, "instance_id": "wf-123""#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::InvalidEnvelopeFormat));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_input_when_bytes_are_not_valid_utf8() {
        let bytes = vec![0xFF, 0xFE, 0xFD, 0x00];
        let result = EventEnvelope::from_bytes(&bytes);
        assert_eq!(result, Err(Error::InvalidInput));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_version_is_absent() {
        let json = r#"{"instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("version".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_instance_id_is_absent() {
        let json = r#"{"version": 1, "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("instance_id".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_sequence_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("sequence".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_timestamp_ms_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("timestamp_ms".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_payload_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("payload".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_metadata_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(
            result,
            Err(Error::MissingEnvelopeField("metadata".to_string()))
        );
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_version_is_not_integer() {
        let json = r#"{"version": "1", "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_instance_id_is_empty() {
        let json = r#"{"version": 1, "instance_id": "", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_sequence_is_zero() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 0, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_bytes_returns_unsupported_envelope_version_when_version_exceeds_max() {
        let json = r#"{"version": 2, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::UnsupportedEnvelopeVersion(2)));
    }

    #[test]
    fn envelope_from_bytes_returns_unsupported_envelope_version_when_version_is_u8_max() {
        let json = r#"{"version": 255, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::UnsupportedEnvelopeVersion(255)));
    }

    #[test]
    fn envelope_from_bytes_returns_invalid_envelope_field_when_metadata_is_not_object() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": []}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
    }

    #[test]
    fn envelope_from_str_returns_ok_when_input_is_valid_json() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
        let result = EventEnvelope::from_str(json);
        result.unwrap();
    }

    #[test]
    fn envelope_from_str_returns_invalid_envelope_format_when_json_is_malformed() {
        let json = r#"{"version": 1, "instance_id": "wf-123""#;
        let result = EventEnvelope::from_str(json);
        assert_eq!(result, Err(Error::InvalidEnvelopeFormat));
    }

    #[test]
    fn envelope_is_supported_returns_true_when_version_is_zero() {
        let envelope = EventEnvelope {
            schema_version: 0,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(envelope.is_supported());
    }

    #[test]
    fn envelope_is_supported_returns_true_when_version_is_one() {
        let envelope = EventEnvelope {
            schema_version: 1,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(envelope.is_supported());
    }

    #[test]
    fn envelope_is_supported_returns_false_when_version_is_two() {
        let envelope = EventEnvelope {
            schema_version: 2,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(!envelope.is_supported());
    }

    #[test]
    fn payload_try_from_json_returns_workflow_started_when_type_is_workflow_started() {
        let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "dag_topology": {}, "binary_hash": "abc123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowStarted {
                workflow_id: "wf-123".to_string(),
                dag_topology: serde_json::json!({}),
                binary_hash: "abc123".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_workflow_completed_when_type_is_workflow_completed() {
        let json = serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "wf-123", "completion_time_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowCompleted {
                workflow_id: "wf-123".to_string(),
                completion_time_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_workflow_failed_when_type_is_workflow_failed() {
        let json = serde_json::json!({"type": "WorkflowFailed", "workflow_id": "wf-123", "failure_reason": "timeout", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowFailed {
                workflow_id: "wf-123".to_string(),
                failure_reason: "timeout".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_workflow_cancelled_when_type_is_workflow_cancelled() {
        let json = serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "wf-123", "cancelled_by": "user", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowCancelled {
                workflow_id: "wf-123".to_string(),
                cancelled_by: "user".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_scheduled_when_type_is_step_scheduled() {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "wf-123", "step_id": "step-1", "attempt": 1, "execution_id": "inst::step::1", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepScheduled {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                attempt: 1,
                execution_id: "inst::step::1".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_started_when_type_is_step_started() {
        let json = serde_json::json!({"type": "StepStarted", "workflow_id": "wf-123", "step_id": "step-1", "started_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepStarted {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                started_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_completed_when_type_is_step_completed() {
        let json = serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-123", "step_id": "step-1", "completed_at_ms": 1000, "output": null, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepCompleted {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                completed_at_ms: 1000,
                output: serde_json::Value::Null
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_step_failed_when_type_is_step_failed() {
        let json = serde_json::json!({"type": "StepFailed", "workflow_id": "wf-123", "step_id": "step-1", "failure_reason": "error", "attempt": 1, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepFailed {
                workflow_id: "wf-123".to_string(),
                step_id: "step-1".to_string(),
                failure_reason: "error".to_string(),
                attempt: 1
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_timer_set_when_type_is_timer_set() {
        let json = serde_json::json!({"type": "TimerSet", "workflow_id": "wf-123", "timer_id": "timer-1", "fire_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::TimerSet {
                workflow_id: "wf-123".to_string(),
                timer_id: "timer-1".to_string(),
                fire_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_timer_fired_when_type_is_timer_fired() {
        let json = serde_json::json!({"type": "TimerFired", "workflow_id": "wf-123", "timer_id": "timer-1", "fired_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::TimerFired {
                workflow_id: "wf-123".to_string(),
                timer_id: "timer-1".to_string(),
                fired_at_ms: 1000
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_cancel_requested_when_type_is_cancel_requested() {
        let json = serde_json::json!({"type": "CancelRequested", "workflow_id": "wf-123", "requested_by": "user", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::CancelRequested {
                workflow_id: "wf-123".to_string(),
                requested_by: "user".to_string()
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_instance_resumed_when_type_is_instance_resumed() {
        let json = serde_json::json!({"type": "InstanceResumed", "workflow_id": "wf-123", "resumed_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::InstanceResumed {
                workflow_id: "wf-123".to_string(),
                resumed_at_ms: 1000
            })
        );
    }

    // -------------------------------------------------------------------------
    // ADR-038: ContinuedAsNew tests
    // -------------------------------------------------------------------------

    #[test]
    fn payload_try_from_json_returns_continued_as_new_when_type_matches() {
        let json = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-abc-123",
            "old_epoch": 0,
            "new_epoch": 1,
            "version": 1
        });
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::ContinuedAsNew {
                workflow_id: "wf-1".to_string(),
                lineage_id: "lin-abc-123".to_string(),
                old_epoch: 0,
                new_epoch: 1,
            })
        );
    }

    #[test]
    fn decode_event_returns_continued_as_new_for_valid_full_event() {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "inst-1",
            "sequence": 42,
            "timestamp_ms": 9999,
            "payload": {
                "type": "ContinuedAsNew",
                "workflow_id": "wf-1",
                "lineage_id": "lin-1",
                "old_epoch": 2,
                "new_epoch": 3,
                "version": 1
            },
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).expect("serialize");
        let result = decode_event(&bytes);
        let (_envelope, payload) = result.expect("decode should succeed");
        match payload {
            EventPayload::ContinuedAsNew {
                workflow_id,
                lineage_id,
                old_epoch,
                new_epoch,
            } => {
                assert_eq!(workflow_id, "wf-1");
                assert_eq!(lineage_id, "lin-1");
                assert_eq!(old_epoch, 2);
                assert_eq!(new_epoch, 3);
            }
            other => panic!("Expected ContinuedAsNew, got {other:?}"),
        }
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_lineage_id_absent() {
        let json = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "old_epoch": 0,
            "new_epoch": 1,
            "version": 1
        });
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("lineage_id".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_old_epoch_absent() {
        let json = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-1",
            "new_epoch": 1,
            "version": 1
        });
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("old_epoch".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_new_epoch_absent() {
        let json = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-1",
            "old_epoch": 0,
            "version": 1
        });
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("new_epoch".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_old_epoch_not_integer() {
        let json = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-1",
            "old_epoch": "bad",
            "new_epoch": 1,
            "version": 1
        });
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::InvalidPayloadField(
                "old_epoch must be an integer".to_string()
            ))
        );
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_new_epoch_not_integer() {
        let json = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-1",
            "old_epoch": 0,
            "new_epoch": "bad",
            "version": 1
        });
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::InvalidPayloadField(
                "new_epoch must be an integer".to_string()
            ))
        );
    }

    #[test]
    fn payload_try_from_json_returns_unknown_payload_type_when_type_is_unrecognized() {
        let json =
            serde_json::json!({"type": "UnknownType", "workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::UnknownPayloadType("UnknownType".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_unsupported_payload_version_when_version_exceeds_max() {
        let json =
            serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 2});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(Error::UnsupportedPayloadVersion(2)));
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_type_is_absent() {
        let json = serde_json::json!({"workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(Error::MissingPayloadField("type".to_string())));
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_variant_field_is_absent() {
        let json = serde_json::json!({"type": "WorkflowStarted", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert!(matches!(result, Err(Error::InvalidPayloadField(_))));
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_format_when_json_is_malformed() {
        let json = serde_json::Value::String("not an object".to_string());
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(Error::InvalidPayloadFormat));
    }

    #[test]
    fn payload_is_version_supported_returns_true_when_version_is_zero() {
        assert!(EventPayload::is_version_supported(0));
    }

    #[test]
    fn payload_is_version_supported_returns_true_when_version_is_one() {
        assert!(EventPayload::is_version_supported(1));
    }

    #[test]
    fn payload_is_version_supported_returns_false_when_version_is_two() {
        assert!(!EventPayload::is_version_supported(2));
    }

    #[test]
    fn payload_is_version_supported_returns_false_when_version_is_u8_max() {
        assert!(!EventPayload::is_version_supported(u8::MAX));
    }

    #[test]
    fn decode_event_returns_ok_when_envelope_and_payload_are_valid() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "binary_hash": "abc123", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        let (envelope, payload) = result.unwrap();
        assert_eq!(envelope.schema_version, 1);
        assert_eq!(envelope.instance_id, "wf-123");
        assert_eq!(envelope.sequence, 1);
        assert_eq!(envelope.timestamp_ms, 1000);
        assert!(matches!(payload, EventPayload::WorkflowStarted { .. }));
    }

    #[test]
    fn decode_event_returns_envelope_decode_failed_when_envelope_is_malformed() {
        let json = r#"{"version": 1, "instance_id": "wf-123""#;
        let result = decode_event(json.as_bytes());
        assert!(matches!(result, Err(Error::EnvelopeDecodeFailed(_))));
    }

    #[test]
    fn decode_event_returns_payload_decode_failed_when_payload_is_invalid() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "UnknownType", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        assert!(matches!(result, Err(Error::PayloadDecodeFailed(_))));
    }

    #[test]
    fn decode_event_returns_payload_decode_skipped_when_envelope_version_exceeds_max() {
        let json = r#"{"version": 2, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        assert_eq!(result, Err(Error::PayloadDecodeSkipped));
    }

    #[rstest]
    #[case(0, 1, "wf-1", 0)]
    #[case(1, 100, "wf-abc", 1000)]
    fn envelope_roundtrip_preserves_data(
        #[case] version: u8,
        #[case] seq: u64,
        #[case] instance_id: &str,
        #[case] ts: u64,
    ) {
        let json = serde_json::json!({
            "version": version,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": ts,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);

        let expected = EventEnvelope {
            schema_version: version,
            instance_id: instance_id.to_string(),
            sequence: seq,
            timestamp_ms: ts,
            payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1}),
            metadata: serde_json::json!({}),
        };
        assert_eq!(result, Ok(expected));
    }

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(2)]
    #[case(3)]
    #[case(4)]
    #[case(5)]
    fn proptest_version_support_is_consistent_across_envelope_and_payload(#[case] version: u8) {
        let envelope = EventEnvelope {
            schema_version: version,
            instance_id: "wf-123".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        let envelope_supported = envelope.is_supported();
        let payload_supported = EventPayload::is_version_supported(version);
        assert_eq!(
            envelope_supported, payload_supported,
            "Inconsistent for version {}",
            version
        );
    }

    #[rstest]
    #[case(1, "wf-1")]
    #[case(100, "wf-100")]
    #[case(999_999_999, "wf-max")]
    fn envelope_parsing_accepts_positive_sequence(#[case] seq: u64, #[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Ok(envelope) = result else {
            panic!("Expected Ok, got {:?}", result);
        };
        assert_eq!(envelope.sequence, seq);
    }

    #[rstest]
    #[case(0, "wf-zero")]
    fn envelope_parsing_rejects_zero_sequence(#[case] seq: u64, #[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": seq,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Err(err) = result else {
            panic!("Expected Err, got {:?}", result);
        };
        assert!(matches!(err, Error::InvalidEnvelopeField(_)));
    }

    #[rstest]
    #[case("a")]
    #[case("wf-123")]
    #[case("instance_with_underscores")]
    fn envelope_parsing_accepts_nonempty_instance_id(#[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Ok(envelope) = result else {
            panic!("Expected Ok, got {:?}", result);
        };
        assert_eq!(envelope.instance_id, instance_id);
    }

    #[rstest]
    #[case("")]
    fn envelope_parsing_rejects_empty_instance_id(#[case] instance_id: &str) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": instance_id,
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Err(err) = result else {
            panic!("Expected Err, got {:?}", result);
        };
        assert!(matches!(err, Error::InvalidEnvelopeField(_)));
    }

    #[rstest]
    #[case(serde_json::json!({}))]
    #[case(serde_json::json!({"key": "value"}))]
    #[case(serde_json::json!({"key1": "value1", "key2": 123}))]
    fn envelope_parsing_accepts_object_metadata(#[case] metadata: serde_json::Value) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "wf-123",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": metadata
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Ok(envelope) = result else {
            panic!("Expected Ok, got {:?}", result);
        };
        assert_eq!(envelope.metadata, metadata);
    }

    #[rstest]
    #[case(serde_json::json!([]))]
    #[case(serde_json::json!("string"))]
    #[case(serde_json::Value::Null)]
    #[case(serde_json::json!(123))]
    fn envelope_parsing_rejects_non_object_metadata(#[case] metadata: serde_json::Value) {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "wf-123",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
            "metadata": metadata
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = EventEnvelope::from_bytes(&bytes);
        let Err(err) = result else {
            panic!("Expected Err, got {:?}", result);
        };
        assert!(matches!(err, Error::InvalidEnvelopeField(_)));
    }

    #[rstest]
    #[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": "bad", "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("sequence must be an integer".to_string()))]
    #[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": "bad", "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("timestamp_ms must be an integer".to_string()))]
    fn envelope_from_str_invalid_types(#[case] json: &str, #[case] expected: Error) {
        let result = EventEnvelope::from_str(json);
        assert_eq!(result, Err(expected));
    }

    // -------------------------------------------------------------------------
    // ADR-027: Error-path tests for new required fields (binary_hash, attempt,
    // execution_id, dag_topology, output)
    // -------------------------------------------------------------------------

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_binary_hash_is_absent() {
        let json =
            serde_json::json!({"type": "WorkflowStarted", "workflow_id": "w1", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("binary_hash".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_attempt_is_absent_for_step_scheduled(
    ) {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "execution_id": "e1", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("attempt".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_attempt_is_not_integer_for_step_scheduled(
    ) {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": "bad", "execution_id": "e1", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::InvalidPayloadField(
                "attempt must be an integer".to_string()
            ))
        );
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_execution_id_is_absent() {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 1, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("execution_id".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_attempt_is_absent_for_step_failed()
    {
        let json = serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::MissingPayloadField("attempt".to_string()))
        );
    }

    #[test]
    fn payload_try_from_json_defaults_dag_topology_to_null_when_absent() {
        let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "w1", "binary_hash": "abc123", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::WorkflowStarted {
                workflow_id: "w1".into(),
                dag_topology: serde_json::Value::Null,
                binary_hash: "abc123".into(),
            })
        );
    }

    #[test]
    fn payload_try_from_json_defaults_output_to_null_when_absent() {
        let json = serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "completed_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepCompleted {
                workflow_id: "w1".into(),
                step_id: "s1".into(),
                completed_at_ms: 1000,
                output: serde_json::Value::Null,
            })
        );
    }

    #[test]
    fn decode_event_returns_correct_binary_hash_and_dag_topology_in_full_pipeline() {
        let json = serde_json::json!({
            "version": 1,
            "instance_id": "inst-1",
            "sequence": 1,
            "timestamp_ms": 1000,
            "payload": {
                "type": "WorkflowStarted",
                "workflow_id": "wf-123",
                "dag_topology": {"nodes": []},
                "binary_hash": "sha256abc",
                "version": 1
            },
            "metadata": {}
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let result = decode_event(&bytes);
        let (_envelope, payload) = result.unwrap();
        match payload {
            EventPayload::WorkflowStarted {
                workflow_id,
                dag_topology,
                binary_hash,
            } => {
                assert_eq!(workflow_id, "wf-123");
                assert_eq!(dag_topology, serde_json::json!({"nodes": []}));
                assert_eq!(binary_hash, "sha256abc");
            }
            other => panic!("Expected WorkflowStarted, got {other:?}"),
        }
    }

    #[test]
    fn payload_try_from_json_handles_attempt_at_u32_max() {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 4294967295_u64, "execution_id": "e1", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Ok(EventPayload::StepScheduled {
                workflow_id: "w1".into(),
                step_id: "s1".into(),
                attempt: u32::MAX,
                execution_id: "e1".into(),
            })
        );
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_attempt_is_not_integer_for_step_failed(
    ) {
        let json = serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "attempt": "bad", "version": 1});
        let result = EventPayload::try_from_json(&json);
        assert_eq!(
            result,
            Err(Error::InvalidPayloadField(
                "attempt must be an integer".to_string()
            ))
        );
    }

    #[rstest]
    #[case(serde_json::json!({"type": "WorkflowStarted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowStarted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("completion_time_ms".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "w1", "completion_time_ms": "bad", "version": 1}), Error::InvalidPayloadField("completion_time_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("failure_reason".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowFailed", "workflow_id": "w1", "failure_reason": 123, "version": 1}), Error::InvalidPayloadField("failure_reason must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("cancelled_by".to_string()))]
    #[case(serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "w1", "cancelled_by": 123, "version": 1}), Error::InvalidPayloadField("cancelled_by must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("started_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "StepStarted", "workflow_id": "w1", "step_id": "s1", "started_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("started_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("completed_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "StepCompleted", "workflow_id": "w1", "step_id": "s1", "completed_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("completed_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("step_id".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": 123, "version": 1}), Error::InvalidPayloadField("step_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "version": 1}), Error::MissingPayloadField("failure_reason".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": 123, "version": 1}), Error::InvalidPayloadField("failure_reason must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("timer_id".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": 123, "version": 1}), Error::InvalidPayloadField("timer_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": "t1", "version": 1}), Error::MissingPayloadField("fire_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "TimerSet", "workflow_id": "w1", "timer_id": "t1", "fire_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("fire_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("timer_id".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": 123, "version": 1}), Error::InvalidPayloadField("timer_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": "t1", "version": 1}), Error::MissingPayloadField("fired_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "TimerFired", "workflow_id": "w1", "timer_id": "t1", "fired_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("fired_at_ms must be an integer".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("requested_by".to_string()))]
    #[case(serde_json::json!({"type": "CancelRequested", "workflow_id": "w1", "requested_by": 123, "version": 1}), Error::InvalidPayloadField("requested_by must be a string".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "version": 1}), Error::InvalidPayloadField("workflow_id is required".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": 123, "version": 1}), Error::InvalidPayloadField("workflow_id must be a string".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("resumed_at_ms".to_string()))]
    #[case(serde_json::json!({"type": "InstanceResumed", "workflow_id": "w1", "resumed_at_ms": "bad", "version": 1}), Error::InvalidPayloadField("resumed_at_ms must be an integer".to_string()))]
    // ADR-027: new required-field missing cases for binary_hash, attempt, execution_id
    #[case(serde_json::json!({"type": "WorkflowStarted", "workflow_id": "w1", "version": 1}), Error::MissingPayloadField("binary_hash".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "execution_id": "e1", "version": 1}), Error::MissingPayloadField("attempt".to_string()))]
    #[case(serde_json::json!({"type": "StepScheduled", "workflow_id": "w1", "step_id": "s1", "attempt": 1, "version": 1}), Error::MissingPayloadField("execution_id".to_string()))]
    #[case(serde_json::json!({"type": "StepFailed", "workflow_id": "w1", "step_id": "s1", "failure_reason": "err", "version": 1}), Error::MissingPayloadField("attempt".to_string()))]

    fn payload_invalid_fields(#[case] json: serde_json::Value, #[case] expected: Error) {
        let result = EventPayload::try_from_json(&json);
        assert_eq!(result, Err(expected));
    }

    #[test]
    fn error_serialization_error_displays_correctly() {
        let err = Error::SerializationError("test_error".to_string());
        assert_eq!(err.to_string(), "Serialization error: test_error");
    }

    #[test]
    fn error_envelope_decode_failed_displays_correctly() {
        let inner = Error::InvalidInput;
        let err = Error::EnvelopeDecodeFailed(Box::new(inner));
        assert_eq!(
            err.to_string(),
            "Envelope decode failed: Input bytes are not valid UTF-8"
        );
    }

    #[test]
    fn error_payload_decode_failed_displays_correctly() {
        let inner = Error::InvalidPayloadFormat;
        let err = Error::PayloadDecodeFailed(Box::new(inner));
        assert_eq!(
            err.to_string(),
            "Payload decode failed: Payload JSON is malformed"
        );
    }

    #[test]
    fn error_payload_decode_skipped_displays_correctly() {
        let err = Error::PayloadDecodeSkipped;
        assert_eq!(
            err.to_string(),
            "Payload decode skipped due to unsupported envelope version"
        );
    }

    #[test]
    fn error_invalid_schema_version_format_displays_correctly() {
        let err = Error::InvalidSchemaVersionFormat;
        assert_eq!(err.to_string(), "Invalid schema version format");
    }

    #[test]
    fn error_unsupported_schema_version_displays_correctly() {
        let err = Error::UnsupportedSchemaVersion(99);
        assert_eq!(err.to_string(), "Unsupported schema version: 99");
    }

    #[test]
    fn error_missing_schema_version_displays_correctly() {
        let err = Error::MissingSchemaVersion;
        assert_eq!(err.to_string(), "Missing schema version");
    }
}
