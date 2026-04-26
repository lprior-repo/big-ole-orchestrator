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
    /// Latest fence token observed from StepScheduled events during replay.
    /// This is the fence that should be restored after restart/recovery.
    /// `None` if no StepScheduled events were replayed.
    pub latest_fence: Option<u64>,
}

/// Errors that can occur during event replay.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        first_at_index: usize,
        second_at_index: usize,
    },
    PayloadDecodeFailed {
        sequence: u64,
        source: String,
    },
    TransitionFailed {
        sequence: u64,
        state: LifecycleState,
        reason: String,
    },
    UnexpectedEventType {
        payload_type: String,
        sequence: u64,
    },
    UpcastingFailed {
        sequence: u64,
        reason: String,
    },
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
            Self::InstanceMismatch { expected, actual } => {
                write!(
                    f,
                    "Instance ID mismatch: expected '{expected}', got '{actual}'"
                )
            }
            Self::SequenceGap {
                expected,
                actual,
                at_index,
            } => {
                write!(
                    f,
                    "Sequence gap at index {at_index}: expected {expected}, got {actual}"
                )
            }
            Self::SequenceDuplicate {
                sequence,
                first_at_index,
                second_at_index,
            } => {
                write!(f, "Duplicate sequence {sequence} at indices {first_at_index} and {second_at_index}")
            }
            Self::PayloadDecodeFailed { sequence, source } => {
                write!(f, "Payload decode failed at sequence {sequence}: {source}")
            }
            Self::TransitionFailed {
                sequence,
                state,
                reason,
            } => {
                write!(
                    f,
                    "Transition failed at sequence {sequence} in state {state:?}: {reason}"
                )
            }
            Self::UnexpectedEventType {
                payload_type,
                sequence,
            } => {
                write!(
                    f,
                    "Unexpected event type '{payload_type}' at sequence {sequence}"
                )
            }
            Self::UpcastingFailed { sequence, reason } => {
                write!(f, "Upcasting failed at sequence {sequence}: {reason}")
            }
            Self::BlobPublicationFailed {
                sequence,
                step_id,
                blob_id,
            } => {
                write!(f, "Blob publication failed at sequence {sequence} for step '{step_id}': blob {blob_id}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

impl ReplayError {
    #[must_use]
    pub fn kind(&self) -> ReplayErrorKind {
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
