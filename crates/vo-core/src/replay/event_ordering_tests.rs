//! Event ordering invariant tests (ADR-027 Section 6).
//!
//! Key constraints:
//! 1. Deterministic iteration order - candidate node selection uses ordered data structures
//! 2. No wall-clock time in decisions - wall-clock time may trigger timers but NOT routing
//! 3. Sequence validation - events must be strictly monotonically increasing per instance
//! 4. Instance consistency - all events for replay must have same instance_id

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;

#[test]
fn replay_requires_strictly_incrementing_sequence() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
    ];

    let err = engine
        .replay(&events)
        .expect_err("should fail with sequence gap");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 3,
            actual: 4,
            at_index: 2,
        }
    ));
}

#[test]
fn replay_rejects_duplicate_sequence_numbers() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 2, step_started_payload("wf-1", "step-1")),
    ];

    let err = engine
        .replay(&events)
        .expect_err("should fail with duplicate sequence");
    assert!(matches!(
        err,
        ReplayError::SequenceDuplicate {
            sequence: 2,
            first_at_index: 1,
            second_at_index: 2,
        }
    ));
}

#[test]
fn replay_rejects_non_monotonic_sequence() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 3, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];

    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 4,
            actual: 2,
            at_index: 1,
        }
    ));
}

#[test]
fn replay_allows_sequence_starting_at_any_value() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 100, workflow_started_payload("wf-1")),
        make_event("inst-1", 101, step_scheduled_payload("wf-1", "step-1")),
    ];

    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::StepScheduled)
    );
}

#[test]
fn replay_allows_sequence_starting_at_zero() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 0, workflow_started_payload("wf-1")),
        make_event("inst-1", 1, step_scheduled_payload("wf-1", "step-1")),
    ];

    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::StepScheduled)
    );
}

#[test]
fn replay_rejects_mixed_instance_ids() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
    ];

    let err = engine
        .replay(&events)
        .expect_err("should fail with instance mismatch");
    assert!(matches!(
        err,
        ReplayError::InstanceMismatch {
            expected: _,
            actual: _,
        }
    ));
}

#[test]
fn replay_allows_empty_instance_id() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("", 1, workflow_started_payload("wf-1")),
        make_event("", 2, step_scheduled_payload("wf-1", "step-1")),
    ];

    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::StepScheduled)
    );
}

#[test]
fn replay_validates_instance_id_across_all_events() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-2", 3, step_started_payload("wf-1", "step-1")),
    ];

    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}

#[test]
fn replay_event_order_matters_for_state_machine() {
    let engine = ReplayEngine::new();

    let valid_order = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];

    let result = engine
        .replay(&valid_order)
        .expect("valid order should succeed");
    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::Completed)
    );
}

#[test]
fn replay_invalid_order_fails() {
    let engine = ReplayEngine::new();

    let invalid_order = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_started_payload("wf-1", "step-1")),
    ];

    let err = engine
        .replay(&invalid_order)
        .expect_err("invalid order should fail");
    assert!(matches!(err, ReplayError::TransitionFailed { .. }));
}

#[test]
fn replay_sequence_gap_at_any_position_fails() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_started_payload("wf-1", "step-1")),
    ];

    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 3,
            actual: 5,
            at_index: 2,
        }
    ));
}

#[test]
fn replay_validates_first_event_sequence_too() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 2, workflow_started_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
    ];

    let result = engine
        .replay(&events)
        .expect("should succeed with non-1 start");
    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::StepScheduled)
    );
}

#[test]
fn replay_accepts_arbitrary_sequence_gap_at_first_event() {
    let engine = ReplayEngine::new();

    let events = [
        make_event("inst-1", 1000, workflow_started_payload("wf-1")),
        make_event("inst-1", 1001, step_scheduled_payload("wf-1", "step-1")),
    ];

    let result = engine.replay(&events).expect("should succeed");
    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::StepScheduled)
    );
}
