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

        #[allow(clippy::cast_possible_truncation)]
        // version validated <= MAX_SUPPORTED_VERSION (u8)
        let version = envelope_u64(obj, "version")? as u8;
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
            version,
            instance_id,
            sequence,
            timestamp_ms,
            payload: payload.clone(),
            metadata: serde_json::Value::Object(metadata.clone()),
        })
    }

    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.version <= MAX_SUPPORTED_VERSION
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    WorkflowStarted {
        workflow_id: String,
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
    },
    StepFailed {
        workflow_id: String,
        step_id: String,
        failure_reason: String,
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
        #[allow(clippy::cast_possible_truncation)]
        // version validated <= MAX_SUPPORTED_VERSION (u8)
        let payload_version = optional_u64(obj, "version", 0) as u8;
        if payload_version > MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedPayloadVersion(payload_version));
        }

        match payload_type.as_str() {
            "WorkflowStarted" => Ok(EventPayload::WorkflowStarted {
                workflow_id: require_string_field(obj, "workflow_id")?,
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
            }),
            "StepFailed" => Ok(EventPayload::StepFailed {
                workflow_id: require_string_field(obj, "workflow_id")?,
                step_id: require_string(obj, "step_id")?,
                failure_reason: require_string(obj, "failure_reason")?,
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
