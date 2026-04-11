//! Event envelope types and parsing.

use crate::events::error::Error;
use crate::events::metadata::EventMetadata;
use crate::events::MAX_SUPPORTED_VERSION;

#[derive(Debug, Clone, PartialEq)]
pub struct EventEnvelope {
    pub schema_version: u8,
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
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

        // Validate payload is an object (not string, array, null, etc.)
        let _payload_obj = payload
            .as_object()
            .ok_or_else(|| Error::InvalidEnvelopeField("payload".to_string()))?;

        // metadata is optional - use default (command_metadata: None) when absent (POST-6)
        let metadata = match obj.get("metadata") {
            Some(v) => {
                // Validate metadata is an object before parsing as EventMetadata
                let v_obj = v.as_object().ok_or_else(|| {
                    Error::InvalidEnvelopeField("metadata must be an object".to_string())
                })?;
                EventMetadata::from_json(&serde_json::Value::Object(v_obj.clone()))?
            }
            None => EventMetadata::default(),
        };

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
            metadata,
        })
    }

    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.schema_version <= MAX_SUPPORTED_VERSION
    }
}

// ---------------------------------------------------------------------------
// Envelope field extraction helpers (EnvelopeError variant)
// ---------------------------------------------------------------------------

pub(crate) fn envelope_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, Error> {
    obj.get(field)
        .ok_or_else(|| Error::MissingEnvelopeField(field.to_string()))?
        .as_str()
        .ok_or_else(|| Error::InvalidEnvelopeField(format!("{field} must be a string")))
        .map(std::string::ToString::to_string)
}

pub(crate) fn envelope_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u64, Error> {
    obj.get(field)
        .ok_or_else(|| Error::MissingEnvelopeField(field.to_string()))?
        .as_u64()
        .ok_or_else(|| Error::InvalidEnvelopeField(format!("{field} must be an integer")))
}
