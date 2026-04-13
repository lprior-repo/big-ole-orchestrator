//! Error types for the projection engine (ADR-037).
//!
//! ## Error Taxonomy
//!
//! - `ProjectionError` — top-level errors from projection operations
//! - `ProjectionStateError` — errors from invalid state transitions
//! - `ProjectionVersionError` — schema version compatibility errors
//! - `ReplayError` — event replay failures
//! - `StorageError` — persistence layer failures

use thiserror::Error;

/// Top-level error type for projection operations.
#[derive(Debug, Clone, Error)]
pub enum ProjectionError {
    #[error("projection '{0}' not found")]
    ProjectionNotFound(String),

    #[error("projection '{0}' already exists")]
    ProjectionAlreadyExists(String),

    #[error("invalid projection state: {0}")]
    InvalidState(String),

    #[error("projection build failed: {0}")]
    BuildFailed(String),

    #[error("projection rebuild failed: {0}")]
    RebuildFailed(String),

    #[error("upcasting failed: {0}")]
    UpcastingFailed(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("incompatible schema version: expected {expected}, got {actual}")]
    IncompatibleSchemaVersion { expected: u8, actual: u8 },

    #[error("sequence gap detected at {0}")]
    SequenceGap(u64),

    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u64, actual: u64 },

    #[error("concurrency conflict: {0}")]
    ConcurrencyConflict(String),

    #[error("throttle exceeded, wait {0}ms")]
    ThrottleExceeded(u64),

    #[error("projection is in failed state: {0}")]
    FailedState(String),
}

impl ProjectionError {
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ThrottleExceeded(_) | Self::ConcurrencyConflict(_) | Self::Storage(_)
        )
    }
}

/// Errors from invalid projection state transitions.
#[derive(Debug, Clone, Error)]
pub enum ProjectionStateError {
    #[error("cannot transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("projection is not in expected state: {0}")]
    UnexpectedState(String),

    #[error("projection is building, operation not permitted")]
    StillBuilding,

    #[error("projection is rebuilding, operation not permitted")]
    StillRebuilding,
}

/// Schema version compatibility errors.
#[derive(Debug, Clone, Error)]
pub enum ProjectionVersionError {
    #[error("schema version {0} is stale, cannot be upcast")]
    StaleVersion(u8),

    #[error("schema version {0} exceeds maximum supported {1}")]
    ExceedsMaxSupported(u8, u8),

    #[error("schema version {0} is invalid (must be >= 1)")]
    InvalidVersion(u8),

    #[error("upcast chain incomplete: missing upcaster for version {0}")]
    MissingUpcaster(u8),
}

/// Event replay errors during projection rebuild.
#[derive(Debug, Clone)]
pub enum ReplayError {
    InstanceMismatch {
        expected: String,
        actual: String,
    },
    SequenceGap {
        expected: u64,
        actual: u64,
        at_index: usize,
    },
    SequenceDuplicate {
        sequence: u64,
        first: usize,
        second: usize,
    },
    PayloadDecodeFailed {
        sequence: u64,
        source: String,
    },
    TransitionFailed {
        sequence: u64,
        state: String,
        reason: String,
    },
    UnexpectedEventType {
        payload_type: String,
        sequence: u64,
    },
    UpcastFailed {
        sequence: u64,
        reason: String,
    },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstanceMismatch { expected, actual } => {
                write!(
                    f,
                    "instance ID mismatch: expected '{expected}', got '{actual}'"
                )
            }
            Self::SequenceGap {
                expected,
                actual,
                at_index,
            } => {
                write!(
                    f,
                    "sequence gap at index {at_index}: expected {expected}, got {actual}"
                )
            }
            Self::SequenceDuplicate {
                sequence,
                first,
                second,
            } => {
                write!(
                    f,
                    "duplicate sequence {sequence} at indices {first} and {second}"
                )
            }
            Self::PayloadDecodeFailed { sequence, source } => {
                write!(f, "payload decode failed at sequence {sequence}: {source}")
            }
            Self::TransitionFailed {
                sequence,
                state,
                reason,
            } => {
                write!(
                    f,
                    "transition failed at sequence {sequence} in state {state}: {reason}"
                )
            }
            Self::UnexpectedEventType {
                payload_type,
                sequence,
            } => {
                write!(
                    f,
                    "event type '{payload_type}' at sequence {sequence} has no mapping"
                )
            }
            Self::UpcastFailed { sequence, reason } => {
                write!(f, "upcasting failed at sequence {sequence}: {reason}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// Storage layer errors for projection persistence.
#[derive(Debug, Clone, Error)]
pub enum StorageError {
    #[error("record not found for projection '{0}'")]
    RecordNotFound(String),

    #[error("serialization failed: {0}")]
    SerializationFailed(String),

    #[error("deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("corrupt record: {0}")]
    CorruptRecord(String),

    #[error("write failed: {0}")]
    WriteFailed(String),

    #[error("batch full for class {class}, depth {depth}/{capacity}")]
    BatchFull {
        class: String,
        depth: usize,
        capacity: usize,
    },

    #[error("budget exceeded for class {class}: item size {item_size}, remaining {remaining}")]
    BudgetExceeded {
        class: String,
        item_size: u64,
        remaining: u64,
    },
}

impl From<StorageError> for ProjectionError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::RecordNotFound(id) => Self::ProjectionNotFound(id),
            StorageError::SerializationFailed(s) => Self::Storage(s),
            StorageError::DeserializationFailed(s) => Self::Storage(s),
            StorageError::CorruptRecord(s) => Self::Storage(s),
            StorageError::WriteFailed(s) => Self::Storage(s),
            StorageError::BatchFull { .. } => Self::Storage(format!("{:?}", e)),
            StorageError::BudgetExceeded { .. } => Self::Storage(format!("{:?}", e)),
        }
    }
}
