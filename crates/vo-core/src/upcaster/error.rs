//! Error types for upcasting operations.

use thiserror::Error;
use vo_types::events::Error as EventEnvelopeErrorAlias;

// Re-export MAX_SUPPORTED_VERSION before use in error attribute
pub use vo_types::events::MAX_SUPPORTED_VERSION;

// Re-export EventEnvelopeError from vo-types (aliased as Error in events.rs)
pub use vo_types::events::Error as EventEnvelopeError;

/// Errors that can occur during upcasting operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UpcasterError {
    /// No upcaster registered for source version during upcast chain resolution.
    #[error("No upcaster registered for source version: {0}")]
    NoUpcasterRegistered(u8),

    /// An upcaster is already registered for the given source version.
    #[error("Duplicate upcaster registration for source version: {0}")]
    DuplicateRegistration(u8),

    #[error("Upcasting failed: {0}")]
    UpcastingFailed(String),

    #[error("Invalid target version: {0} (maximum supported: {MAX_SUPPORTED_VERSION})")]
    InvalidTargetVersion(u8),

    #[error("Circular upcaster chain detected for version: {0}")]
    CircularChain(u8),

    #[error("Upcaster produced invalid envelope: {0}")]
    InvalidUpcastedEnvelope(#[from] EventEnvelopeErrorAlias),
}
