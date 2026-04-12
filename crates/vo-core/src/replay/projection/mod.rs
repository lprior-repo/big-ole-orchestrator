//! Projection engine types (ADR-037).
//!
//! Types for the event sourcing projection engine — transforms immutable
//! event sequences into materialized read models (projections).

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod projector_tests;
#[cfg(test)]
mod state_machine_tests;
#[cfg(test)]
mod result_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod build_protocol_tests;
#[cfg(test)]
mod incremental_tests;
#[cfg(test)]
mod self_healing_tests;
#[cfg(test)]
mod observability_tests;
#[cfg(test)]
mod invariant_tests;
#[cfg(test)]
mod proptest;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod edge_case_tests;
#[cfg(test)]
mod serde_tests;
#[cfg(test)]
mod error_taxonomy_tests;
#[cfg(test)]
mod display_tests;

pub mod error;

pub use error::{
    ProjectionError, ProjectionStateError, ProjectionVersionError, ReplayError, StorageError,
};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionState {
    Building,
    Ready,
    Stale { detected_at: u64, reason: StaleReason },
    Rebuilding { progress: u32, from_sequence: u64 },
    Failed { reason: String, attempted_at: u64 },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaleReason {
    SchemaVersionMismatch { expected: u8, actual: u8 },
    SequenceGapDetected { gap_at: u64 },
    CorruptionDetected,
    ManualInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

pub trait Projector<S, E>
where
    S: Clone + Default + serde::Serialize,
{
    type Error: Into<ProjectionError>;

    fn project(&self, state: S, event: &E) -> Result<S, Self::Error>;

    fn initial_state() -> S;

    fn schema_version(&self) -> u8;
}

pub struct ProjectionEngine {
    upcaster_registry: Option<Box<dyn crate::upcaster::UpcasterRegistry>>,
    compatibility_window: vo_storage::projection_compat::ProjectionCompatibilityWindow,
}

impl ProjectionEngine {
    pub fn new(
        compatibility_window: vo_storage::projection_compat::ProjectionCompatibilityWindow,
    ) -> Self {
        Self {
            upcaster_registry: None,
            compatibility_window,
        }
    }

    pub fn with_upcaster(
        mut self,
        registry: Box<dyn crate::upcaster::UpcasterRegistry>,
    ) -> Self {
        self.upcaster_registry = Some(registry);
        self
    }

    pub fn compatibility_window(&self) -> vo_storage::projection_compat::ProjectionCompatibilityWindow {
        self.compatibility_window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_state_is_terminal() {
        assert!(!ProjectionState::Building.is_terminal());
        assert!(!ProjectionState::Ready.is_terminal());
        assert!(!ProjectionState::Stale {
            detected_at: 0,
            reason: StaleReason::ManualInvalidation
        }
        .is_terminal());
        assert!(!ProjectionState::Rebuilding {
            progress: 50,
            from_sequence: 1
        }
        .is_terminal());
        assert!(ProjectionState::Failed {
            reason: "test".to_string(),
            attempted_at: 100
        }
        .is_terminal());
    }

    #[test]
    fn projection_state_is_stale() {
        assert!(!ProjectionState::Building.is_stale());
        assert!(!ProjectionState::Ready.is_stale());
        assert!(ProjectionState::Stale {
            detected_at: 0,
            reason: StaleReason::ManualInvalidation
        }
        .is_stale());
        assert!(ProjectionState::Rebuilding {
            progress: 50,
            from_sequence: 1
        }
        .is_stale());
        assert!(!ProjectionState::Failed {
            reason: "test".to_string(),
            attempted_at: 100
        }
        .is_stale());
    }

    #[test]
    fn stale_reason_variants() {
        use StaleReason::*;
        let reasons = vec![
            SchemaVersionMismatch {
                expected: 1,
                actual: 0,
            },
            SequenceGapDetected { gap_at: 100 },
            CorruptionDetected,
            ManualInvalidation,
        ];
        for reason in reasons {
            let debug = format!("{:?}", reason);
            assert!(!debug.is_empty());
        }
    }

    #[test]
    fn projection_record_construction() {
        let record = ProjectionRecord::new(
            "test-projection".to_string(),
            1,
            vec![1, 2, 3],
            (1, 100),
            12345,
            1000,
            2000,
        );
        assert_eq!(record.projection_id, "test-projection");
        assert_eq!(record.schema_version, 1);
        assert_eq!(record.state_bytes, vec![1, 2, 3]);
        assert_eq!(record.sequence_range, (1, 100));
        assert_eq!(record.checksum, 12345);
        assert_eq!(record.created_at, 1000);
        assert_eq!(record.updated_at, 2000);
    }

    #[test]
    fn projection_result_construction() {
        let result: ProjectionResult<String> = ProjectionResult::new(
            "final state".to_string(),
            50,
            1,
            50,
            100,
            1,
        );
        assert_eq!(result.state, "final state");
        assert_eq!(result.events_applied, 50);
        assert_eq!(result.starting_sequence, 1);
        assert_eq!(result.ending_sequence, 50);
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.schema_version, 1);
    }
}
