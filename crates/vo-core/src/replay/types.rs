//! Result and error types for the replay engine.

use std::error::Error as StdError;
use thiserror::Error;
use vo_types::state::LifecycleState;

/// Wrapper for String that implements std::error::Error for use with thiserror.
/// This allows storing error details in error fields without requiring the inner
/// type to implement Error directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorString(pub String);

impl std::fmt::Display for ErrorString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl StdError for ErrorString {}

/// Categorizes replay errors to determine system behavior.
/// Deterministic errors mark state as permanently blocked/corrupt.
/// Transient errors should be retried by the infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayErrorKind {
    /// Permanent corruption - state cannot be recovered, no retry should occur.
    Deterministic,
    /// Temporary failure - infrastructure should retry the operation.
    Transient,
}

/// Result of replaying events through the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayResult {
    /// Final reconstructed lifecycle state. `None` if no events were applied.
    pub final_state: Option<LifecycleState>,
    /// Number of events successfully applied.
    pub events_applied: usize,
    /// Latest fence token observed from StepScheduled events during replay.
    /// This is the fence that should be restored after restart/recovery.
    /// `None` if no StepScheduled events were replayed.
    pub latest_fence: Option<u64>,
}

/// Errors that can occur during event replay.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReplayError {
    #[error("instance mismatch: expected {expected}, actual {actual}")]
    InstanceMismatch { expected: String, actual: String },
    #[error("sequence gap at index {at_index}: expected {expected}, actual {actual}")]
    SequenceGap {
        expected: u64,
        actual: u64,
        at_index: usize,
    },
    #[error("duplicate sequence {sequence} at indices {first_at_index} and {second_at_index}")]
    SequenceDuplicate {
        sequence: u64,
        first_at_index: usize,
        second_at_index: usize,
    },
    #[error("payload decode failed at sequence {sequence}: {source}")]
    PayloadDecodeFailed { sequence: u64, source: ErrorString },
    #[error("transition failed at sequence {sequence}: {reason}")]
    TransitionFailed {
        sequence: u64,
        state: LifecycleState,
        reason: String,
    },
    #[error("unexpected event type {payload_type} at sequence {sequence}")]
    UnexpectedEventType { payload_type: String, sequence: u64 },
    #[error("upcasting failed at sequence {sequence}: {reason}")]
    UpcastingFailed { sequence: u64, reason: String },
    /// Blob publication failed for a required output (ADR-040 §3).
    /// The step stays incomplete and may be retried or failed.
    #[error("Blob publication failed at sequence {sequence} for step {step_id}: blob {blob_id}")]
    BlobPublicationFailed {
        sequence: u64,
        step_id: String,
        blob_id: String,
    },
}

impl ReplayError {
    pub fn kind(&self) -> ReplayErrorKind {
        match self {
            ReplayError::InstanceMismatch { .. } => ReplayErrorKind::Deterministic,
            ReplayError::SequenceGap { .. } => ReplayErrorKind::Deterministic,
            ReplayError::SequenceDuplicate { .. } => ReplayErrorKind::Deterministic,
            ReplayError::PayloadDecodeFailed { .. } => ReplayErrorKind::Deterministic,
            ReplayError::TransitionFailed { .. } => ReplayErrorKind::Deterministic,
            ReplayError::UnexpectedEventType { .. } => ReplayErrorKind::Deterministic,
            ReplayError::UpcastingFailed { .. } => ReplayErrorKind::Deterministic,
            ReplayError::BlobPublicationFailed { .. } => ReplayErrorKind::Transient,
        }
    }
}
