//! Projection domain types and traits (ADR-037).
//!
//! Core data types for the event sourcing projection engine — transforms
//! immutable event sequences into materialized read models.

use serde::{Deserialize, Serialize};

// =====================================================================
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionRecord {
    pub projection_id: String,
    pub schema_version: u8,
    pub state_bytes: Vec<u8>,
    pub sequence_range: (u64, u64),
    pub checksum: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

impl ProjectionRecord {
    pub fn new(
        projection_id: String,
        schema_version: u8,
        state_bytes: Vec<u8>,
        sequence_range: (u64, u64),
        checksum: u64,
        created_at: u64,
        updated_at: u64,
    ) -> Self {
        Self {
            projection_id,
            schema_version,
            state_bytes,
            sequence_range,
            checksum,
            created_at,
            updated_at,
        }
    }
}

// =====================================================================#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionResult<S> {
    pub state: S,
    pub events_applied: u64,
    pub starting_sequence: u64,
    pub ending_sequence: u64,
    pub duration_ms: u64,
    pub schema_version: u8,
}

impl<S> ProjectionResult<S> {
    pub fn new(
        state: S,
        events_applied: u64,
        starting_sequence: u64,
        ending_sequence: u64,
        duration_ms: u64,
        schema_version: u8,
    ) -> Self {
        Self {
            state,
            events_applied,
            starting_sequence,
            ending_sequence,
            duration_ms,
            schema_version,
        }
    }
}

// =====================================================================#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionState {
    Building,
    Ready,
    Stale {
        detected_at: u64,
        reason: StaleReason,
    },
    Rebuilding {
        progress: u32,
        from_sequence: u64,
    },
    Failed {
        reason: String,
        attempted_at: u64,
    },
}

impl ProjectionState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, ProjectionState::Failed { .. })
    }

    pub fn is_stale(&self) -> bool {
        matches!(
            self,
            ProjectionState::Stale { .. } | ProjectionState::Rebuilding { .. }
        )
    }
}

// =====================================================================#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaleReason {
    SchemaVersionMismatch { expected: u8, actual: u8 },
    SequenceGapDetected { gap_at: u64 },
    CorruptionDetected,
    ManualInvalidation,
}

// =====================================================================#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionEvent {
    ProjectionStarted {
        projection_id: String,
        from_sequence: u64,
    },
    ProjectionProgress {
        projection_id: String,
        percent: u32,
        at_sequence: u64,
    },
    ProjectionCompleted {
        projection_id: String,
        events_applied: u64,
    },
    ProjectionStale {
        projection_id: String,
        reason: StaleReason,
    },
    ProjectionRebuildStarted {
        projection_id: String,
        reason: StaleReason,
    },
    ProjectionRebuildFailed {
        projection_id: String,
        error: String,
    },
}

// =====================================================================pub trait Projector<S, E>
where
    S: Clone + Default + serde::Serialize,
{
    type Error: Into<super::ProjectionError>;

    fn project(&self, state: S, event: &E) -> Result<S, Self::Error>;

    fn initial_state() -> S;

    fn schema_version(&self) -> u8;
}
