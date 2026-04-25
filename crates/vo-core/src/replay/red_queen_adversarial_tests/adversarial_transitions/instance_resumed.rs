use super::super::*;
use vo_types::state::LifecycleState;

#[test]
fn replay_rejects_instance_resumed_from_pending() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, instance_resumed_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed {
            sequence: 2,
            state: LifecycleState::RunningDecision,
            ..
        }
    ));
}

#[test]
fn replay_rejects_instance_resumed_from_running_decision() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, instance_resumed_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed {
            sequence: 2,
            state: LifecycleState::RunningDecision,
            ..
        }
    ));
}

#[test]
fn replay_rejects_instance_resumed_from_step_scheduled() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, instance_resumed_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed {
            sequence: 3,
            state: LifecycleState::StepScheduled,
            ..
        }
    ));
}

#[test]
fn replay_rejects_instance_resumed_from_step_executing() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, instance_resumed_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed {
            sequence: 4,
            state: LifecycleState::StepExecuting,
            ..
        }
    ));
}

#[test]
fn replay_rejects_instance_resumed_from_waiting_for_timer() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
    ];
    let err = engine.replay(&events).expect_err("should fail");
    assert!(matches!(
        err,
        ReplayError::TransitionFailed {
            sequence: 5,
            state: LifecycleState::WaitingForTimer,
            ..
        }
    ));
}

#[test]
fn replay_stops_processing_at_completed_ignores_instance_resumed() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn replay_stops_processing_at_cancelled_ignores_instance_resumed() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, workflow_cancelled_payload("wf-1")),
        make_event("inst-1", 3, instance_resumed_payload("wf-1")),
    ];
    let result = engine.replay(&events).expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
    assert_eq!(result.events_applied, 2);
}

#[test]
fn replay_rejects_step_scheduled_after_failed() {
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
        ReplayError::TransitionFailed {
            sequence: 5,
            state: LifecycleState::Failed,
            ..
        }
    ));
}

#[test]
fn replay_rejects_timer_set_after_failed() {
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
        ReplayError::TransitionFailed {
            sequence: 5,
            state: LifecycleState::Failed,
            ..
        }
    ));
}

#[test]
fn replay_rejects_double_instance_resumed() {
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
        ReplayError::TransitionFailed {
            sequence: 4,
            state: LifecycleState::RunningDecision,
            ..
        }
    ));
}

#[test]
fn replay_handles_multiple_failure_recovery_cycles() {
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
