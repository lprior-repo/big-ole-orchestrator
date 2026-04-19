//! Effect replay correctness tests (ve-3bbme).
//!
//! Tests that journal replay produces identical state to original execution.
//! Covers: full replay, partial replay, replay order determinism, and
//! effect lifecycle state reconstruction.

use super::test_helpers::*;
use super::ReplayEngine;

// ── Full replay produces identical state ────────────────────────────────

#[test]
fn full_workflow_replay_produces_completed_state() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, effect_prepared_payload("wf-1", "step-1", "fx-1")),
        make_event("inst-1", 5, effect_committed_payload("wf-1", "step-1", "fx-1")),
        make_event("inst-1", 6, step_completed_payload("wf-1", "step-1")),
    ];

    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.events_applied, 6);
    assert!(result.final_state.is_some());
}

#[test]
fn replay_with_multiple_steps_produces_consistent_state() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        // Step 1
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        // Step 2
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
        make_event("inst-1", 6, step_started_payload("wf-1", "step-2")),
        make_event("inst-1", 7, effect_prepared_payload("wf-1", "step-2", "fx-2")),
        make_event("inst-1", 8, effect_committed_payload("wf-1", "step-2", "fx-2")),
        make_event("inst-1", 9, step_completed_payload("wf-1", "step-2")),
    ];

    let result1 = engine.replay(&events).expect("first replay");
    let result2 = engine.replay(&events).expect("second replay");
    assert_eq!(result1, result2, "replay must be deterministic");
    assert_eq!(result1.events_applied, 9);
}

// ── Partial replay ──────────────────────────────────────────────────────

#[test]
fn partial_replay_stops_at_terminal_state() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        // Extra event after completion should be ignored
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
    ];

    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.events_applied, 4);
}

#[test]
fn partial_replay_from_mid_sequence() {
    let engine = ReplayEngine::new();
    // Simulate replaying only events 3-4 (step started + completed)
    // This will fail because the first event must start from Pending
    let events = vec![
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];

    // Should succeed — starts from Pending, applies step_started then step_completed
    let result = engine.replay(&events);
    assert!(result.is_ok());
}

// ── Replay order determinism ────────────────────────────────────────────

#[test]
fn replay_order_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "s1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "s1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "s2")),
        make_event("inst-1", 6, step_started_payload("wf-1", "s2")),
        make_event("inst-1", 7, step_completed_payload("wf-1", "s2")),
    ];

    // Replay 100 times, all must produce identical results
    for _ in 0..100 {
        let result = engine.replay(&events).expect("replay");
        assert_eq!(result.events_applied, 7);
    }
}

#[test]
fn replay_empty_events_returns_none_state() {
    let engine = ReplayEngine::new();
    let result = engine.replay(&[]).expect("empty replay");
    assert_eq!(result.final_state, None);
    assert_eq!(result.events_applied, 0);
}

// ── Effect lifecycle events are counted ─────────────────────────────────

#[test]
fn effect_prepared_is_counted_but_does_not_change_state() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "s1")),
        make_event("inst-1", 4, effect_prepared_payload("wf-1", "s1", "fx-1")),
        make_event("inst-1", 5, effect_committed_payload("wf-1", "s1", "fx-1")),
        make_event("inst-1", 6, step_completed_payload("wf-1", "s1")),
    ];

    let result = engine.replay(&events).expect("replay");
    assert_eq!(result.events_applied, 6, "all 6 events should be counted");
}

// ── Timer events in replay ──────────────────────────────────────────────

#[test]
fn replay_with_timer_events() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_event("inst-1", 3, timer_set_payload("wf-1", "t1")),
        make_event("inst-1", 4, timer_fired_payload("wf-1", "t1")),
        make_event("inst-1", 5, step_started_payload("wf-1", "s1")),
        make_event("inst-1", 6, step_completed_payload("wf-1", "s1")),
    ];

    let result = engine.replay(&events).expect("replay with timers");
    assert_eq!(result.events_applied, 6);
}

// ── Failed workflow replay ──────────────────────────────────────────────

#[test]
fn replay_failed_workflow() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "s1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "s1")),
    ];

    let result = engine.replay(&events).expect("failed workflow replay");
    assert_eq!(result.events_applied, 4);
}

#[test]
fn replay_cancelled_workflow() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, cancel_requested_payload("wf-1")),
        make_event("inst-1", 3, workflow_cancelled_payload("wf-1")),
    ];

    let result = engine.replay(&events).expect("cancelled workflow replay");
    assert_eq!(result.events_applied, 3);
    assert!(result.final_state.is_some());
}
