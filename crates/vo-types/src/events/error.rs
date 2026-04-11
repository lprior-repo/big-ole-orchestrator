//! Event error types.

use thiserror::Error;

/// Errors that can occur during event decoding or serialization.
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

    #[error("Invalid command metadata")]
    InvalidCommandMetadata,

    #[error("Invalid issuer: {0}")]
    InvalidIssuer(String),
}
