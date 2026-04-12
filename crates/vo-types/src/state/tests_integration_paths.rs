//! Integration path tests for lifecycle state machine (behaviors 178-190)
//!
//! These tests verify full lifecycle paths through the state machine,
//! covering the complete journey from Pending to various terminal states.

use crate::state::compiler::create_lifecycle_table;
use crate::state::lifecycle::{LifecycleState, TransitionEvent};

/// Full happy path: Pending -> Completed
/// 178. Full happy path: Pending -> AssignToNode -> RunningDecision -> StepScheduled -> ExecuteStep -> StepExecuting -> CompleteStep -> Completed
#[test]
fn test_path_happy_to_completed() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::CompleteStep).unwrap();
    assert_eq!(state4, LifecycleState::Completed);
}

/// Fail from RunningDecision
/// 179. Fail from RunningDecision: Pending -> AssignToNode -> RunningDecision -> Fail -> Failed
#[test]
fn test_path_fail_from_running_decision() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::Fail).unwrap();
    assert_eq!(state2, LifecycleState::Failed);
}

/// Fail from StepScheduled
/// 180. Fail from StepScheduled: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> StepExecuting -> Fail -> Failed
#[test]
fn test_path_fail_from_step_scheduled() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::Fail).unwrap();
    assert_eq!(state4, LifecycleState::Failed);
}

/// Fail from StepExecuting
/// 181. Fail from StepExecuting: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> StepExecuting -> Fail -> Failed
#[test]
fn test_path_fail_from_step_executing() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::Fail).unwrap();
    assert_eq!(state4, LifecycleState::Failed);
}

/// Fail from WaitingForTimer
/// 182. Fail from WaitingForTimer: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> Fail -> Failed
#[test]
fn test_path_fail_from_waiting_for_timer() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::WaitForTimer).unwrap();
    assert_eq!(state4, LifecycleState::WaitingForTimer);

    let state5 = table.apply(state4, TransitionEvent::Fail).unwrap();
    assert_eq!(state5, LifecycleState::Failed);
}

/// TimerExpired path
/// 183. TimerExpired path: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> TimerExpired -> Failed
#[test]
fn test_path_timer_expired() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::WaitForTimer).unwrap();
    assert_eq!(state4, LifecycleState::WaitingForTimer);

    let state5 = table.apply(state4, TransitionEvent::TimerExpired).unwrap();
    assert_eq!(state5, LifecycleState::Failed);
}

/// Cancel from Pending
/// 184. Cancel from Pending: Pending -> Cancel -> Cancelled
#[test]
fn test_path_cancel_from_pending() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::Cancel).unwrap();
    assert_eq!(state1, LifecycleState::Cancelled);
}

/// Cancel from RunningDecision
/// 185. Cancel from RunningDecision: Pending -> AssignToNode -> RunningDecision -> Cancel -> Cancelled
#[test]
fn test_path_cancel_from_running_decision() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::Cancel).unwrap();
    assert_eq!(state2, LifecycleState::Cancelled);
}

/// Cancel from StepScheduled
/// 186. Cancel from StepScheduled: Pending -> AssignToNode -> StepScheduled -> Cancel -> Cancelled
#[test]
fn test_path_cancel_from_step_scheduled() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::Cancel).unwrap();
    assert_eq!(state3, LifecycleState::Cancelled);
}

/// Cancel from StepExecuting
/// 187. Cancel from StepExecuting: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> StepExecuting -> Cancel -> Cancelled
#[test]
fn test_path_cancel_from_step_executing() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::Cancel).unwrap();
    assert_eq!(state4, LifecycleState::Cancelled);
}

/// Cancel from WaitingForTimer
/// 188. Cancel from WaitingForTimer: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> Cancel -> Cancelled
#[test]
fn test_path_cancel_from_waiting_for_timer() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::WaitForTimer).unwrap();
    assert_eq!(state4, LifecycleState::WaitingForTimer);

    let state5 = table.apply(state4, TransitionEvent::Cancel).unwrap();
    assert_eq!(state5, LifecycleState::Cancelled);
}

/// Recovery path
/// 189. Recovery path: Pending -> AssignToNode -> RunningDecision -> StepScheduled -> ExecuteStep -> StepExecuting -> Fail -> Failed -> InstanceResumed -> RunningDecision
#[test]
fn test_path_recovery() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::Fail).unwrap();
    assert_eq!(state4, LifecycleState::Failed);

    let state5 = table
        .apply(state4, TransitionEvent::InstanceResumed)
        .unwrap();
    assert_eq!(state5, LifecycleState::RunningDecision);
}

/// Timer firing path
/// 190. Timer firing path: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> TimerFired -> StepExecuting
#[test]
fn test_path_timer_fired() {
    let table = create_lifecycle_table();

    let state0 = LifecycleState::Pending;
    let state1 = table.apply(state0, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state1, LifecycleState::RunningDecision);

    let state2 = table.apply(state1, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state2, LifecycleState::StepScheduled);

    let state3 = table.apply(state2, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state3, LifecycleState::StepExecuting);

    let state4 = table.apply(state3, TransitionEvent::WaitForTimer).unwrap();
    assert_eq!(state4, LifecycleState::WaitingForTimer);

    let state5 = table.apply(state4, TransitionEvent::TimerFired).unwrap();
    assert_eq!(state5, LifecycleState::StepExecuting);
}
