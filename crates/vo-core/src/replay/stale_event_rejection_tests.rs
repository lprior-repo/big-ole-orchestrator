//! Stale event rejection tests for the replay engine (ADR-027).
//!
//! "Stale" events are events that arrive after the workflow has already
//! reached a terminal state or should not affect the replay due to
//! timing/order constraints. These tests verify proper handling.
//!
//! Key scenarios:
//! 1. Events arriving after Completed state are silently ignored (not errors)
//! 2. Events arriving after Cancelled state are silently ignored (not errors)
//! 3. Failed state allows InstanceResumed but rejects other stale events
//! 4. Non-monotonic timestamps in events don't affect replay decisions

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
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
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
        make_event("inst-1", 2, cancel_requested_payload("wf-1")),
        make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 2);
}

#[test]
fn instance_resumed_after_failed_recovers_workflow() {
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
fn non_instance_resumed_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed {
            state: LifecycleState::Failed,
            ..
        }
    ));
}

#[test]
fn multiple_stale_events_after_completed_are_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
        make_event("inst-1", 6, timer_fired_payload("wf-1", "timer-1")),
        make_event("inst-1", 7, cancel_requested_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn continued_as_new_at_completed_is_ignored() {
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
fn workflow_started_at_completed_is_ignored() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, workflow_started_payload("wf-2")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
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
fn double_instance_resumed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_failed_payload("wf-1")),
        make_event("inst-1", 3, instance_resumed_payload("wf-1")),
        make_event("inst-1", 4, instance_resumed_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed {
            state: LifecycleState::RunningDecision,
            ..
        }
    ));
}

#[test]
fn stale_step_scheduled_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-2")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed {
            state: LifecycleState::Failed,
            ..
        }
    ));
}

#[test]
fn stale_timer_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed {
            state: LifecycleState::Failed,
            ..
        }
    ));
}

#[test]
fn stale_workflow_cancelled_after_failed_is_rejected() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, workflow_cancelled_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        super::types::ReplayError::TransitionFailed {
            state: LifecycleState::Failed,
            ..
        }
    ));
}

#[test]
fn multiple_failure_recovery_cycles_converge() {
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
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    assert_eq!(result.events_applied, 10);
}

#[test]
fn recovery_after_third_failure_succeeds() {
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
        make_event("inst-1", 12, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 13, instance_resumed_payload("wf-1")),
        make_event("inst-1", 14, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    assert_eq!(result.events_applied, 14);
}
