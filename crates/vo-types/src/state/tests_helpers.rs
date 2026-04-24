//! Tests for helper functions: get_operational_status, is_terminal, get_valid_transitions.

use super::*;

#[test]
fn get_operational_status_returns_healthy_for_pending() {
    assert_eq!(
        get_operational_status(LifecycleState::Pending),
        OperationalStatus::Healthy
    );
}

#[test]
fn get_operational_status_returns_healthy_for_running_decision() {
    assert_eq!(
        get_operational_status(LifecycleState::RunningDecision),
        OperationalStatus::Healthy
    );
}

#[test]
fn get_operational_status_returns_healthy_for_step_scheduled() {
    assert_eq!(
        get_operational_status(LifecycleState::StepScheduled),
        OperationalStatus::Healthy
    );
}

#[test]
fn get_operational_status_returns_healthy_for_step_executing() {
    assert_eq!(
        get_operational_status(LifecycleState::StepExecuting),
        OperationalStatus::Healthy
    );
}

#[test]
fn get_operational_status_returns_healthy_for_waiting_for_timer() {
    assert_eq!(
        get_operational_status(LifecycleState::WaitingForTimer),
        OperationalStatus::Healthy
    );
}

#[test]
fn get_operational_status_returns_recovering_for_failed() {
    assert_eq!(
        get_operational_status(LifecycleState::Failed),
        OperationalStatus::Recovering
    );
}

#[test]
fn get_operational_status_returns_blocked_manual_hold_for_completed() {
    assert_eq!(
        get_operational_status(LifecycleState::Completed),
        OperationalStatus::Blocked(BlockedReason::ManualHold)
    );
}

#[test]
fn get_operational_status_returns_blocked_manual_hold_for_cancelled() {
    assert_eq!(
        get_operational_status(LifecycleState::Cancelled),
        OperationalStatus::Blocked(BlockedReason::ManualHold)
    );
}

#[test]
fn is_terminal_returns_true_for_completed() {
    assert!(is_terminal(LifecycleState::Completed));
}

#[test]
fn is_terminal_returns_true_for_failed() {
    assert!(is_terminal(LifecycleState::Failed));
}

#[test]
fn is_terminal_returns_true_for_cancelled() {
    assert!(is_terminal(LifecycleState::Cancelled));
}

#[test]
fn is_terminal_returns_false_for_pending() {
    assert!(!is_terminal(LifecycleState::Pending));
}

#[test]
fn is_terminal_returns_false_for_running_decision() {
    assert!(!is_terminal(LifecycleState::RunningDecision));
}

#[test]
fn is_terminal_returns_false_for_step_scheduled() {
    assert!(!is_terminal(LifecycleState::StepScheduled));
}

#[test]
fn is_terminal_returns_false_for_step_executing() {
    assert!(!is_terminal(LifecycleState::StepExecuting));
}

#[test]
fn is_terminal_returns_false_for_waiting_for_timer() {
    assert!(!is_terminal(LifecycleState::WaitingForTimer));
}

#[test]
fn get_valid_transitions_returns_correct_events_for_pending() {
    let transitions = get_valid_transitions(LifecycleState::Pending);
    assert_eq!(transitions.len(), 2);
    assert!(transitions.contains(&TransitionEvent::AssignToNode));
    assert!(transitions.contains(&TransitionEvent::Cancel));
}

#[test]
fn get_valid_transitions_returns_correct_events_for_running_decision() {
    let transitions = get_valid_transitions(LifecycleState::RunningDecision);
    assert_eq!(transitions.len(), 3);
    assert!(transitions.contains(&TransitionEvent::StepScheduled));
    assert!(transitions.contains(&TransitionEvent::Cancel));
    assert!(transitions.contains(&TransitionEvent::Fail));
}

#[test]
fn get_valid_transitions_returns_correct_events_for_step_scheduled() {
    let transitions = get_valid_transitions(LifecycleState::StepScheduled);
    assert_eq!(transitions.len(), 3);
    assert!(transitions.contains(&TransitionEvent::ExecuteStep));
    assert!(transitions.contains(&TransitionEvent::Cancel));
    assert!(transitions.contains(&TransitionEvent::Fail));
}

#[test]
fn get_valid_transitions_returns_correct_events_for_step_executing() {
    let transitions = get_valid_transitions(LifecycleState::StepExecuting);
    assert_eq!(transitions.len(), 6);
    assert!(transitions.contains(&TransitionEvent::WaitForTimer));
    assert!(transitions.contains(&TransitionEvent::YieldWithBlob));
    assert!(transitions.contains(&TransitionEvent::CompleteStep));
    assert!(transitions.contains(&TransitionEvent::PrepareEffect));
    assert!(transitions.contains(&TransitionEvent::Cancel));
    assert!(transitions.contains(&TransitionEvent::Fail));
}

#[test]
fn get_valid_transitions_returns_correct_events_for_waiting_for_timer() {
    let transitions = get_valid_transitions(LifecycleState::WaitingForTimer);
    assert_eq!(transitions.len(), 4);
    assert!(transitions.contains(&TransitionEvent::TimerFired));
    assert!(transitions.contains(&TransitionEvent::TimerExpired));
    assert!(transitions.contains(&TransitionEvent::Cancel));
    assert!(transitions.contains(&TransitionEvent::Fail));
}

#[test]
fn get_valid_transitions_returns_empty_vec_when_state_has_no_valid_transitions() {
    let transitions = get_valid_transitions(LifecycleState::Completed);
    assert_eq!(transitions.len(), 0);
}

#[test]
fn get_valid_transitions_returns_empty_vec_when_cancelled_has_no_valid_transitions() {
    let transitions = get_valid_transitions(LifecycleState::Cancelled);
    assert_eq!(transitions.len(), 0);
}

#[test]
fn get_valid_transitions_returns_instance_resumed_for_failed() {
    let transitions = get_valid_transitions(LifecycleState::Failed);
    assert_eq!(transitions.len(), 1);
    assert!(transitions.contains(&TransitionEvent::InstanceResumed));
}
