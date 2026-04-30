//! CommandEnvelope type for command identity, correlation, and causation (ADR-036).
//!
//! Every mutating API or CLI action enters the Engine as a versioned `CommandEnvelope`.
//! This module provides the canonical command envelope with parsing and validation.

mod correlation;
mod metadata;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum supported command envelope version.
///
/// Currently version 1 is supported. Higher versions will be rejected.
pub use correlation::MAX_SUPPORTED_COMMAND_VERSION;

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
    pub metadata: super::CommandMetadata,
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
        correlation::parse_envelope(json_str)
    }

    /// Decode a `CommandEnvelope` from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns envelope-level errors if the JSON is malformed, missing
    /// required fields, or contains an unsupported version.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &str) -> Result<Self, CommandEnvelopeError> {
        correlation::parse_envelope(input)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod parsing_tests;
#[cfg(test)]
mod validation_tests;
#[cfg(test)]
mod behavior_tests;
