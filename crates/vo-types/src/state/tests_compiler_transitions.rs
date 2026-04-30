use super::compiler::create_lifecycle_table;
use super::lifecycle::{LifecycleState, TransitionEvent};

#[test]
fn test_lifecycle_table_complete() {
    let table = create_lifecycle_table();

    assert_eq!(
        table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode),
        Ok(LifecycleState::RunningDecision)
    );
    assert_eq!(
        table.apply(
            LifecycleState::RunningDecision,
            TransitionEvent::StepScheduled
        ),
        Ok(LifecycleState::StepScheduled)
    );
    assert_eq!(
        table.apply(LifecycleState::StepScheduled, TransitionEvent::ExecuteStep),
        Ok(LifecycleState::StepExecuting)
    );
    assert_eq!(
        table.apply(LifecycleState::StepExecuting, TransitionEvent::CompleteStep),
        Ok(LifecycleState::Completed)
    );
}

#[test]
fn test_lifecycle_table_cancel() {
    let table = create_lifecycle_table();

    assert_eq!(
        table.apply(LifecycleState::Pending, TransitionEvent::Cancel),
        Ok(LifecycleState::Cancelled)
    );
}

#[test]
fn test_lifecycle_table_fail() {
    let table = create_lifecycle_table();

    assert_eq!(
        table.apply(LifecycleState::RunningDecision, TransitionEvent::Fail),
        Ok(LifecycleState::Failed)
    );
}

#[test]
fn test_lifecycle_table_recovery() {
    let table = create_lifecycle_table();

    assert_eq!(
        table.apply(LifecycleState::Failed, TransitionEvent::InstanceResumed),
        Ok(LifecycleState::RunningDecision)
    );
}

#[test]
fn test_lifecycle_table_timer_path() {
    let table = create_lifecycle_table();

    assert_eq!(
        table.apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer),
        Ok(LifecycleState::WaitingForTimer)
    );
    assert_eq!(
        table.apply(LifecycleState::WaitingForTimer, TransitionEvent::TimerFired),
        Ok(LifecycleState::StepExecuting)
    );
}

#[test]
fn test_lifecycle_table_timer_expired() {
    let table = create_lifecycle_table();

    assert_eq!(
        table.apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer),
        Ok(LifecycleState::WaitingForTimer)
    );
    assert_eq!(
        table.apply(
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerExpired
        ),
        Ok(LifecycleState::Failed)
    );
}

#[test]
fn test_dot_visualization() {
    let table = create_lifecycle_table();
    let dot = table.to_dot();

    assert!(dot.contains("digraph LifecycleStateMachine"));
    assert!(dot.contains("Pending"));
    assert!(dot.contains("RunningDecision"));
}

#[test]
fn test_get_transitions_from() {
    let table = create_lifecycle_table();
    let transitions = table.get_transitions_from(LifecycleState::Pending);

    assert_eq!(transitions.len(), 2);
}

#[test]
fn test_is_terminal_state() {
    let table = create_lifecycle_table();

    assert!(table.is_terminal_state(&LifecycleState::Completed));
    assert!(table.is_terminal_state(&LifecycleState::Failed));
    assert!(table.is_terminal_state(&LifecycleState::Cancelled));
    assert!(!table.is_terminal_state(&LifecycleState::Pending));
}
