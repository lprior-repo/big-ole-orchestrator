use super::*;
use crate::state::lifecycle::{LifecycleState, TransitionEvent};

#[test]
fn test_transition_table_builder_basic() {
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .build()
        .build();

    let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
    assert_eq!(result, Ok(LifecycleState::RunningDecision));
}

#[test]
fn test_transition_table_invalid_transition() {
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .build()
        .build();

    let result = table.apply(LifecycleState::Pending, TransitionEvent::StepScheduled);
    assert_eq!(result, Err(CompilerTransitionError::InvalidTransition));
}

#[test]
fn test_terminal_state_rejection() {
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .build()
        .build();

    let result = table.apply(LifecycleState::Completed, TransitionEvent::AssignToNode);
    assert_eq!(
        result,
        Err(CompilerTransitionError::TerminalStateTransition)
    );
}

#[test]
fn test_guard_always_accepts() {
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .with_guard(Guard::Always)
        .build()
        .build();

    let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
    assert_eq!(result, Ok(LifecycleState::RunningDecision));
}

#[test]
fn test_guard_never_rejects() {
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .with_guard(Guard::Never)
        .build()
        .build();

    let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
    assert_eq!(result, Err(CompilerTransitionError::GuardRejected));
}

#[test]
fn test_guard_predicate() {
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .with_guard(Guard::If(|_, _| true))
        .build()
        .build();

    let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
    assert_eq!(result, Ok(LifecycleState::RunningDecision));
}

#[test]
fn test_side_effect_execution() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static EXECUTED: AtomicBool = AtomicBool::new(false);
    let table = TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .with_side_effect(SideEffect::Fn {
            f: Box::new(|_, _, _| {
                EXECUTED.store(true, Ordering::SeqCst);
                SideEffectResult::Executed
            }),
        })
        .build()
        .build();

    table
        .apply(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .unwrap();
    assert!(EXECUTED.load(Ordering::SeqCst));
}

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
