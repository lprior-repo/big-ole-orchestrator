//! JSON parsing helpers for CommandEnvelope correlation/causation fields.
//!
//! This module handles the low-level JSON deserialization that powers
//! `CommandEnvelope::from_str` and `CommandEnvelope::from_bytes`.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum supported command envelope version.
///
/// Currently version 1 is supported. Higher versions will be rejected.
pub const MAX_SUPPORTED_COMMAND_VERSION: u8 = 1;

use thiserror::Error;

use crate::{CommandEnvelope, CommandEnvelopeError};

// ---------------------------------------------------------------------------
// Helper Functions
// ---------------------------------------------------------------------------

fn envelope_string(
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

fn envelope_u64(
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

fn envelope_u8(
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

fn parse_issuer(s: &str) -> Result<crate::Issuer, CommandEnvelopeError> {
    match s {
        "system" => Ok(crate::Issuer::System),
        "api_client" => Ok(crate::Issuer::ApiClient),
        "operator" => Ok(crate::Issuer::Operator),
        "ai_agent" => Ok(crate::Issuer::AiAgent),
        "timer_loop" => Ok(crate::Issuer::TimerLoop),
        "recovery_loop" => Ok(crate::Issuer::RecoveryLoop),
        other => Err(CommandEnvelopeError::InvalidEnvelopeField(format!(
            "unknown issuer: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Envelope Parsing
// ---------------------------------------------------------------------------

/// Parse a `CommandEnvelope` from a JSON string.
///
/// # Errors
///
/// Returns envelope-level errors if the JSON is malformed, missing
/// required fields, or contains an unsupported version.
pub(super) fn parse_envelope(input: &str) -> Result<crate::CommandEnvelope, CommandEnvelopeError> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|_| CommandEnvelopeError::InvalidEnvelopeFormat)?;

    let obj = value
        .as_object()
        .ok_or(CommandEnvelopeError::InvalidEnvelopeFormat)?;

    // Parse version
    let version = envelope_u8(obj, "version")?;
    if version > MAX_SUPPORTED_COMMAND_VERSION {
        return Err(CommandEnvelopeError::UnsupportedEnvelopeVersion(version));
    }

    // Parse required string fields
    let command_id = envelope_string(obj, "command_id")?;
    let correlation_id = envelope_string(obj, "correlation_id")?;
    let causation_id = envelope_string(obj, "causation_id")?;
    let issuer_str = envelope_string(obj, "issuer")?;
    let issued_at = envelope_u64(obj, "issued_at")?;

    // Parse issuer enum
    let issuer = parse_issuer(&issuer_str)?;

    // Parse metadata
    let metadata = crate::CommandMetadata {
        command_id: crate::IdempotencyKey::parse(&command_id).map_err(|e| {
            CommandEnvelopeError::InvalidEnvelopeField(format!("command_id: {}", e))
        })?,
        correlation_id: crate::IdempotencyKey::parse(&correlation_id).map_err(|e| {
            CommandEnvelopeError::InvalidEnvelopeField(format!("correlation_id: {}", e))
        })?,
        causation_id: crate::IdempotencyKey::parse(&causation_id).map_err(|e| {
            CommandEnvelopeError::InvalidEnvelopeField(format!("causation_id: {}", e))
        })?,
        issuer,
        issued_at: crate::TimestampMs::try_from(issued_at).map_err(|_| {
            CommandEnvelopeError::InvalidEnvelopeField(
                "issued_at exceeds maximum allowed value".to_string(),
            )
        })?,
    };

    Ok(CommandEnvelope {
        schema_version: version,
        metadata,
    })
}
