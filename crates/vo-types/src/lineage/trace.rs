//! Lineage lifecycle status and state tracking (ADR-042 Section 5).

use serde::{Deserialize, Serialize};

use crate::lineage::parent::{Epoch, WorkflowLineage};

/// Status of a lineage - tracks whether it can accept new epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineageStatus {
    /// Lineage is active and can spawn new epochs.
    Active,
    /// Lineage has been permanently tombstoned - no more epochs allowed.
    Tombstoned,
}

impl LineageStatus {
    /// Returns `true` if the lineage is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns `true` if the lineage is tombstoned.
    #[must_use]
    pub const fn is_tombstoned(&self) -> bool {
        matches!(self, Self::Tombstoned)
    }
}

/// Lineage state combines lineage identity with operational status.
///
/// Tracks the full lifecycle state of a lineage including whether
/// it has been permanently tombstoned due to a lineage-scoped failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageState {
    lineage: WorkflowLineage,
    status: LineageStatus,
}

impl LineageState {
    /// Create a new active lineage state.
    #[must_use]
    pub fn new(lineage: WorkflowLineage) -> Self {
        Self {
            lineage,
            status: LineageStatus::Active,
        }
    }

    /// Create a lineage state with explicit status.
    #[must_use]
    pub fn with_status(lineage: WorkflowLineage, status: LineageStatus) -> Self {
        Self { lineage, status }
    }

    /// Returns `true` if this lineage can spawn new epochs.
    #[must_use]
    pub fn can_spawn_epoch(&self) -> bool {
        self.status == LineageStatus::Active
    }

    /// Returns the lineage_id.
    #[must_use]
    pub fn lineage_id(&self) -> &str {
        self.lineage.lineage_id()
    }

    /// Returns the current epoch.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.lineage.epoch()
    }

    /// Returns the lineage identity.
    #[must_use]
    pub fn lineage(&self) -> &WorkflowLineage {
        &self.lineage
    }

    /// Returns the current status.
    #[must_use]
    pub fn status(&self) -> LineageStatus {
        self.status
    }

    /// Tombstone this lineage, permanently preventing new epochs.
    #[must_use]
    pub fn tombstone(&self) -> Self {
        Self {
            lineage: self.lineage.clone(),
            status: LineageStatus::Tombstoned,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_status_active_is_active() {
        assert!(LineageStatus::Active.is_active());
        assert!(!LineageStatus::Active.is_tombstoned());
    }

    #[test]
    fn lineage_status_tombstoned_is_tombstoned() {
        assert!(!LineageStatus::Tombstoned.is_active());
        assert!(LineageStatus::Tombstoned.is_tombstoned());
    }

    #[test]
    fn lineage_status_debug() {
        assert_eq!(format!("{:?}", LineageStatus::Active), "Active");
        assert_eq!(format!("{:?}", LineageStatus::Tombstoned), "Tombstoned");
    }

    #[test]
    fn lineage_state_new_is_active() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::new(lineage);
        assert_eq!(state.status(), LineageStatus::Active);
        assert!(state.can_spawn_epoch());
    }

    #[test]
    fn lineage_state_with_status() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::with_status(lineage, LineageStatus::Tombstoned);
        assert_eq!(state.status(), LineageStatus::Tombstoned);
        assert!(!state.can_spawn_epoch());
    }

    #[test]
    fn lineage_state_tombstone() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::new(lineage);
        assert!(state.can_spawn_epoch());

        let tombstoned = state.tombstone();
        assert_eq!(tombstoned.status(), LineageStatus::Tombstoned);
        assert!(!tombstoned.can_spawn_epoch());
        assert_eq!(tombstoned.lineage_id(), "test-lineage");
    }

    #[test]
    fn lineage_state_epoch_accessors() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::new(lineage.clone());
        assert_eq!(state.epoch(), Epoch::ZERO);

        let child_lineage = lineage.continue_as_new().expect("ok");
        let child_state = LineageState::new(child_lineage);
        assert_eq!(child_state.epoch(), Epoch::new(1));
    }

    #[test]
    fn lineage_state_serde_roundtrip() {
        let lineage = WorkflowLineage::new("serde-test".to_string()).expect("ok");
        let state = LineageState::new(lineage);

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: LineageState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.lineage_id(), "serde-test");
        assert_eq!(restored.status(), LineageStatus::Active);
    }

    #[test]
    fn lineage_state_tombstoned_serde_roundtrip() {
        let lineage = WorkflowLineage::new("tombstoned-test".to_string()).expect("ok");
        let state = LineageState::with_status(lineage, LineageStatus::Tombstoned);

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: LineageState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.lineage_id(), "tombstoned-test");
        assert_eq!(restored.status(), LineageStatus::Tombstoned);
    }
}
