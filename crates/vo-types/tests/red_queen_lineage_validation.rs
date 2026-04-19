//! Red Queen adversarial tests for WorkflowLineage control character rejection.
//!
//! These tests verify that lineage_id containing control characters is rejected.
//! Control characters (codepoints U+0000–U+001F) must not appear in lineage_id
//! because they corrupt logs, headers, and wire formats.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::useless_vec, unused_imports, unused_variables)]


use vo_types::{LineageError, WorkflowLineage};

// ===========================================================================
// Gen 2: lineage_id_X_should_be_rejected
// ===========================================================================

#[test]
fn lineage_id_newline_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\nwith-newline".to_string());
    assert!(
        result.is_err(),
        "lineage_id with newline (U+000A) should be rejected, but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_carriage_return_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\rwith-cr".to_string());
    assert!(
        result.is_err(),
        "lineage_id with carriage return (U+000D) should be rejected, but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_tab_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\twith-tab".to_string());
    assert!(
        result.is_err(),
        "lineage_id with tab (U+0009) should be rejected, but was accepted: {:?}",
        result
    );
}

// ===========================================================================
// Decode boundary: decode_X_in_lineage_id_accepted_but_should_not_be
// These test that WorkflowLineage::new rejects control chars that might slip
// through deserialization or string construction.
// ===========================================================================

#[test]
fn decode_newline_in_lineage_id_accepted_but_should_not_be() {
    let result = WorkflowLineage::new("valid-prefix\nsuffix".to_string());
    assert!(
        result.is_err(),
        "RED QUEEN: newline in lineage_id was accepted — control character validation is missing!\n\
         lineage_id contains U+000A (LINE FEED) which must be rejected.\n\
         Result: {:?}",
        result
    );
}

#[test]
fn decode_carriage_return_in_lineage_id_accepted_but_should_not_be() {
    let result = WorkflowLineage::new("valid-prefix\rsuffix".to_string());
    assert!(
        result.is_err(),
        "RED QUEEN: carriage return in lineage_id was accepted — control character validation is missing!\n\
         lineage_id contains U+000D (CARRIAGE RETURN) which must be rejected.\n\
         Result: {:?}",
        result
    );
}

// ===========================================================================
// Additional control character coverage
// ===========================================================================

#[test]
fn lineage_id_null_byte_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\u{0000}with-null".to_string());
    assert!(
        result.is_err(),
        "lineage_id with null byte (U+0000) should be rejected, but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_bell_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\u{0007}with-bell".to_string());
    assert!(
        result.is_err(),
        "lineage_id with bell (U+0007) should be rejected, but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_escape_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\u{001B}with-escape".to_string());
    assert!(
        result.is_err(),
        "lineage_id with escape (U+001B) should be rejected, but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_delete_should_be_rejected() {
    let result = WorkflowLineage::new("lineage\u{007F}with-delete".to_string());
    assert!(
        result.is_err(),
        "lineage_id with DEL (U+007F) should be rejected, but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_valid_id_still_works() {
    let result = WorkflowLineage::new("valid-lineage-id-123".to_string());
    assert!(
        result.is_ok(),
        "valid lineage_id should still be accepted: {:?}",
        result
    );
}
