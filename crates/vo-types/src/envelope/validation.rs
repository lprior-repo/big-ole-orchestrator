//! Validation helpers for `CommandEnvelope` parsing.

use super::signing::{CommandEnvelope, CommandEnvelopeError};

// ---------------------------------------------------------------------------
// Validation: is_supported
// ---------------------------------------------------------------------------

impl CommandEnvelope {
    /// Returns `true` if the envelope version is supported.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.schema_version <= super::signing::MAX_SUPPORTED_COMMAND_VERSION
    }
}

// ---------------------------------------------------------------------------
// Helper Functions
// ---------------------------------------------------------------------------

pub(super) fn envelope_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<String, CommandEnvelopeError> {
    obj.get(field)
        .ok_or_else(|| CommandEnvelopeError::MissingEnvelopeField(field.to_string()))?
        .as_str()
        .ok_or_else(|| {
            CommandEnvelopeError::InvalidEnvelopeField(format!("{field} must be a string"))
        })
        .map(std::string::ToString::to_string)
}

pub(super) fn envelope_u64(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u64, CommandEnvelopeError> {
    obj.get(field)
        .ok_or_else(|| CommandEnvelopeError::MissingEnvelopeField(field.to_string()))?
        .as_u64()
        .ok_or_else(|| {
            CommandEnvelopeError::InvalidEnvelopeField(format!("{field} must be an integer"))
        })
}

pub(super) fn envelope_u8(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u8, CommandEnvelopeError> {
    let value = envelope_u64(obj, field)?;
    u8::try_from(value).map_err(|_| {
        CommandEnvelopeError::InvalidEnvelopeField(
            format!("{field} exceeds maximum allowed value",),
        )
    })
}

pub(super) fn parse_issuer(s: &str) -> Result<super::super::Issuer, CommandEnvelopeError> {
    match s {
        "system" => Ok(super::super::Issuer::System),
        "api_client" => Ok(super::super::Issuer::ApiClient),
        "operator" => Ok(super::super::Issuer::Operator),
        "ai_agent" => Ok(super::super::Issuer::AiAgent),
        "timer_loop" => Ok(super::super::Issuer::TimerLoop),
        "recovery_loop" => Ok(super::super::Issuer::RecoveryLoop),
        other => Err(CommandEnvelopeError::InvalidEnvelopeField(format!(
            "unknown issuer: {other}"
        ))),
    }
}
