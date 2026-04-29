//! Tests for apply() error behaviors: terminal rejections and invalid transitions.

use super::*;

// ========================================================================
// Terminal State Rejections (20 tests)
// ========================================================================

// Completed state rejections (10)
#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_assign_to_node() {
    let result = apply(LifecycleState::Completed, TransitionEvent::AssignToNode);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_cancel() {
    let result = apply(LifecycleState::Completed, TransitionEvent::Cancel);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_step_scheduled() {
    let result = apply(LifecycleState::Completed, TransitionEvent::StepScheduled);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_execute_step() {
    let result = apply(LifecycleState::Completed, TransitionEvent::ExecuteStep);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_wait_for_timer() {
    let result = apply(LifecycleState::Completed, TransitionEvent::WaitForTimer);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_complete_step() {
    let result = apply(LifecycleState::Completed, TransitionEvent::CompleteStep);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_timer_fired() {
    let result = apply(LifecycleState::Completed, TransitionEvent::TimerFired);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_timer_expired() {
    let result = apply(LifecycleState::Completed, TransitionEvent::TimerExpired);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_fail() {
    let result = apply(LifecycleState::Completed, TransitionEvent::Fail);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_completed_receives_instance_resumed() {
    let result = apply(LifecycleState::Completed, TransitionEvent::InstanceResumed);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

// Cancelled state rejections (10)
#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_assign_to_node() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::AssignToNode);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_cancel() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::Cancel);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_step_scheduled() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::StepScheduled);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_execute_step() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::ExecuteStep);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_wait_for_timer() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::WaitForTimer);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_complete_step() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::CompleteStep);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_timer_fired() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::TimerFired);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_timer_expired() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::TimerExpired);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_fail() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::Fail);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_cancelled_receives_instance_resumed() {
    let result = apply(LifecycleState::Cancelled, TransitionEvent::InstanceResumed);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

// ========================================================================
// InvalidTransition from Non-Terminal States (13 tests)
// ========================================================================

// From Pending
#[test]
fn apply_returns_invalid_transition_when_pending_receives_step_scheduled() {
    let result = apply(LifecycleState::Pending, TransitionEvent::StepScheduled);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_pending_receives_execute_step() {
    let result = apply(LifecycleState::Pending, TransitionEvent::ExecuteStep);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_pending_receives_timer_fired() {
    let result = apply(LifecycleState::Pending, TransitionEvent::TimerFired);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_pending_receives_instance_resumed() {
    let result = apply(LifecycleState::Pending, TransitionEvent::InstanceResumed);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

// From RunningDecision
#[test]
fn apply_returns_invalid_transition_when_running_decision_receives_execute_step() {
    let result = apply(
        LifecycleState::RunningDecision,
        TransitionEvent::ExecuteStep,
    );
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_running_decision_receives_timer_fired() {
    let result = apply(LifecycleState::RunningDecision, TransitionEvent::TimerFired);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_running_decision_receives_instance_resumed() {
    let result = apply(
        LifecycleState::RunningDecision,
        TransitionEvent::InstanceResumed,
    );
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

// From StepScheduled
#[test]
fn apply_returns_invalid_transition_when_step_scheduled_receives_assign_to_node() {
    let result = apply(LifecycleState::StepScheduled, TransitionEvent::AssignToNode);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_step_scheduled_receives_timer_fired() {
    let result = apply(LifecycleState::StepScheduled, TransitionEvent::TimerFired);
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_step_scheduled_receives_instance_resumed() {
    let result = apply(
        LifecycleState::StepScheduled,
        TransitionEvent::InstanceResumed,
    );
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

// From StepExecuting
#[test]
fn apply_returns_invalid_transition_when_step_executing_receives_step_scheduled() {
    let result = apply(
        LifecycleState::StepExecuting,
        TransitionEvent::StepScheduled,
    );
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

#[test]
fn apply_returns_invalid_transition_when_step_executing_receives_instance_resumed() {
    let result = apply(
        LifecycleState::StepExecuting,
        TransitionEvent::InstanceResumed,
    );
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

// From WaitingForTimer
#[test]
fn apply_returns_invalid_transition_when_waiting_for_timer_receives_instance_resumed() {
    let result = apply(
        LifecycleState::WaitingForTimer,
        TransitionEvent::InstanceResumed,
    );
    assert_eq!(result, Err(TransitionError::InvalidTransition));
}

// ========================================================================
// Failed State Rejections (9 tests) - INV-004: Only InstanceResumed is valid
// ========================================================================

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_assign_to_node() {
    let result = apply(LifecycleState::Failed, TransitionEvent::AssignToNode);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_cancel() {
    let result = apply(LifecycleState::Failed, TransitionEvent::Cancel);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_step_scheduled() {
    let result = apply(LifecycleState::Failed, TransitionEvent::StepScheduled);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_execute_step() {
    let result = apply(LifecycleState::Failed, TransitionEvent::ExecuteStep);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_wait_for_timer() {
    let result = apply(LifecycleState::Failed, TransitionEvent::WaitForTimer);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_complete_step() {
    let result = apply(LifecycleState::Failed, TransitionEvent::CompleteStep);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_timer_fired() {
    let result = apply(LifecycleState::Failed, TransitionEvent::TimerFired);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_timer_expired() {
    let result = apply(LifecycleState::Failed, TransitionEvent::TimerExpired);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}

#[test]
fn apply_returns_terminal_state_transition_when_failed_receives_fail() {
    let result = apply(LifecycleState::Failed, TransitionEvent::Fail);
    assert_eq!(result, Err(TransitionError::TerminalStateTransition));
}
