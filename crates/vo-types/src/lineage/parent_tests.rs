//! Tests for Epoch and WorkflowLineage types.

use super::{Epoch, LineageError, WorkflowLineage};

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
    let result =
        WorkflowLineage::with_parent("lineage\x1fwith control".to_string(), Epoch::new(1), None);
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