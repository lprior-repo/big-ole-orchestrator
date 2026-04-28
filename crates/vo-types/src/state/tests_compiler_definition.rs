use super::compiler::{
    CompilerTransitionError, Guard, SideEffect, SideEffectResult, TransitionTable,
};
use super::lifecycle::{LifecycleState, TransitionEvent};

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
    assert_eq!(
        result,
        Err(CompilerTransitionError::InvalidTransition)
    );
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
    assert_eq!(
        result,
        Err(CompilerTransitionError::GuardRejected)
    );
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
