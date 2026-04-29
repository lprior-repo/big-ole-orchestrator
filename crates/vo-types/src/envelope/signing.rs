//! Envelope signing (parsing/decoding) for `CommandEnvelope`.
//!
//! Handles decoding raw bytes/strings into a validated `CommandEnvelope`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::validation::{envelope_string, envelope_u64, envelope_u8, parse_issuer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum supported command envelope version.
///
/// Currently version 1 is supported. Higher versions will be rejected.
pub const MAX_SUPPORTED_COMMAND_VERSION: u8 = 1;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors that can occur when parsing or validating a `CommandEnvelope`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandEnvelopeError {
    /// Input bytes are not valid UTF-8.
    #[error("Input bytes are not valid UTF-8")]
    InvalidInput,

    /// Envelope JSON is malformed.
    #[error("Envelope JSON is malformed")]
    InvalidEnvelopeFormat,

    /// Missing required envelope field.
    #[error("Missing envelope field: {0}")]
    MissingEnvelopeField(String),

    /// Invalid envelope field (wrong type or invalid value).
    #[error("Invalid envelope field: {0}")]
    InvalidEnvelopeField(String),

    /// Unsupported envelope version.
    #[error("Unsupported envelope version: {0}")]
    UnsupportedEnvelopeVersion(u8),

    /// Envelope decode failed (nested error).
    #[error("Envelope decode failed: {0}")]
    EnvelopeDecodeFailed(Box<CommandEnvelopeError>),
}

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

/// Command envelope containing command identity metadata.
///
/// Every mutating API or CLI action enters the Engine as a versioned
/// `CommandEnvelope`. This provides durable lineage for events:
///
/// - `command_id` — stable identity for dedupe and idempotent retries
/// - `correlation_id` — groups all work caused by a higher-level business request
/// - `causation_id` — points to the immediate parent event or command that caused this
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    #[serde(rename = "version")]
    pub schema_version: u8,
    #[serde(flatten)]
    pub metadata: super::super::CommandMetadata,
}

impl CommandEnvelope {
    /// Decode a `CommandEnvelope` from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if the bytes are not valid UTF-8, or
    /// various envelope errors if the JSON is malformed or missing required fields.
    pub fn from_bytes(input: &[u8]) -> Result<Self, CommandEnvelopeError> {
        let json_str =
            std::str::from_utf8(input).map_err(|_| CommandEnvelopeError::InvalidInput)?;
        Self::from_str(json_str)
    }

    /// Decode a `CommandEnvelope` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns envelope-level errors if the JSON is malformed, missing
    /// required fields, or contains an unsupported version.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &str) -> Result<Self, CommandEnvelopeError> {
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
        let metadata = super::super::CommandMetadata {
            command_id: super::super::IdempotencyKey::parse(&command_id).map_err(|e| {
                CommandEnvelopeError::InvalidEnvelopeField(format!("command_id: {}", e))
            })?,
            correlation_id: super::super::IdempotencyKey::parse(&correlation_id).map_err(|e| {
                CommandEnvelopeError::InvalidEnvelopeField(format!("correlation_id: {}", e))
            })?,
            causation_id: super::super::IdempotencyKey::parse(&causation_id).map_err(|e| {
                CommandEnvelopeError::InvalidEnvelopeField(format!("causation_id: {}", e))
            })?,
            issuer,
            issued_at: super::super::TimestampMs::try_from(issued_at).map_err(|_| {
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
}
