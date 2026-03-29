//! Domain events for the vo-engine.

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub version: u8,
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

impl EventEnvelope {
    pub fn from_bytes(input: &[u8]) -> Result<Self, Error> {
        let json_str = std::str::from_utf8(input).map_err(|_| Error::InvalidInput)?;
        Self::from_str(json_str)
    }

    pub fn from_str(input: &str) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_str(input)
            .map_err(|_| Error::InvalidEnvelopeFormat)?;

        let version = value.get("version")
            .ok_or_else(|| Error::MissingEnvelopeField("version".to_string()))?
            .as_u64()
            .ok_or_else(|| Error::InvalidEnvelopeField("version must be an integer".to_string()))? as u8;

        let instance_id = value.get("instance_id")
            .ok_or_else(|| Error::MissingEnvelopeField("instance_id".to_string()))?
            .as_str()
            .ok_or_else(|| Error::InvalidEnvelopeField("instance_id must be a string".to_string()))?
            .to_string();

        let sequence = value.get("sequence")
            .ok_or_else(|| Error::MissingEnvelopeField("sequence".to_string()))?
            .as_u64()
            .ok_or_else(|| Error::InvalidEnvelopeField("sequence must be an integer".to_string()))?;

        let timestamp_ms = value.get("timestamp_ms")
            .ok_or_else(|| Error::MissingEnvelopeField("timestamp_ms".to_string()))?
            .as_u64()
            .ok_or_else(|| Error::InvalidEnvelopeField("timestamp_ms must be an integer".to_string()))?;

        let payload = value.get("payload")
            .ok_or_else(|| Error::MissingEnvelopeField("payload".to_string()))?;

        let metadata = value.get("metadata")
            .ok_or_else(|| Error::MissingEnvelopeField("metadata".to_string()))?
            .as_object()
            .ok_or_else(|| Error::InvalidEnvelopeField("metadata must be an object".to_string()))?;

        if instance_id.is_empty() {
            return Err(Error::InvalidEnvelopeField("instance_id cannot be empty".to_string()));
        }

        if sequence == 0 {
            return Err(Error::InvalidEnvelopeField("sequence must be >= 1".to_string()));
        }

        if version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedEnvelopeVersion(version));
        }

        Ok(EventEnvelope {
            version,
            instance_id,
            sequence,
            timestamp_ms,
            payload: payload.clone(),
            metadata: serde_json::Value::Object(metadata.clone()),
        })
    }

    pub fn is_supported(&self) -> bool {
        self.version <= MAX_SUPPORTED_VERSION
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    WorkflowStarted { workflow_id: String },
    WorkflowCompleted { workflow_id: String, completion_time_ms: u64 },
    WorkflowFailed { workflow_id: String, failure_reason: String },
    WorkflowCancelled { workflow_id: String, cancelled_by: String },
    StepScheduled { workflow_id: String, step_id: String },
    StepStarted { workflow_id: String, step_id: String, started_at_ms: u64 },
    StepCompleted { workflow_id: String, step_id: String, completed_at_ms: u64 },
    StepFailed { workflow_id: String, step_id: String, failure_reason: String },
    TimerSet { workflow_id: String, timer_id: String, fire_at_ms: u64 },
    TimerFired { workflow_id: String, timer_id: String, fired_at_ms: u64 },
    CancelRequested { workflow_id: String, requested_by: String },
    InstanceResumed { workflow_id: String, resumed_at_ms: u64 },
}

impl EventPayload {
    pub fn try_from_json(payload_json: serde_json::Value) -> Result<Self, Error> {
        let value = payload_json.as_object().ok_or(Error::InvalidPayloadFormat)?;

        let payload_type = value.get("type")
            .ok_or_else(|| Error::MissingPayloadField("type".to_string()))?
            .as_str()
            .ok_or_else(|| Error::InvalidPayloadField("type must be a string".to_string()))?;

        let payload_version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        if payload_version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedPayloadVersion(payload_version));
        }

        match payload_type {
            "WorkflowStarted" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                Ok(EventPayload::WorkflowStarted { workflow_id })
            }
            "WorkflowCompleted" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let completion_time_ms = value.get("completion_time_ms")
                    .ok_or_else(|| Error::MissingPayloadField("completion_time_ms".to_string()))?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidPayloadField("completion_time_ms must be an integer".to_string()))?;
                Ok(EventPayload::WorkflowCompleted { workflow_id, completion_time_ms })
            }
            "WorkflowFailed" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let failure_reason = value.get("failure_reason")
                    .ok_or_else(|| Error::MissingPayloadField("failure_reason".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("failure_reason must be a string".to_string()))?
                    .to_string();
                Ok(EventPayload::WorkflowFailed { workflow_id, failure_reason })
            }
            "WorkflowCancelled" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let cancelled_by = value.get("cancelled_by")
                    .ok_or_else(|| Error::MissingPayloadField("cancelled_by".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("cancelled_by must be a string".to_string()))?
                    .to_string();
                Ok(EventPayload::WorkflowCancelled { workflow_id, cancelled_by })
            }
            "StepScheduled" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let step_id = value.get("step_id")
                    .ok_or_else(|| Error::MissingPayloadField("step_id".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("step_id must be a string".to_string()))?
                    .to_string();
                Ok(EventPayload::StepScheduled { workflow_id, step_id })
            }
            "StepStarted" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let step_id = value.get("step_id")
                    .ok_or_else(|| Error::MissingPayloadField("step_id".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("step_id must be a string".to_string()))?
                    .to_string();
                let started_at_ms = value.get("started_at_ms")
                    .ok_or_else(|| Error::MissingPayloadField("started_at_ms".to_string()))?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidPayloadField("started_at_ms must be an integer".to_string()))?;
                Ok(EventPayload::StepStarted { workflow_id, step_id, started_at_ms })
            }
            "StepCompleted" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let step_id = value.get("step_id")
                    .ok_or_else(|| Error::MissingPayloadField("step_id".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("step_id must be a string".to_string()))?
                    .to_string();
                let completed_at_ms = value.get("completed_at_ms")
                    .ok_or_else(|| Error::MissingPayloadField("completed_at_ms".to_string()))?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidPayloadField("completed_at_ms must be an integer".to_string()))?;
                Ok(EventPayload::StepCompleted { workflow_id, step_id, completed_at_ms })
            }
            "StepFailed" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let step_id = value.get("step_id")
                    .ok_or_else(|| Error::MissingPayloadField("step_id".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("step_id must be a string".to_string()))?
                    .to_string();
                let failure_reason = value.get("failure_reason")
                    .ok_or_else(|| Error::MissingPayloadField("failure_reason".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("failure_reason must be a string".to_string()))?
                    .to_string();
                Ok(EventPayload::StepFailed { workflow_id, step_id, failure_reason })
            }
            "TimerSet" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let timer_id = value.get("timer_id")
                    .ok_or_else(|| Error::MissingPayloadField("timer_id".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("timer_id must be a string".to_string()))?
                    .to_string();
                let fire_at_ms = value.get("fire_at_ms")
                    .ok_or_else(|| Error::MissingPayloadField("fire_at_ms".to_string()))?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidPayloadField("fire_at_ms must be an integer".to_string()))?;
                Ok(EventPayload::TimerSet { workflow_id, timer_id, fire_at_ms })
            }
            "TimerFired" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let timer_id = value.get("timer_id")
                    .ok_or_else(|| Error::MissingPayloadField("timer_id".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("timer_id must be a string".to_string()))?
                    .to_string();
                let fired_at_ms = value.get("fired_at_ms")
                    .ok_or_else(|| Error::MissingPayloadField("fired_at_ms".to_string()))?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidPayloadField("fired_at_ms must be an integer".to_string()))?;
                Ok(EventPayload::TimerFired { workflow_id, timer_id, fired_at_ms })
            }
            "CancelRequested" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let requested_by = value.get("requested_by")
                    .ok_or_else(|| Error::MissingPayloadField("requested_by".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("requested_by must be a string".to_string()))?
                    .to_string();
                Ok(EventPayload::CancelRequested { workflow_id, requested_by })
            }
            "InstanceResumed" => {
                let workflow_id = value.get("workflow_id")
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id is required".to_string()))?
                    .as_str()
                    .ok_or_else(|| Error::InvalidPayloadField("workflow_id must be a string".to_string()))?
                    .to_string();
                let resumed_at_ms = value.get("resumed_at_ms")
                    .ok_or_else(|| Error::MissingPayloadField("resumed_at_ms".to_string()))?
                    .as_u64()
                    .ok_or_else(|| Error::InvalidPayloadField("resumed_at_ms must be an integer".to_string()))?;
                Ok(EventPayload::InstanceResumed { workflow_id, resumed_at_ms })
            }
            _ => Err(Error::UnknownPayloadType(payload_type.to_string())),
        }
    }

    pub fn is_version_supported(version: u8) -> bool {
        version <= MAX_SUPPORTED_VERSION
    }
}

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
    let payload = EventPayload::try_from_json(envelope.payload.clone())
        .map_err(|e| Error::PayloadDecodeFailed(Box::new(e)))?;
    Ok((envelope, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_from_bytes_returns_ok_when_input_is_valid_json() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert!(result.is_ok());
        let envelope = result.unwrap();
        assert_eq!(envelope.version, 1);
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
        assert_eq!(result, Err(Error::MissingEnvelopeField("version".to_string())));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_instance_id_is_absent() {
        let json = r#"{"version": 1, "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::MissingEnvelopeField("instance_id".to_string())));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_sequence_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::MissingEnvelopeField("sequence".to_string())));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_timestamp_ms_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "payload": {"x":0}, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::MissingEnvelopeField("timestamp_ms".to_string())));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_payload_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "metadata": {}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::MissingEnvelopeField("payload".to_string())));
    }

    #[test]
    fn envelope_from_bytes_returns_missing_envelope_field_when_metadata_is_absent() {
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}}"#;
        let result = EventEnvelope::from_bytes(json.as_bytes());
        assert_eq!(result, Err(Error::MissingEnvelopeField("metadata".to_string())));
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
        assert!(result.is_ok());
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
            version: 0,
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
            version: 1,
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
            version: 2,
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
        let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::WorkflowStarted { workflow_id: "wf-123".to_string() }));
    }

    #[test]
    fn payload_try_from_json_returns_workflow_completed_when_type_is_workflow_completed() {
        let json = serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "wf-123", "completion_time_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::WorkflowCompleted { workflow_id: "wf-123".to_string(), completion_time_ms: 1000 }));
    }

    #[test]
    fn payload_try_from_json_returns_workflow_failed_when_type_is_workflow_failed() {
        let json = serde_json::json!({"type": "WorkflowFailed", "workflow_id": "wf-123", "failure_reason": "timeout", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::WorkflowFailed { workflow_id: "wf-123".to_string(), failure_reason: "timeout".to_string() }));
    }

    #[test]
    fn payload_try_from_json_returns_workflow_cancelled_when_type_is_workflow_cancelled() {
        let json = serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "wf-123", "cancelled_by": "user", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::WorkflowCancelled { workflow_id: "wf-123".to_string(), cancelled_by: "user".to_string() }));
    }

    #[test]
    fn payload_try_from_json_returns_step_scheduled_when_type_is_step_scheduled() {
        let json = serde_json::json!({"type": "StepScheduled", "workflow_id": "wf-123", "step_id": "step-1", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::StepScheduled { workflow_id: "wf-123".to_string(), step_id: "step-1".to_string() }));
    }

    #[test]
    fn payload_try_from_json_returns_step_started_when_type_is_step_started() {
        let json = serde_json::json!({"type": "StepStarted", "workflow_id": "wf-123", "step_id": "step-1", "started_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::StepStarted { workflow_id: "wf-123".to_string(), step_id: "step-1".to_string(), started_at_ms: 1000 }));
    }

    #[test]
    fn payload_try_from_json_returns_step_completed_when_type_is_step_completed() {
        let json = serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-123", "step_id": "step-1", "completed_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::StepCompleted { workflow_id: "wf-123".to_string(), step_id: "step-1".to_string(), completed_at_ms: 1000 }));
    }

    #[test]
    fn payload_try_from_json_returns_step_failed_when_type_is_step_failed() {
        let json = serde_json::json!({"type": "StepFailed", "workflow_id": "wf-123", "step_id": "step-1", "failure_reason": "error", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::StepFailed { workflow_id: "wf-123".to_string(), step_id: "step-1".to_string(), failure_reason: "error".to_string() }));
    }

    #[test]
    fn payload_try_from_json_returns_timer_set_when_type_is_timer_set() {
        let json = serde_json::json!({"type": "TimerSet", "workflow_id": "wf-123", "timer_id": "timer-1", "fire_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::TimerSet { workflow_id: "wf-123".to_string(), timer_id: "timer-1".to_string(), fire_at_ms: 1000 }));
    }

    #[test]
    fn payload_try_from_json_returns_timer_fired_when_type_is_timer_fired() {
        let json = serde_json::json!({"type": "TimerFired", "workflow_id": "wf-123", "timer_id": "timer-1", "fired_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::TimerFired { workflow_id: "wf-123".to_string(), timer_id: "timer-1".to_string(), fired_at_ms: 1000 }));
    }

    #[test]
    fn payload_try_from_json_returns_cancel_requested_when_type_is_cancel_requested() {
        let json = serde_json::json!({"type": "CancelRequested", "workflow_id": "wf-123", "requested_by": "user", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::CancelRequested { workflow_id: "wf-123".to_string(), requested_by: "user".to_string() }));
    }

    #[test]
    fn payload_try_from_json_returns_instance_resumed_when_type_is_instance_resumed() {
        let json = serde_json::json!({"type": "InstanceResumed", "workflow_id": "wf-123", "resumed_at_ms": 1000, "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Ok(EventPayload::InstanceResumed { workflow_id: "wf-123".to_string(), resumed_at_ms: 1000 }));
    }

    #[test]
    fn payload_try_from_json_returns_unknown_payload_type_when_type_is_unrecognized() {
        let json = serde_json::json!({"type": "UnknownType", "workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Err(Error::UnknownPayloadType("UnknownType".to_string())));
    }

    #[test]
    fn payload_try_from_json_returns_unsupported_payload_version_when_version_exceeds_max() {
        let json = serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 2});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Err(Error::UnsupportedPayloadVersion(2)));
    }

    #[test]
    fn payload_try_from_json_returns_missing_payload_field_when_type_is_absent() {
        let json = serde_json::json!({"workflow_id": "wf-123", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert_eq!(result, Err(Error::MissingPayloadField("type".to_string())));
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_field_when_variant_field_is_absent() {
        let json = serde_json::json!({"type": "WorkflowStarted", "version": 1});
        let result = EventPayload::try_from_json(json);
        assert!(matches!(result, Err(Error::InvalidPayloadField(_))));
    }

    #[test]
    fn payload_try_from_json_returns_invalid_payload_format_when_json_is_malformed() {
        let json = serde_json::Value::String("not an object".to_string());
        let result = EventPayload::try_from_json(json);
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
        let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1}, "metadata": {}}"#;
        let result = decode_event(json.as_bytes());
        assert!(result.is_ok());
        let (envelope, payload) = result.unwrap();
        assert_eq!(envelope.version, 1);
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

    #[test]
    fn proptest_envelope_roundtrip_preserves_data() {
        let test_cases = vec![
            (0u64, 1, "wf-1", 0),
            (1u64, 100, "wf-abc", 1000),
        ];

        for (version, seq, instance_id, ts) in test_cases {
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
            assert!(result.is_ok(), "Failed for version {}", version);
        }
    }

    #[test]
    fn proptest_version_support_is_consistent_across_envelope_and_payload() {
        for version in 0..=5u8 {
            let envelope = EventEnvelope {
                version,
                instance_id: "wf-123".to_string(),
                sequence: 1,
                timestamp_ms: 1000,
                payload: serde_json::json!({}),
                metadata: serde_json::json!({}),
            };
            let envelope_supported = envelope.is_supported();
            let payload_supported = EventPayload::is_version_supported(version);
            assert_eq!(envelope_supported, payload_supported, "Inconsistent for version {}", version);
        }
    }

    #[test]
    fn proptest_sequence_is_always_positive_on_success() {
        let valid_cases = vec![(1u64, "wf-1"), (100u64, "wf-100"), (999999999u64, "wf-max")];
        let invalid_cases = vec![(0u64, "wf-zero")];

        for (seq, instance_id) in valid_cases {
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
            assert!(result.is_ok(), "Failed for valid sequence: {}", seq);
        }

        for (seq, instance_id) in invalid_cases {
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
            assert!(result.is_err(), "Should have failed for sequence: {}", seq);
        }
    }

    #[test]
    fn proptest_instance_id_is_always_nonempty_on_success() {
        let valid_cases = vec!["a", "wf-123", "instance_with_underscores"];
        let invalid_cases = vec![""];

        for instance_id in valid_cases {
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
            assert!(result.is_ok(), "Failed for valid instance_id: {}", instance_id);
        }

        for instance_id in invalid_cases {
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
            assert!(result.is_err(), "Should have failed for empty instance_id");
        }
    }

    #[test]
    fn proptest_metadata_is_always_object_on_success() {
        let valid_metadata = vec![
            serde_json::json!({}),
            serde_json::json!({"key": "value"}),
            serde_json::json!({"key1": "value1", "key2": 123}),
        ];

        let invalid_metadata = vec![
            serde_json::json!([]),
            serde_json::json!("string"),
            serde_json::Value::Null,
            serde_json::json!(123),
        ];

        for metadata in valid_metadata {
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
            assert!(result.is_ok(), "Failed for valid metadata");
        }

        for metadata in invalid_metadata {
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
            assert!(result.is_err(), "Should have failed for invalid metadata");
        }
    }
}
