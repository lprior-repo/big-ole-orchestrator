//! Integration tests for WriteClass taxonomy.
//!
//! These tests exercise the public API of WriteClass and WriteBudget
//! through the real serde layer, verifying end-to-end behavior.

use vo_core::write_class::{WriteBudget, WriteClass};

/// Helper to create a budget with standard limits for testing.
fn standard_budget() -> WriteBudget {
    WriteBudget::new(100, 200, 300)
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteClass JSON Serialization Integration Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn write_class_serializes_to_json_when_critical_control_plane() {
    let wc = WriteClass::CriticalControlPlane;
    let json = serde_json::to_string(&wc).unwrap();
    assert_eq!(json, "\"critical_control_plane\"");
}

#[test]
fn write_class_serializes_to_json_when_operator_projection() {
    let wc = WriteClass::OperatorProjection;
    let json = serde_json::to_string(&wc).unwrap();
    assert_eq!(json, "\"operator_projection\"");
}

#[test]
fn write_class_serializes_to_json_when_bulk_blob() {
    let wc = WriteClass::BulkBlob;
    let json = serde_json::to_string(&wc).unwrap();
    assert_eq!(json, "\"bulk_blob\"");
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteClass JSON Deserialization Integration Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn write_class_deserializes_from_json_when_critical_control_plane() {
    let json = "\"critical_control_plane\"";
    let parsed: WriteClass =
        serde_json::from_str(json).expect("critical_control_plane should deserialize");
    assert_eq!(parsed, WriteClass::CriticalControlPlane);
}

#[test]
fn write_class_deserializes_from_json_when_operator_projection() {
    let json = "\"operator_projection\"";
    let parsed: WriteClass =
        serde_json::from_str(json).expect("operator_projection should deserialize");
    assert_eq!(parsed, WriteClass::OperatorProjection);
}

#[test]
fn write_class_deserializes_from_json_when_bulk_blob() {
    let json = "\"bulk_blob\"";
    let parsed: WriteClass = serde_json::from_str(json).expect("bulk_blob should deserialize");
    assert_eq!(parsed, WriteClass::BulkBlob);
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteBudget Integration Tests (multi-operation sequences)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn write_budget_multiple_reserves_across_classes() {
    let budget = standard_budget();

    // Reserve from different classes
    assert_eq!(budget.reserve(WriteClass::CriticalControlPlane, 30), Ok(()));
    assert_eq!(budget.reserve(WriteClass::OperatorProjection, 50), Ok(()));
    assert_eq!(budget.reserve(WriteClass::BulkBlob, 100), Ok(()));

    // Check remaining for each class
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 70);
    assert_eq!(budget.remaining(WriteClass::OperatorProjection), 150);
    assert_eq!(budget.remaining(WriteClass::BulkBlob), 200);
}

#[test]
fn write_budget_exhaustion_isolation_between_classes() {
    let budget = standard_budget();

    // Exhaust critical
    assert_eq!(
        budget.reserve(WriteClass::CriticalControlPlane, 100),
        Ok(())
    );
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);

    // Other classes should be unaffected
    assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
    assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);

    // Can still write to other classes
    assert!(budget.can_write(WriteClass::OperatorProjection, 50));
    assert!(budget.can_write(WriteClass::BulkBlob, 50));

    // Cannot write to exhausted class
    assert!(!budget.can_write(WriteClass::CriticalControlPlane, 1));
}

#[test]
fn write_budget_can_write_at_boundary_after_partial_reserve() {
    let budget = standard_budget();

    // Reserve 50 from critical (leaving 50)
    assert_eq!(budget.reserve(WriteClass::CriticalControlPlane, 50), Ok(()));

    // Can still write exactly 50 more
    assert!(budget.can_write(WriteClass::CriticalControlPlane, 50));

    // But 51 would exceed
    assert!(!budget.can_write(WriteClass::CriticalControlPlane, 51));
}

#[test]
fn write_budget_concurrent_reserve_same_class_isolated() {
    // This tests that each WriteBudget instance maintains its own state
    let budget1 = WriteBudget::new(100, 200, 300);
    let budget2 = WriteBudget::new(100, 200, 300);

    // Reserve from budget1
    assert_eq!(
        budget1.reserve(WriteClass::CriticalControlPlane, 50),
        Ok(())
    );

    // budget2 should be unaffected
    assert_eq!(budget2.remaining(WriteClass::CriticalControlPlane), 100);
    assert!(budget2.can_write(WriteClass::CriticalControlPlane, 100));
}
