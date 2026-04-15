//! Result and error types for the replay engine.

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// Events have different instance_ids.
    InstanceMismatch { expected: String, actual: String },
    /// Sequence numbers have a gap.
    SequenceGap {
        expected: u64,
        actual: u64,
        at_index: usize,
    },
    /// Duplicate sequence number found.
    SequenceDuplicate {
        sequence: u64,
        first_at_index: usize,
        second_at_index: usize,
    },
    /// Event payload could not be decoded.
    PayloadDecodeFailed { sequence: u64, source: String },
    /// State machine rejected a transition.
    TransitionFailed {
        sequence: u64,
        state: LifecycleState,
        reason: String,
    },
    /// Event payload variant has no mapping to a TransitionEvent.
    UnexpectedEventType { payload_type: String, sequence: u64 },
    /// Upcasting failed during replay_with_upcaster.
    UpcastingFailed { sequence: u64, reason: String },
    /// Blob publication failed for a required output (ADR-040 §3).
    /// The step stays incomplete and may be retried or failed.
    BlobPublicationFailed {
        sequence: u64,
        step_id: String,
        blob_id: String,
    },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::InstanceMismatch { expected, actual } => {
                write!(
                    f,
                    "Instance ID mismatch: expected '{expected}', got '{actual}'"
                )
            }
            ReplayError::SequenceGap {
                expected,
                actual,
                at_index,
            } => {
                write!(
                    f,
                    "Sequence gap at index {at_index}: expected {expected}, got {actual}"
                )
            }
            ReplayError::SequenceDuplicate {
                sequence,
                first_at_index,
                second_at_index,
            } => {
                write!(
                    f,
                    "Duplicate sequence {sequence} at indices {first_at_index} and {second_at_index}"
                )
            }
            ReplayError::PayloadDecodeFailed { sequence, source } => {
                write!(f, "Payload decode failed at sequence {sequence}: {source}")
            }
            ReplayError::TransitionFailed {
                sequence,
                state,
                reason,
            } => {
                write!(
                    f,
                    "Transition failed at sequence {sequence} in state {state:?}: {reason}"
                )
            }
            ReplayError::UnexpectedEventType {
                payload_type,
                sequence,
            } => {
                write!(
                    f,
                    "Unexpected event type '{payload_type}' at sequence {sequence}"
                )
            }
            ReplayError::UpcastingFailed { sequence, reason } => {
                write!(f, "Upcasting failed at sequence {sequence}: {reason}")
            }
            ReplayError::BlobPublicationFailed {
                sequence,
                step_id,
                blob_id,
            } => {
                write!(
                    f,
                    "Blob publication failed for required output at sequence {sequence}: step_id={step_id}, blob_id={blob_id}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

impl ReplayError {
    /// Returns the categorization of this error.
    ///
    /// Deterministic errors indicate permanent state corruption that cannot
    /// be recovered via retry. Transient errors are temporary failures
    /// that the infrastructure should retry.
    #[must_use]
    pub const fn kind(&self) -> ReplayErrorKind {
        match self {
            ReplayError::InstanceMismatch { .. }
            | ReplayError::SequenceGap { .. }
            | ReplayError::SequenceDuplicate { .. }
            | ReplayError::PayloadDecodeFailed { .. }
            | ReplayError::TransitionFailed { .. }
            | ReplayError::UnexpectedEventType { .. }
            | ReplayError::UpcastingFailed { .. } => ReplayErrorKind::Deterministic,
        }
    }
}
