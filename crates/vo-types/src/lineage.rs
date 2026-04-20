//! Workflow lineage and epoch types for continue-as-new (ADR-038).
//!
//! These types track workflow identity across epoch rollover boundaries:
//! - [`Epoch`] identifies one execution epoch within a lineage
//! - [`LineageId`] is a validated string identifying a workflow lineage
//! - [`WorkflowLineage`] binds a stable lineage_id to an epoch with optional parent
//! - [`LineageError`] enumerates construction failures

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ParseError;

/// Newtype wrapper for a monotonically increasing epoch counter.
///
/// Epoch 0 is the initial epoch. Each continue-as-new rollover increments
/// the epoch. Epochs are strictly ordered and comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Epoch(pub u64);

impl Epoch {
    /// Epoch 0 — the initial epoch of any lineage.
    pub const ZERO: Self = Epoch(0);

    /// Create a new epoch from a raw u64 value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Epoch(value)
    }
}

impl std::fmt::Display for Epoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Newtype wrapper for a validated lineage identifier string.
///
/// A lineage ID is a stable identifier that persists across epoch rollovers.
/// It must be non-empty and contain no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LineageId(pub(crate) String);

impl LineageId {
    /// Parse a `LineageId` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Empty` if the lineage_id is empty or whitespace-only.
    /// Returns `ParseError::InvalidCharacters` if the lineage_id contains control characters.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "LineageId";
        if input.trim().is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.chars().any(|c| c.is_control()) {
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: input
                    .chars()
                    .filter(|c| c.is_control())
                    .take(10)
                    .collect::<String>(),
            });
        }
        Ok(Self(input.to_string()))
    }

    /// Returns the inner string value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LineageId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl TryFrom<&str> for LineageId {
    type Error = ParseError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<LineageId> for String {
    fn from(value: LineageId) -> String {
        value.0
    }
}

impl std::fmt::Display for LineageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Immutable workflow lineage across continue-as-new boundaries.
///
/// A lineage binds a stable `lineage_id` (which persists across epoch rollovers)
/// to a specific `epoch` and optional `parent_epoch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowLineage {
    /// Stable identifier for the logical long-lived workflow.
    pub lineage_id: LineageId,
    /// Current epoch within this lineage.
    pub epoch: Epoch,
    /// Previous epoch, present when this lineage was created via continue-as-new.
    pub parent_epoch: Option<Epoch>,
}

impl WorkflowLineage {
    /// Create a root lineage (epoch 0, no parent).
    ///
    /// # Errors
    ///
    /// Returns [`LineageError::EmptyLineageId`] if `lineage_id` is empty or whitespace-only.
    /// Returns [`LineageError::ControlCharacters`] if `lineage_id` contains control characters.
    pub fn new(lineage_id: impl TryInto<LineageId>) -> Result<Self, LineageError> {
        let lineage_id = lineage_id
            .try_into()
            .map_err(|_| LineageError::EmptyLineageId)?;
        Ok(Self {
            lineage_id,
            epoch: Epoch::ZERO,
            parent_epoch: None,
        })
    }

    /// Create a lineage with an explicit epoch and optional parent.
    ///
    /// # Errors
    ///
    /// - Returns [`LineageError::EmptyLineageId`] if `lineage_id` is empty or whitespace-only.
    /// - Returns [`LineageError::InvalidEpochTransition`] if `parent_epoch >= epoch`.
    pub fn with_parent(
        lineage_id: impl TryInto<LineageId>,
        epoch: Epoch,
        parent_epoch: Option<Epoch>,
    ) -> Result<Self, LineageError> {
        let lineage_id = lineage_id
            .try_into()
            .map_err(|_| LineageError::EmptyLineageId)?;
        if let Some(parent) = parent_epoch {
            if parent >= epoch {
                return Err(LineageError::InvalidEpochTransition {
                    parent_epoch: parent.0,
                    epoch: epoch.0,
                });
            }
        }
        Ok(Self {
            lineage_id,
            epoch,
            parent_epoch,
        })
    }

    /// Create a new epoch via continue-as-new rollover.
    ///
    /// Atomically:
    /// 1. writes `ContinuedAsNew` marker for the old epoch
    /// 2. creates a new lineage with epoch = current epoch + 1
    /// 3. carries forward the lineage_id
    /// 4. sets parent_epoch to the current epoch
    ///
    /// # Errors
    ///
    /// Returns [`LineageError::EpochOverflow`] if the current epoch is already `u64::MAX`.
    pub fn continue_as_new(&self) -> Result<Self, LineageError> {
        let next_epoch_value = self
            .epoch
            .0
            .checked_add(1)
            .ok_or(LineageError::EpochOverflow)?;
        Self::with_parent(
            self.lineage_id.clone(),
            Epoch::new(next_epoch_value),
            Some(self.epoch),
        )
    }
}

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
    /// The lineage identity and epoch information.
    pub lineage: WorkflowLineage,
    /// The current status of the lineage.
    pub status: LineageStatus,
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
        self.lineage.lineage_id.as_str()
    }

    /// Returns the current epoch.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.lineage.epoch
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

/// Errors that can occur when constructing lineage values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LineageError {
    #[error("lineage_id must not be empty")]
    EmptyLineageId,
    #[error("parent_epoch ({parent_epoch}) must be less than epoch ({epoch})")]
    InvalidEpochTransition { parent_epoch: u64, epoch: u64 },
    #[error("epoch overflow: cannot advance beyond u64::MAX")]
    EpochOverflow,
    #[error("lineage_id must not contain control characters")]
    ControlCharacters,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Epoch newtype tests
    // -----------------------------------------------------------------------

    #[test]
    fn epoch_new_returns_expected_value() {
        let epoch = Epoch::new(42);
        assert_eq!(epoch.0, 42);
    }

    #[test]
    fn epoch_zero_is_zero() {
        assert_eq!(Epoch::ZERO.0, 0);
    }

    #[test]
    fn epoch_ord_is_consistent_with_u64() {
        let e0 = Epoch::new(0);
        let e1 = Epoch::new(1);
        let e10 = Epoch::new(10);
        assert!(e0 < e1);
        assert!(e1 < e10);
        assert!(e0 < e10);
    }

    #[test]
    fn epoch_partial_ord_ordering() {
        let e1 = Epoch::new(5);
        let e2 = Epoch::new(5);
        let e3 = Epoch::new(10);
        assert_eq!(e1, e2);
        assert!(e3 > e1);
    }

    #[test]
    fn epoch_new_with_u64_max() {
        let epoch = Epoch::new(u64::MAX);
        assert_eq!(epoch.0, u64::MAX);
    }

    #[test]
    fn epoch_new_with_zero() {
        let epoch = Epoch::new(0);
        assert_eq!(epoch.0, 0);
        assert_eq!(epoch, Epoch::ZERO);
    }

    #[test]
    fn epoch_display_shows_value() {
        assert_eq!(Epoch::new(42).to_string(), "42");
        assert_eq!(Epoch::ZERO.to_string(), "0");
    }

    // -----------------------------------------------------------------------
    // WorkflowLineage happy path tests
    // -----------------------------------------------------------------------

    #[test]
    fn lineage_new_creates_root_with_epoch_zero_and_no_parent() {
        let lineage = WorkflowLineage::new("wf-abc-123".to_string()).expect("should succeed");
        assert_eq!(lineage.lineage_id.as_str(), "wf-abc-123");
        assert_eq!(lineage.epoch, Epoch::ZERO);
        assert_eq!(lineage.parent_epoch, None);
    }

    #[test]
    fn lineage_new_with_valid_id_returns_ok() {
        let lineage = WorkflowLineage::new("lineage-root".to_string()).expect("should succeed");
        assert_eq!(lineage.lineage_id.as_str(), "lineage-root");
    }

    #[test]
    fn lineage_with_parent_creates_child_epoch() {
        let lineage =
            WorkflowLineage::with_parent("lin-1".to_string(), Epoch::new(2), Some(Epoch::new(1)))
                .expect("should succeed");
        assert_eq!(lineage.lineage_id.as_str(), "lin-1");
        assert_eq!(lineage.epoch, Epoch::new(2));
        assert_eq!(lineage.parent_epoch, Some(Epoch::new(1)));
    }

    #[test]
    fn lineage_with_parent_none_creates_root_with_explicit_epoch() {
        let lineage = WorkflowLineage::with_parent("lin-2".to_string(), Epoch::new(0), None)
            .expect("should succeed");
        assert_eq!(lineage.epoch, Epoch::new(0));
        assert_eq!(lineage.parent_epoch, None);
    }

    #[test]
    fn lineage_serializes_to_json() {
        let lineage =
            WorkflowLineage::with_parent("lin-1".to_string(), Epoch::new(1), Some(Epoch::new(0)))
                .expect("should succeed");
        let json = serde_json::to_value(&lineage).expect("serialization should succeed");
        assert_eq!(json["lineage_id"], "lin-1");
        assert_eq!(json["epoch"], 1);
        assert_eq!(json["parent_epoch"], 0);
    }

    #[test]
    fn lineage_serializes_to_json_without_parent() {
        let lineage = WorkflowLineage::new("lin-root".to_string()).expect("should succeed");
        let json = serde_json::to_value(&lineage).expect("serialization should succeed");
        assert_eq!(json["lineage_id"], "lin-root");
        assert_eq!(json["epoch"], 0);
        assert_eq!(json["parent_epoch"], serde_json::Value::Null);
    }

    #[test]
    fn lineage_deserializes_from_json() {
        let json = serde_json::json!({
            "lineage_id": "lin-1",
            "epoch": 3,
            "parent_epoch": 2
        });
        let lineage: WorkflowLineage =
            serde_json::from_value(json).expect("deserialization should succeed");
        assert_eq!(lineage.lineage_id.as_str(), "lin-1");
        assert_eq!(lineage.epoch, Epoch::new(3));
        assert_eq!(lineage.parent_epoch, Some(Epoch::new(2)));
    }

    #[test]
    fn lineage_deserializes_from_json_without_parent() {
        let json = serde_json::json!({
            "lineage_id": "lin-root",
            "epoch": 0
        });
        let lineage: WorkflowLineage =
            serde_json::from_value(json).expect("deserialization should succeed");
        assert_eq!(lineage.parent_epoch, None);
    }

    #[test]
    fn lineage_roundtrip_json_preserves_all_fields() {
        let original = WorkflowLineage::with_parent(
            "lin-roundtrip".to_string(),
            Epoch::new(5),
            Some(Epoch::new(4)),
        )
        .expect("should succeed");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: WorkflowLineage = serde_json::from_value(json).expect("deserialize");
        assert_eq!(original, restored);
    }

    // -----------------------------------------------------------------------
    // WorkflowLineage error path tests
    // -----------------------------------------------------------------------

    #[test]
    fn lineage_new_returns_empty_lineage_id_when_id_is_empty() {
        let result = WorkflowLineage::new(String::new());
        assert_eq!(result, Err(LineageError::EmptyLineageId));
    }

    #[test]
    fn lineage_new_returns_empty_lineage_id_when_id_is_whitespace_only() {
        let result = WorkflowLineage::new("   ".to_string());
        assert_eq!(result, Err(LineageError::EmptyLineageId));
    }

    #[test]
    fn lineage_with_parent_returns_empty_lineage_id_when_id_is_empty() {
        let result =
            WorkflowLineage::with_parent(String::new(), Epoch::new(1), Some(Epoch::new(0)));
        assert_eq!(result, Err(LineageError::EmptyLineageId));
    }

    #[test]
    fn lineage_with_parent_returns_invalid_epoch_transition_when_parent_equals_epoch() {
        let result =
            WorkflowLineage::with_parent("lin-1".to_string(), Epoch::new(3), Some(Epoch::new(3)));
        assert_eq!(
            result,
            Err(LineageError::InvalidEpochTransition {
                parent_epoch: 3,
                epoch: 3
            })
        );
    }

    #[test]
    fn lineage_with_parent_returns_invalid_epoch_transition_when_parent_exceeds_epoch() {
        let result =
            WorkflowLineage::with_parent("lin-1".to_string(), Epoch::new(1), Some(Epoch::new(5)));
        assert_eq!(
            result,
            Err(LineageError::InvalidEpochTransition {
                parent_epoch: 5,
                epoch: 1
            })
        );
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn lineage_with_parent_epoch_1_parent_epoch_0() {
        let lineage = WorkflowLineage::with_parent(
            "lin-edge".to_string(),
            Epoch::new(1),
            Some(Epoch::new(0)),
        )
        .expect("should succeed");
        assert_eq!(lineage.epoch, Epoch::new(1));
        assert_eq!(lineage.parent_epoch, Some(Epoch::new(0)));
    }

    #[test]
    fn lineage_with_large_epoch_values() {
        let lineage = WorkflowLineage::with_parent(
            "lin-large".to_string(),
            Epoch::new(u64::MAX),
            Some(Epoch::new(u64::MAX - 1)),
        )
        .expect("should succeed");
        assert_eq!(lineage.epoch, Epoch::new(u64::MAX));
        assert_eq!(lineage.parent_epoch, Some(Epoch::new(u64::MAX - 1)));
    }

    // -----------------------------------------------------------------------
    // Contract verification / invariant tests
    // -----------------------------------------------------------------------

    #[test]
    fn invariant_epoch_monotonic_parent_less_than_epoch() {
        for epoch_val in 1..100u64 {
            let result = WorkflowLineage::with_parent(
                "inv".to_string(),
                Epoch::new(epoch_val),
                Some(Epoch::new(epoch_val)),
            );
            assert_eq!(
                result,
                Err(LineageError::InvalidEpochTransition {
                    parent_epoch: epoch_val,
                    epoch: epoch_val
                })
            );
        }
    }

    #[test]
    fn invariant_lineage_id_never_empty() {
        let lineage = WorkflowLineage::new("non-empty".to_string()).expect("ok");
        assert!(!lineage.lineage_id.0.is_empty());
    }

    #[test]
    fn invariant_epoch_zero_has_no_parent() {
        let lineage = WorkflowLineage::new("root".to_string()).expect("ok");
        assert_eq!(lineage.epoch, Epoch::ZERO);
        assert_eq!(lineage.parent_epoch, None);
    }

    // -----------------------------------------------------------------------
    // LineageError display tests
    // -----------------------------------------------------------------------

    #[test]
    fn lineage_error_empty_lineage_id_displays_correctly() {
        let err = LineageError::EmptyLineageId;
        assert_eq!(err.to_string(), "lineage_id must not be empty");
    }

    #[test]
    fn lineage_error_invalid_epoch_transition_displays_correctly() {
        let err = LineageError::InvalidEpochTransition {
            parent_epoch: 5,
            epoch: 3,
        };
        assert_eq!(
            err.to_string(),
            "parent_epoch (5) must be less than epoch (3)"
        );
    }

    // -----------------------------------------------------------------------
    // LineageStatus tests (ADR-042 Section 5)
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // LineageState tests (ADR-042 Section 5)
    // -----------------------------------------------------------------------

    #[test]
    fn lineage_state_new_is_active() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::new(lineage);
        assert_eq!(state.status, LineageStatus::Active);
        assert!(state.can_spawn_epoch());
    }

    #[test]
    fn lineage_state_with_status() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::with_status(lineage, LineageStatus::Tombstoned);
        assert_eq!(state.status, LineageStatus::Tombstoned);
        assert!(!state.can_spawn_epoch());
    }

    #[test]
    fn lineage_state_tombstone() {
        let lineage = WorkflowLineage::new("test-lineage".to_string()).expect("ok");
        let state = LineageState::new(lineage);
        assert!(state.can_spawn_epoch());

        let tombstoned = state.tombstone();
        assert_eq!(tombstoned.status, LineageStatus::Tombstoned);
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
        assert_eq!(restored.status, LineageStatus::Active);
    }

    #[test]
    fn lineage_state_tombstoned_serde_roundtrip() {
        let lineage = WorkflowLineage::new("tombstoned-test".to_string()).expect("ok");
        let state = LineageState::with_status(lineage, LineageStatus::Tombstoned);

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: LineageState = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(restored.lineage_id(), "tombstoned-test");
        assert_eq!(restored.status, LineageStatus::Tombstoned);
    }
}
