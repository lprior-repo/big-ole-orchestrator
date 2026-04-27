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
