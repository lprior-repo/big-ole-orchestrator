//! Happy-path unit tests for the replay engine (Behaviors 1–13, 21–22).

use super::engine::ReplayEngine;
use super::test_helpers::*;
use vo_types::state::LifecycleState;

// =========================================================================
// Behavior 1: ReplayEngine::new()
// =========================================================================

#[test]
fn replay_engine_new_creates_instance() {
    let _engine = ReplayEngine::new();
}

#[test]
fn replay_engine_default_creates_instance() {
    let _engine = ReplayEngine;
}

// =========================================================================
// Behavior 2: Empty event list
// =========================================================================

#[test]
fn replay_returns_empty_result_when_event_list_is_empty() {
    let engine = ReplayEngine::new();
    let result = engine.replay(&[]).expect("empty replay should succeed");
    assert_eq!(result.final_state, None);
    assert_eq!(result.events_applied, 0);
}

// =========================================================================
// Behavior 3: WorkflowStarted maps to AssignToNode
// =========================================================================

#[test]
fn replay_maps_workflow_started_to_assign_to_node_transition() {
    let engine = ReplayEngine::new();
    let events = [make_event("inst-1", 1, workflow_started_payload("wf-1"))];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 1);
}

// =========================================================================
// Behavior 4: StepScheduled maps correctly
// =========================================================================

#[test]
fn replay_maps_step_scheduled_to_step_scheduled_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
    assert_eq!(result.events_applied, 2);
}

// =========================================================================
// Behavior 5: StepStarted maps to ExecuteStep
// =========================================================================

#[test]
fn replay_maps_step_started_to_execute_step_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
    assert_eq!(result.events_applied, 3);
}

// =========================================================================
// Behavior 6: StepCompleted maps to CompleteStep
// =========================================================================

#[test]
fn replay_maps_step_completed_to_complete_step_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

// =========================================================================
// Behavior 7: StepFailed maps to Fail
// =========================================================================

#[test]
fn replay_maps_step_failed_to_fail_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Failed));
    assert_eq!(result.events_applied, 4);
}

// =========================================================================
// Behavior 8: TimerSet maps to WaitForTimer
// =========================================================================

#[test]
fn replay_maps_timer_set_to_wait_for_timer_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::WaitingForTimer));
    assert_eq!(result.events_applied, 4);
}

// =========================================================================
// Behavior 9: TimerFired maps correctly
// =========================================================================

#[test]
fn replay_maps_timer_fired_to_timer_fired_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
        make_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
    assert_eq!(result.events_applied, 5);
}

// =========================================================================
// Behavior 10: Cancel transitions
// =========================================================================

#[test]
fn replay_maps_workflow_cancelled_to_cancel_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 2);
}

#[test]
fn replay_maps_cancel_requested_to_cancel_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, cancel_requested_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 4);
}

// =========================================================================
// Behavior 11: WorkflowFailed maps to Fail
// =========================================================================

#[test]
fn replay_maps_workflow_failed_to_fail_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_failed_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Failed));
    assert_eq!(result.events_applied, 2);
}

// =========================================================================
// Behavior 12: InstanceResumed maps correctly
// =========================================================================

#[test]
fn replay_maps_instance_resumed_to_instance_resumed_transition() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_failed_payload("wf-1")),
        make_event("inst-1", 3, instance_resumed_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 3);
}

// =========================================================================
// Behavior 13: ContinuedAsNew is no-op
// =========================================================================

#[test]
fn replay_treats_continued_as_new_as_noop() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, continued_as_new_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 2);
}

// =========================================================================
// Behavior 21: Determinism
// =========================================================================

#[test]
fn replay_is_deterministic_same_events_produce_same_result() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let result1 = engine.replay(&events).expect("first replay");
    let result2 = engine.replay(&events).expect("second replay");
    assert_eq!(result1, result2);
}

// =========================================================================
// Behavior 22: events_applied count
// =========================================================================

#[test]
fn replay_reports_correct_events_applied_count() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.events_applied, 3);
}
