//! Error types for DbWriterMessage operations.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for DbWriterMessage operations.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbWriterMessageError {
    /// Fence token was zero (must be nonzero).
    #[error("fence token must be nonzero")]
    ZeroFenceToken,
    /// Sequence number was zero (must be nonzero).
    #[error("sequence number must be nonzero")]
    ZeroSequenceNumber,
    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),
    /// Unknown variant tag encountered during deserialization.
    #[error("unknown DbWriterMessage variant: {0}")]
    UnknownVariant(String),
    /// Required field was missing during deserialization.
    #[error("missing field: {0}")]
    MissingField(String),
}
