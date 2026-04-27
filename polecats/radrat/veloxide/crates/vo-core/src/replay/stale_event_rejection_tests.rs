//! Stale event rejection tests for exactly-once replay correctness (ADR-027).
//!
//! Property: replaying any valid event sequence produces the same final state
//! regardless of checkpoint boundaries. Events arriving after terminal states
//! (Completed, Cancelled) must be silently ignored. Events arriving after Failed
//! that are not InstanceResumed must be rejected.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use vo_types::state::LifecycleState;

#[test]
fn events_after_completed_are_silently_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
        make_event("inst-1", 6, timer_fired_payload("wf-1", "timer-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn events_after_cancelled_are_silently_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, cancel_requested_payload("wf-1")),
        make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 3);
}

#[test]
fn non_instance_resumed_event_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
    ];
    let result = engine.replay(&events);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed { .. }
    ));
}

#[test]
fn instance_resumed_after_failed_is_accepted() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 5);
}

#[test]
fn multiple_failure_recovery_cycles_converge_correctly() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
        make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 8, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 9, instance_resumed_payload("wf-1")),
        make_event("inst-1", 10, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 11, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 12, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 12);
}

#[test]
fn continued_as_new_at_completed_state_is_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, continued_as_new_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn continued_as_new_at_cancelled_state_is_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
        make_event("inst-1", 3, continued_as_new_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 2);
}

#[test]
fn workflow_failed_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, workflow_failed_payload("wf-1")),
    ];
    let result = engine.replay(&events);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed { .. }
    ));
}

#[test]
fn cancel_requested_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, cancel_requested_payload("wf-1")),
    ];
    let result = engine.replay(&events);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed { .. }
    ));
}

#[test]
fn timer_events_after_completed_are_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
        make_event("inst-1", 6, timer_fired_payload("wf-1", "timer-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn step_started_after_completed_is_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_started_payload("wf-1", "step-2")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn instance_resumed_at_running_state_is_accepted() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
        make_event("inst-1", 6, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 7, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 8, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 9, instance_resumed_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 8);
}

#[test]
fn stale_events_not_counted_in_events_applied() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
        make_event("inst-1", 6, step_started_payload("wf-1", "step-2")),
        make_event("inst-1", 7, step_completed_payload("wf-1", "step-2")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
    assert_ne!(result.events_applied, 7);
}

#[test]
fn continued_as_new_during_running_is_noop() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, continued_as_new_payload("wf-1")),
        make_event("inst-1", 4, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 5);
}
