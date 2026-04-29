//! Tests for apply() happy-path transitions.

use super::*;

#[test]
fn apply_returns_running_decision_when_pending_assigned_to_node() {
    let result = apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
    assert_eq!(result, Ok(LifecycleState::RunningDecision));
}

#[test]
fn apply_returns_cancelled_when_pending_cancels() {
    let result = apply(LifecycleState::Pending, TransitionEvent::Cancel);
    assert_eq!(result, Ok(LifecycleState::Cancelled));
}

#[test]
fn apply_returns_step_scheduled_when_running_decision_step_scheduled() {
    let result = apply(
        LifecycleState::RunningDecision,
        TransitionEvent::StepScheduled,
    );
    assert_eq!(result, Ok(LifecycleState::StepScheduled));
}

#[test]
fn apply_returns_cancelled_when_running_decision_cancels() {
    let result = apply(LifecycleState::RunningDecision, TransitionEvent::Cancel);
    assert_eq!(result, Ok(LifecycleState::Cancelled));
}

#[test]
fn apply_returns_failed_when_running_decision_fails() {
    let result = apply(LifecycleState::RunningDecision, TransitionEvent::Fail);
    assert_eq!(result, Ok(LifecycleState::Failed));
}

#[test]
fn apply_returns_step_executing_when_step_scheduled_execute_step() {
    let result = apply(LifecycleState::StepScheduled, TransitionEvent::ExecuteStep);
    assert_eq!(result, Ok(LifecycleState::StepExecuting));
}

#[test]
fn apply_returns_cancelled_when_step_scheduled_cancels() {
    let result = apply(LifecycleState::StepScheduled, TransitionEvent::Cancel);
    assert_eq!(result, Ok(LifecycleState::Cancelled));
}

#[test]
fn apply_returns_failed_when_step_scheduled_fails() {
    let result = apply(LifecycleState::StepScheduled, TransitionEvent::Fail);
    assert_eq!(result, Ok(LifecycleState::Failed));
}

#[test]
fn apply_returns_waiting_for_timer_when_step_executing_wait_for_timer() {
    let result = apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer);
    assert_eq!(result, Ok(LifecycleState::WaitingForTimer));
}

#[test]
fn apply_returns_completed_when_step_executing_complete_step() {
    let result = apply(LifecycleState::StepExecuting, TransitionEvent::CompleteStep);
    assert_eq!(result, Ok(LifecycleState::Completed));
}

#[test]
fn apply_returns_cancelled_when_step_executing_cancels() {
    let result = apply(LifecycleState::StepExecuting, TransitionEvent::Cancel);
    assert_eq!(result, Ok(LifecycleState::Cancelled));
}

#[test]
fn apply_returns_failed_when_step_executing_fails() {
    let result = apply(LifecycleState::StepExecuting, TransitionEvent::Fail);
    assert_eq!(result, Ok(LifecycleState::Failed));
}

#[test]
fn apply_returns_step_executing_when_waiting_for_timer_timer_fired() {
    let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::TimerFired);
    assert_eq!(result, Ok(LifecycleState::StepExecuting));
}

#[test]
fn apply_returns_failed_when_waiting_for_timer_timer_expired() {
    let result = apply(
        LifecycleState::WaitingForTimer,
        TransitionEvent::TimerExpired,
    );
    assert_eq!(result, Ok(LifecycleState::Failed));
}

#[test]
fn apply_returns_cancelled_when_waiting_for_timer_cancels() {
    let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::Cancel);
    assert_eq!(result, Ok(LifecycleState::Cancelled));
}

#[test]
fn apply_returns_failed_when_waiting_for_timer_fails() {
    let result = apply(LifecycleState::WaitingForTimer, TransitionEvent::Fail);
    assert_eq!(result, Ok(LifecycleState::Failed));
}

#[test]
fn apply_returns_running_decision_when_failed_instance_resumed() {
    let result = apply(LifecycleState::Failed, TransitionEvent::InstanceResumed);
    assert_eq!(result, Ok(LifecycleState::RunningDecision));
}
