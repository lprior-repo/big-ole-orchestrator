//! Red Queen Gen 3 tests: control character injection, matching-lineage, and decode-boundary
//! adversarial tests for WorkflowLineage.
//!
//! These tests verify that exotic control characters (form feed, bell, backspace,
//! vertical tab) are rejected, and that dual-side injection scenarios cannot bypass
//! validation.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::useless_vec, unused_imports, unused_variables)]


use vo_types::WorkflowLineage;

// ---------------------------------------------------------------------------
// Gen 3 control_char_injection tests
// ---------------------------------------------------------------------------

#[test]
fn lineage_id_form_feed_should_be_rejected() {
    let result = WorkflowLineage::new("bad\x0Cid".to_string());
    assert!(
        result.is_err(),
        "lineage_id with form feed (\\x0C) should be rejected but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_bell_should_be_rejected() {
    let result = WorkflowLineage::new("bad\x07id".to_string());
    assert!(
        result.is_err(),
        "lineage_id with bell (\\x07) should be rejected but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_backspace_should_be_rejected() {
    let result = WorkflowLineage::new("bad\x08id".to_string());
    assert!(
        result.is_err(),
        "lineage_id with backspace (\\x08) should be rejected but was accepted: {:?}",
        result
    );
}

#[test]
fn lineage_id_vertical_tab_should_be_rejected() {
    let result = WorkflowLineage::new("bad\x0Bid".to_string());
    assert!(
        result.is_err(),
        "lineage_id with vertical tab (\\x0B) should be rejected but was accepted: {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// Gen 3 matching_lineages tests (both sides accepted but dangerous)
// ---------------------------------------------------------------------------

#[test]
fn matching_lineages_with_newline_both_sides_accepted_but_dangerous() {
    let left = WorkflowLineage::new("lineage\na".to_string());
    let right = WorkflowLineage::new("lineage\nb".to_string());
    assert!(
        left.is_err(),
        "left lineage with embedded newline should be rejected: {:?}",
        left
    );
    assert!(
        right.is_err(),
        "right lineage with embedded newline should be rejected: {:?}",
        right
    );
}

#[test]
fn matching_lineages_with_tab_both_sides_accepted_but_dangerous() {
    let left = WorkflowLineage::new("lineage\ta".to_string());
    let right = WorkflowLineage::new("lineage\tb".to_string());
    assert!(
        left.is_err(),
        "left lineage with embedded tab should be rejected: {:?}",
        left
    );
    assert!(
        right.is_err(),
        "right lineage with embedded tab should be rejected: {:?}",
        right
    );
}

// ---------------------------------------------------------------------------
// Gen 3 decode_boundary tests
// ---------------------------------------------------------------------------

#[test]
fn decode_tab_in_lineage_id_accepted_but_should_not_be() {
    let result = WorkflowLineage::new("tab\tsneaky".to_string());
    assert!(
        result.is_err(),
        "lineage_id with tab should be rejected: {:?}",
        result
    );
}
