//! Result and error types for the replay engine.

use thiserror::Error;
use vo_types::state::LifecycleState;

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
}

/// Errors that can occur during event replay.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReplayError {
    #[error("Instance ID mismatch: expected '{expected}', got '{actual}'")]
    InstanceMismatch { expected: String, actual: String },
    #[error("Sequence gap at index {at_index}: expected {expected}, got {actual}")]
    SequenceGap {
        expected: u64,
        actual: u64,
        at_index: usize,
    },
    #[error("Duplicate sequence {sequence} at indices {first_at_index} and {second_at_index}")]
    SequenceDuplicate {
        sequence: u64,
        first_at_index: usize,
        second_at_index: usize,
    },
    #[error("Payload decode failed at sequence {sequence}: {detail}")]
    PayloadDecodeFailed { sequence: u64, detail: String },
    #[error("Transition failed at sequence {sequence} in state {state:?}: {reason}")]
    TransitionFailed {
        sequence: u64,
        state: LifecycleState,
        reason: String,
    },
    #[error("Unexpected event type '{payload_type}' at sequence {sequence}")]
    UnexpectedEventType { payload_type: String, sequence: u64 },
    #[error("Upcasting failed at sequence {sequence}: {reason}")]
    UpcastingFailed { sequence: u64, reason: String },
    /// Blob publication failed for a required output (ADR-040 §3).
    /// The step stays incomplete and may be retried or failed.
    #[error("Blob publication failed at sequence {sequence} for step '{step_id}': blob {blob_id}")]
    BlobPublicationFailed {
        sequence: u64,
        step_id: String,
        blob_id: String,
    },
}

impl ReplayError {
    /// Classify this error as deterministic (permanent) or transient (retryable).
    #[must_use]
    pub const fn kind(&self) -> ReplayErrorKind {
        match self {
            Self::InstanceMismatch { .. }
            | Self::SequenceGap { .. }
            | Self::SequenceDuplicate { .. }
            | Self::PayloadDecodeFailed { .. }
            | Self::TransitionFailed { .. }
            | Self::UnexpectedEventType { .. }
            | Self::UpcastingFailed { .. }
            | Self::BlobPublicationFailed { .. } => ReplayErrorKind::Deterministic,
        }
    }
}
