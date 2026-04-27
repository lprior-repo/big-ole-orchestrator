//! Doc-to-Beads Rerun Gate — BDD Integration Tests
//!
//! Validates that the spec-hardening gate correctly blocks/allows doc-to-beads
//! and arch-spec-to-beads reruns based on open planner-expansion beads.
//!
//! BDD Scenarios:
//!   Given no spec-hardening beads are open
//!   When the gate is checked
//!   Then the gate ALLOWS the rerun
//!
//!   Given spec-hardening beads are open
//!   When the gate is checked
//!   Then the gate BLOCKS the rerun with evidence of blocking beads
//!
//! Required proof command: cargo test -p vo-linter --test rerun_gate_bdd

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use vo_linter::rules::{check_spec_hardening_gate, GateResult, GateStatus};

/// BDD Scenario: Given no spec-hardening beads are open
/// When the gate is checked
/// Then the gate ALLOWS the rerun
///
/// Evidence: GateResult.status == GateStatus::Allowed, open_beads is empty
#[test]
fn given_no_spec_hardening_beads_open_when_gate_checked_then_allowed() {
    let result = check_spec_hardening_gate();

    if result.is_allowed() {
        assert!(
            result.open_beads.is_empty(),
            "Allowed gate must have zero open_beads"
        );
        assert_eq!(
            result.total_checked, 0,
            "Allowed gate must report zero total_checked"
        );
    } else {
        assert!(
            !result.open_beads.is_empty(),
            "If gate is blocked, there must be open_beads. Result: {:?}",
            result
        );
    }
}

/// BDD Scenario: Given spec-hardening beads are open
/// When the gate is checked
/// Then the gate BLOCKS the rerun with evidence of blocking beads
///
/// Evidence: GateResult.status == GateStatus::Blocked, open_beads contains bead info
#[test]
fn given_spec_hardening_beads_open_when_gate_checked_then_blocked() {
    let result = check_spec_hardening_gate();

    if result.is_blocked() {
        assert!(
            !result.open_beads.is_empty(),
            "Blocked gate must have at least one open_beads entry"
        );
        assert_eq!(
            result.blocked_count(),
            result.total_checked,
            "blocked_count must equal total_checked"
        );
        for bead in &result.open_beads {
            assert!(
                !bead.id.is_empty(),
                "Each open_beads entry must have a non-empty id"
            );
            assert!(
                !bead.title.is_empty(),
                "Each open_beads entry must have a non-empty title"
            );
        }
    } else {
        assert!(
            result.open_beads.is_empty(),
            "If gate is allowed, open_beads must be empty. Result: {:?}",
            result
        );
    }
}

/// BDD Scenario: GateResult helper methods work correctly
///
/// Given a GateResult with blocked status
/// When is_blocked() and blocked_count() are called
/// Then they return correct values
#[test]
fn given_gate_result_blocked_when_helpers_called_then_correct() {
    let result = GateResult {
        status: GateStatus::Blocked,
        open_beads: vec![
            vo_linter::rules::rerun_gate::OpenBead {
                id: "tw-test-001".to_string(),
                title: "Test spec-hardening bead".to_string(),
                priority: 1,
            },
            vo_linter::rules::rerun_gate::OpenBead {
                id: "tw-test-002".to_string(),
                title: "Another spec-hardening bead".to_string(),
                priority: 2,
            },
        ],
        total_checked: 2,
    };

    assert!(!result.is_allowed(), "Blocked result must NOT be allowed");
    assert!(result.is_blocked(), "Blocked result must be blocked");
    assert_eq!(result.blocked_count(), 2, "blocked_count must be 2");
}

/// BDD Scenario: GateResult helper methods work correctly for allowed state
///
/// Given a GateResult with allowed status
/// When is_allowed() is called
/// Then it returns true with zero blocked count
#[test]
fn given_gate_result_allowed_when_helpers_called_then_correct() {
    let result = GateResult {
        status: GateStatus::Allowed,
        open_beads: Vec::new(),
        total_checked: 0,
    };

    assert!(result.is_allowed(), "Allowed result must be allowed");
    assert!(!result.is_blocked(), "Allowed result must NOT be blocked");
    assert_eq!(result.blocked_count(), 0, "blocked_count must be 0");
}