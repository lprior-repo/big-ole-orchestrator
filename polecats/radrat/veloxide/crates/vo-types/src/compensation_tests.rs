//! Unit tests for compensation types (ADR-034).

use crate::compensation::*;
use rstest::rstest;

// ========================================================================
// CompensationStatus Derive Tests
// ========================================================================

#[test]
fn compensationstatus_debug_format_equals_variant_name() {
    assert_eq!(format!("{:?}", CompensationStatus::NotNeeded), "NotNeeded");
    assert_eq!(format!("{:?}", CompensationStatus::Pending), "Pending");
    assert_eq!(
        format!("{:?}", CompensationStatus::InProgress),
        "InProgress"
    );
    assert_eq!(format!("{:?}", CompensationStatus::Succeeded), "Succeeded");
    assert_eq!(format!("{:?}", CompensationStatus::Failed), "Failed");
}

#[test]
fn compensationstatus_clone_copy_semantics_preserve_equality() {
    let state = CompensationStatus::Pending;
    let copy = state;
    assert_eq!(state, copy);

    let state1 = CompensationStatus::Succeeded;
    let state2 = state1;
    assert_eq!(state1, state2);
}

#[test]
fn compensationstatus_partial_eq_and_hash_are_consistent() {
    assert_eq!(CompensationStatus::Pending, CompensationStatus::Pending);
    assert_ne!(CompensationStatus::Pending, CompensationStatus::Failed);
    assert_ne!(CompensationStatus::Succeeded, CompensationStatus::Failed);

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h1 = DefaultHasher::new();
    CompensationStatus::Pending.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    CompensationStatus::Pending.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

// ========================================================================
// CompensationStatus Serde Round-Trip
// ========================================================================

#[rstest]
#[case(CompensationStatus::NotNeeded, "NotNeeded")]
#[case(CompensationStatus::Pending, "Pending")]
#[case(CompensationStatus::InProgress, "InProgress")]
#[case(CompensationStatus::Succeeded, "Succeeded")]
#[case(CompensationStatus::Failed, "Failed")]
fn compensationstatus_serializes_and_deserializes_for_all_variants(
    #[case] variant: CompensationStatus,
    #[case] expected_json: &str,
) {
    let json = serde_json::to_string(&variant).unwrap();
    assert_eq!(json, format!("\"{expected_json}\""));
    let recovered: CompensationStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, variant);
}

// ========================================================================
// CompensationStatus::is_terminal
// ========================================================================

#[test]
fn compensationstatus_is_terminal_returns_true_when_not_needed() {
    assert!(CompensationStatus::NotNeeded.is_terminal());
}

#[test]
fn compensationstatus_is_terminal_returns_false_when_pending() {
    assert!(!CompensationStatus::Pending.is_terminal());
}

#[test]
fn compensationstatus_is_terminal_returns_false_when_in_progress() {
    assert!(!CompensationStatus::InProgress.is_terminal());
}

#[test]
fn compensationstatus_is_terminal_returns_true_when_succeeded() {
    assert!(CompensationStatus::Succeeded.is_terminal());
}

#[test]
fn compensationstatus_is_terminal_returns_true_when_failed() {
    assert!(CompensationStatus::Failed.is_terminal());
}

// ========================================================================
// CompensationStatus::all_variants
// ========================================================================

#[test]
fn compensationstatus_all_variants_returns_five_variants_in_declaration_order() {
    let variants = CompensationStatus::all_variants();
    assert_eq!(variants.len(), 5);
    assert_eq!(variants[0], CompensationStatus::NotNeeded);
    assert_eq!(variants[1], CompensationStatus::Pending);
    assert_eq!(variants[2], CompensationStatus::InProgress);
    assert_eq!(variants[3], CompensationStatus::Succeeded);
    assert_eq!(variants[4], CompensationStatus::Failed);
}

// ========================================================================
// CompensationTransitionEvent::all_variants
// ========================================================================

#[test]
fn compensation_transition_event_all_variants_returns_three_events() {
    let variants = CompensationTransitionEvent::all_variants();
    assert_eq!(variants.len(), 3);
    assert_eq!(variants[0], CompensationTransitionEvent::Start);
    assert_eq!(variants[1], CompensationTransitionEvent::Succeed);
    assert_eq!(variants[2], CompensationTransitionEvent::Fail);
}

// ========================================================================
// apply_compensation_transition — Happy Paths
// ========================================================================

#[test]
fn apply_compensation_transition_returns_in_progress_when_pending_start() {
    let result = apply_compensation_transition(
        CompensationStatus::Pending,
        CompensationTransitionEvent::Start,
    );
    assert_eq!(result, Ok(CompensationStatus::InProgress));
}

#[test]
fn apply_compensation_transition_returns_succeeded_when_in_progress_succeed() {
    let result = apply_compensation_transition(
        CompensationStatus::InProgress,
        CompensationTransitionEvent::Succeed,
    );
    assert_eq!(result, Ok(CompensationStatus::Succeeded));
}

#[test]
fn apply_compensation_transition_returns_failed_when_pending_fail() {
    let result = apply_compensation_transition(
        CompensationStatus::Pending,
        CompensationTransitionEvent::Fail,
    );
    assert_eq!(result, Ok(CompensationStatus::Failed));
}

#[test]
fn apply_compensation_transition_returns_failed_when_in_progress_fail() {
    let result = apply_compensation_transition(
        CompensationStatus::InProgress,
        CompensationTransitionEvent::Fail,
    );
    assert_eq!(result, Ok(CompensationStatus::Failed));
}

// ========================================================================
// apply_compensation_transition — Terminal Rejections (INV-COMP-002)
// ========================================================================

#[test]
fn apply_compensation_transition_returns_terminal_error_when_not_needed_start() {
    let result = apply_compensation_transition(
        CompensationStatus::NotNeeded,
        CompensationTransitionEvent::Start,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_not_needed_fail() {
    let result = apply_compensation_transition(
        CompensationStatus::NotNeeded,
        CompensationTransitionEvent::Fail,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_not_needed_succeed() {
    let result = apply_compensation_transition(
        CompensationStatus::NotNeeded,
        CompensationTransitionEvent::Succeed,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_succeeded_start() {
    let result = apply_compensation_transition(
        CompensationStatus::Succeeded,
        CompensationTransitionEvent::Start,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_succeeded_fail() {
    let result = apply_compensation_transition(
        CompensationStatus::Succeeded,
        CompensationTransitionEvent::Fail,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_succeeded_succeed() {
    let result = apply_compensation_transition(
        CompensationStatus::Succeeded,
        CompensationTransitionEvent::Succeed,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_failed_start() {
    let result = apply_compensation_transition(
        CompensationStatus::Failed,
        CompensationTransitionEvent::Start,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_failed_fail() {
    let result = apply_compensation_transition(
        CompensationStatus::Failed,
        CompensationTransitionEvent::Fail,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

#[test]
fn apply_compensation_transition_returns_terminal_error_when_failed_succeed() {
    let result = apply_compensation_transition(
        CompensationStatus::Failed,
        CompensationTransitionEvent::Succeed,
    );
    assert_eq!(
        result,
        Err(CompensationTransitionError::TerminalStateTransition)
    );
}

// ========================================================================
// apply_compensation_transition — Invalid Transitions
// ========================================================================

#[test]
fn apply_compensation_transition_returns_invalid_error_when_pending_succeed() {
    let result = apply_compensation_transition(
        CompensationStatus::Pending,
        CompensationTransitionEvent::Succeed,
    );
    assert_eq!(result, Err(CompensationTransitionError::InvalidTransition));
}

#[test]
fn apply_compensation_transition_returns_invalid_error_when_in_progress_start() {
    let result = apply_compensation_transition(
        CompensationStatus::InProgress,
        CompensationTransitionEvent::Start,
    );
    assert_eq!(result, Err(CompensationTransitionError::InvalidTransition));
}

// ========================================================================
// CompensationTransitionError Tests
// ========================================================================

#[test]
fn compensation_transition_error_terminal_displays_correct_message() {
    let err = CompensationTransitionError::TerminalStateTransition;
    assert_eq!(
        err.to_string(),
        "Cannot transition from terminal compensation state"
    );
}

#[test]
fn compensation_transition_error_invalid_displays_correct_message() {
    let err = CompensationTransitionError::InvalidTransition;
    assert_eq!(err.to_string(), "Invalid compensation state transition");
}

#[test]
fn compensation_transition_error_implements_std_error_error() {
    let err: Box<dyn std::error::Error> =
        Box::new(CompensationTransitionError::TerminalStateTransition);
    assert_eq!(
        err.to_string(),
        "Cannot transition from terminal compensation state"
    );
}

// ========================================================================
// CompensationRecord Construction
// ========================================================================

#[test]
fn compensationrecord_returns_some_when_constructed_with_typical_components() {
    let record = CompensationRecord::new(
        "fx-123".to_string(),
        crate::effects::CompensationPolicy::Automatic,
        CompensationStatus::Pending,
        None,
        None,
        None,
    );
    assert!(record.is_some());
    let r = record.unwrap();
    assert_eq!(r.effect_id(), "fx-123");
    assert_eq!(r.policy(), crate::effects::CompensationPolicy::Automatic);
    assert_eq!(r.status(), CompensationStatus::Pending);
    assert_eq!(r.compensation_effect_id(), None);
    assert_eq!(r.started_at(), None);
    assert_eq!(r.completed_at(), None);
}

#[test]
fn compensationrecord_returns_some_when_constructed_with_single_char_effect_id() {
    let record = CompensationRecord::new(
        "a".to_string(),
        crate::effects::CompensationPolicy::Manual,
        CompensationStatus::Pending,
        None,
        None,
        None,
    );
    assert!(record.is_some());
    assert_eq!(record.unwrap().effect_id(), "a");
}

#[test]
fn compensationrecord_returns_none_when_effect_id_is_empty() {
    let result = CompensationRecord::new(
        "".to_string(),
        crate::effects::CompensationPolicy::Automatic,
        CompensationStatus::Pending,
        None,
        None,
        None,
    );
    assert_eq!(result, None);
}

#[test]
fn compensationrecord_returns_some_when_constructed_with_compensation_effect_id_and_timestamps() {
    let started = crate::types::TimestampMs(1000);
    let completed = crate::types::TimestampMs(2000);
    let record = CompensationRecord::new(
        "fx-456".to_string(),
        crate::effects::CompensationPolicy::Automatic,
        CompensationStatus::Succeeded,
        Some("comp-789".to_string()),
        Some(started),
        Some(completed),
    );
    assert!(record.is_some());
    let r = record.unwrap();
    assert_eq!(r.effect_id(), "fx-456");
    assert_eq!(r.status(), CompensationStatus::Succeeded);
    assert_eq!(r.compensation_effect_id(), Some("comp-789"));
    assert_eq!(r.started_at(), Some(&started));
    assert_eq!(r.completed_at(), Some(&completed));
}

#[test]
fn compensationrecord_serializes_and_deserializes_via_json_round_trip() {
    let record = CompensationRecord::new(
        "fx-789".to_string(),
        crate::effects::CompensationPolicy::Manual,
        CompensationStatus::Pending,
        Some("comp-101".to_string()),
        None,
        None,
    );
    let r = record.unwrap();
    let json = serde_json::to_string(&r).unwrap();
    let recovered: CompensationRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, r);
}
