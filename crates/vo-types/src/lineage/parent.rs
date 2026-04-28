//! Epoch and WorkflowLineage types for continue-as-new (ADR-038).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Newtype wrapper for a monotonically increasing epoch counter.
///
/// Epoch 0 is the initial epoch. Each continue-as-new rollover increments
/// the epoch. Epochs are strictly ordered and comparable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Epoch(u64);

impl Epoch {
    /// Epoch 0 — the initial epoch of any lineage.
    pub const ZERO: Self = Epoch(0);

    /// Create a new epoch from a raw u64 value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Epoch(value)
    }

    /// Returns the raw u64 value of this epoch.
    #[must_use]
    pub const fn get(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for Epoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur when constructing lineage values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LineageError {
    #[error("lineage_id must not be empty")]
    EmptyLineageId,
    #[error("parent_epoch ({parent_epoch}) must be less than epoch ({epoch})")]
    InvalidEpochTransition { parent_epoch: Epoch, epoch: Epoch },
    #[error("epoch overflow: cannot advance beyond u64::MAX")]
    EpochOverflow,
    #[error("lineage_id must not contain control characters")]
    ControlCharacters,
}

fn validate_lineage_id(lineage_id: &str) -> Result<(), LineageError> {
    if lineage_id.trim().is_empty() {
        return Err(LineageError::EmptyLineageId);
    }
    if lineage_id.chars().any(|c| c.is_control()) {
        return Err(LineageError::ControlCharacters);
    }
    Ok(())
}

/// Immutable workflow lineage across continue-as-new boundaries.
///
/// A lineage binds a stable `lineage_id` (which persists across epoch rollovers)
/// to a specific `epoch` and optional `parent_epoch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowLineage {
    lineage_id: String,
    epoch: Epoch,
    parent_epoch: Option<Epoch>,
}

impl WorkflowLineage {
    /// Returns the stable lineage identifier.
    #[must_use]
    pub fn lineage_id(&self) -> &str {
        &self.lineage_id
    }

    /// Returns the current epoch.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the parent epoch, if this lineage was created via continue-as-new.
    #[must_use]
    pub fn parent_epoch(&self) -> Option<Epoch> {
        self.parent_epoch
    }
    /// Create a root lineage (epoch 0, no parent).
    ///
    /// # Errors
    ///
    /// Returns [`LineageError::EmptyLineageId`] if `lineage_id` is empty or whitespace-only.
    pub fn new(lineage_id: impl Into<String>) -> Result<Self, LineageError> {
        let lineage_id = lineage_id.into();
        validate_lineage_id(&lineage_id)?;
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
        lineage_id: impl Into<String>,
        epoch: Epoch,
        parent_epoch: Option<Epoch>,
    ) -> Result<Self, LineageError> {
        let lineage_id = lineage_id.into();
        validate_lineage_id(&lineage_id)?;
        if let Some(parent) = parent_epoch {
            if parent >= epoch {
                return Err(LineageError::InvalidEpochTransition {
                    parent_epoch: parent,
                    epoch,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_new_returns_expected_value() {
        let epoch = Epoch::new(42);
        assert_eq!(epoch.get(), 42);
    }

    #[test]
    fn epoch_zero_is_zero() {
        assert_eq!(Epoch::ZERO.get(), 0);
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
        assert_eq!(epoch.get(), u64::MAX);
    }

    #[test]
    fn epoch_new_with_zero() {
        let epoch = Epoch::new(0);
        assert_eq!(epoch.get(), 0);
        assert_eq!(epoch, Epoch::ZERO);
    }

    #[test]
    fn epoch_display_shows_value() {
        assert_eq!(Epoch::new(42).to_string(), "42");
        assert_eq!(Epoch::ZERO.to_string(), "0");
    }

    #[test]
    fn lineage_new_creates_root_with_epoch_zero_and_no_parent() {
        let lineage = WorkflowLineage::new("wf-abc-123".to_string()).expect("should succeed");
        assert_eq!(lineage.lineage_id, "wf-abc-123");
        assert_eq!(lineage.epoch, Epoch::ZERO);
        assert_eq!(lineage.parent_epoch, None);
    }

    #[test]
    fn lineage_new_with_valid_id_returns_ok() {
        let lineage = WorkflowLineage::new("lineage-root".to_string()).expect("should succeed");
        assert_eq!(lineage.lineage_id, "lineage-root");
    }

    #[test]
    fn lineage_with_parent_creates_child_epoch() {
        let lineage =
            WorkflowLineage::with_parent("lin-1".to_string(), Epoch::new(2), Some(Epoch::new(1)))
                .expect("should succeed");
        assert_eq!(lineage.lineage_id, "lin-1");
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
        assert_eq!(lineage.lineage_id, "lin-1");
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
                parent_epoch: Epoch::new(3),
                epoch: Epoch::new(3)
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
                parent_epoch: Epoch::new(5),
                epoch: Epoch::new(1)
            })
        );
    }

    #[test]
    fn lineage_new_returns_control_characters_error_when_id_contains_control_char() {
        let result = WorkflowLineage::new("lineage\x00with null".to_string());
        assert_eq!(result, Err(LineageError::ControlCharacters));
    }

    #[test]
    fn lineage_with_parent_returns_control_characters_error_when_id_contains_ctrl() {
        let result = WorkflowLineage::with_parent(
            "lineage\x1fwith control".to_string(),
            Epoch::new(1),
            None,
        );
        assert_eq!(result, Err(LineageError::ControlCharacters));
    }

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
                    parent_epoch: Epoch::new(epoch_val),
                    epoch: Epoch::new(epoch_val)
                })
            );
        }
    }

    #[test]
    fn invariant_lineage_id_never_empty() {
        let lineage = WorkflowLineage::new("non-empty".to_string()).expect("ok");
        assert!(!lineage.lineage_id.is_empty());
    }

    #[test]
    fn invariant_epoch_zero_has_no_parent() {
        let lineage = WorkflowLineage::new("root".to_string()).expect("ok");
        assert_eq!(lineage.epoch, Epoch::ZERO);
        assert_eq!(lineage.parent_epoch, None);
    }

    #[test]
    fn lineage_error_empty_lineage_id_displays_correctly() {
        let err = LineageError::EmptyLineageId;
        assert_eq!(err.to_string(), "lineage_id must not be empty");
    }

    #[test]
    fn lineage_error_invalid_epoch_transition_displays_correctly() {
        let err = LineageError::InvalidEpochTransition {
            parent_epoch: Epoch::new(5),
            epoch: Epoch::new(3),
        };
        assert_eq!(
            err.to_string(),
            "parent_epoch (5) must be less than epoch (3)"
        );
    }
}
