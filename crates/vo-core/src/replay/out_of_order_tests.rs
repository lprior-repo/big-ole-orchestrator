//! Out-of-order event rejection tests (ve-gr2ly).
//!
//! The replay engine enforces strict monotonic sequence ordering.
//! Out-of-order events are detected and rejected with specific error types.
//! Tests cover: minor reorders, major reorders, gap detection, and
//! timestamp vs sequence independence.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;

// ---------------------------------------------------------------------------
// Minor reorder: adjacent events swapped
// ---------------------------------------------------------------------------

#[test]
fn minor_reorder_adjacent_swap_rejected() {
    let engine = ReplayEngine::new();
    // Sequences 2 and 3 are swapped: [1, 3, 2]
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 2, step_started_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("swapped adjacent events should fail");
    assert!(matches!(err, ReplayError::SequenceGap { expected: 2, actual: 3, .. }));
}

#[test]
fn minor_reorder_last_two_swapped() {
    let engine = ReplayEngine::new();
    // Valid order would be 1,2,3,4. We swap 3 and 4.
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("swapped last two events should fail");
    assert!(matches!(err, ReplayError::SequenceGap { expected: 3, actual: 4, .. }));
}

// ---------------------------------------------------------------------------
// Major reorder: large sequence disruption
// ---------------------------------------------------------------------------

#[test]
fn major_reorder_first_and_last_swapped() {
    let engine = ReplayEngine::new();
    // Sequences: [4, 2, 3, 1] — first and last swapped
    let events = [
        make_event("inst-1", 4, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 1, step_completed_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("major reorder should fail");
    assert!(matches!(err, ReplayError::SequenceGap { .. }));
}

#[test]
fn major_reorder_reverse_order() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("reverse order should fail");
    assert!(matches!(err, ReplayError::SequenceGap { .. }));
}

#[test]
fn major_reorder_large_gap_in_middle() {
    let engine = ReplayEngine::new();
    // Sequences jump from 2 to 1000
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 1000, step_started_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("large gap should fail");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 3,
            actual: 1000,
            at_index: 2,
        }
    ));
}

// ---------------------------------------------------------------------------
// Gap detection
// ---------------------------------------------------------------------------

#[test]
fn gap_detection_single_missing_event() {
    let engine = ReplayEngine::new();
    // Missing sequence 3: [1, 2, 4, 5]
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_completed_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("gap should be detected");
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
fn gap_detection_multiple_missing_events() {
    let engine = ReplayEngine::new();
    // Missing sequences 3-99: [1, 2, 100]
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 100, step_started_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("large gap should be detected");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 3,
            actual: 100,
            at_index: 2,
        }
    ));
}

#[test]
fn gap_detection_gap_at_beginning_after_first() {
    let engine = ReplayEngine::new();
    // First event at seq 1, then jumps to 10
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 10, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("gap after first event");
    assert!(matches!(
        err,
        ReplayError::SequenceGap {
            expected: 2,
            actual: 10,
            at_index: 1,
        }
    ));
}

#[test]
fn gap_detection_no_gap_succeeds() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("no gap should succeed");
    assert_eq!(result.events_applied, 3);
}

// ---------------------------------------------------------------------------
// Timestamp independence: timestamps can be out of order
// ---------------------------------------------------------------------------

#[test]
fn timestamp_out_of_order_with_correct_sequence_succeeds() {
    let engine = ReplayEngine::new();
    // Sequences are correct (1, 2, 3) but timestamps are out of order (3000, 1000, 2000)
    let mut events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
    ];
    // Scramble timestamps
    events[0].timestamp_ms = 3000;
    events[1].timestamp_ms = 1000;
    events[2].timestamp_ms = 2000;

    let result = engine.replay(&events).expect("correct sequence should succeed regardless of timestamps");
    assert_eq!(result.events_applied, 3);
    assert_eq!(result.final_state, Some(vo_types::state::LifecycleState::StepExecuting));
}

#[test]
fn timestamp_backward_with_correct_sequence_succeeds() {
    let engine = ReplayEngine::new();
    let mut events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    events[0].timestamp_ms = 9999;
    events[1].timestamp_ms = 1000;

    let result = engine.replay(&events).expect("sequence ordering, not timestamp ordering");
    assert_eq!(result.events_applied, 2);
}

// ---------------------------------------------------------------------------
// Fully shuffled sequence
// ---------------------------------------------------------------------------

#[test]
fn fully_shuffled_four_events_rejected() {
    let engine = ReplayEngine::new();
    // Sequences: [3, 1, 4, 2]
    let events = [
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine.replay(&events).expect_err("shuffled events should fail");
    assert!(matches!(err, ReplayError::SequenceGap { .. }));
}

// ---------------------------------------------------------------------------
// Correctly ordered sequence succeeds (control test)
// ---------------------------------------------------------------------------

#[test]
fn correctly_ordered_events_succeed() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("correctly ordered events should succeed");
    assert_eq!(result.events_applied, 4);
    assert_eq!(result.final_state, Some(vo_types::state::LifecycleState::Completed));
}
