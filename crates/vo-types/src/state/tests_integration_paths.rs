//! Integration path tests for complete lifecycle sequences.
//!
//! These tests verify end-to-end state machine paths through the lifecycle.
//! Covers behaviors 178-190 from the test plan.

use super::*;

// ========================================================================
// 3.1 Happy Path: Pending -> Completed
// ========================================================================

#[test]
fn happy_path_pending_to_completed() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state, LifecycleState::RunningDecision);
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    assert_eq!(state, LifecycleState::StepScheduled);
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    assert_eq!(state, LifecycleState::StepExecuting);
    let state = apply(state, TransitionEvent::CompleteStep).unwrap();
    assert_eq!(state, LifecycleState::Completed);
}

// ========================================================================
// 3.2 Fail Path via Fail Event
// ========================================================================

#[test]
fn fail_path_from_running_decision() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    assert_eq!(state, LifecycleState::RunningDecision);
    let state = apply(state, TransitionEvent::Fail).unwrap();
    assert_eq!(state, LifecycleState::Failed);
}

#[test]
fn fail_path_from_step_scheduled() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::Fail).unwrap();
    assert_eq!(state, LifecycleState::Failed);
}

#[test]
fn fail_path_from_step_executing() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::Fail).unwrap();
    assert_eq!(state, LifecycleState::Failed);
}

#[test]
fn fail_path_from_waiting_for_timer() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::WaitForTimer).unwrap();
    let state = apply(state, TransitionEvent::Fail).unwrap();
    assert_eq!(state, LifecycleState::Failed);
}

// ========================================================================
// 3.3 Fail Path via TimerExpired
// ========================================================================

#[test]
fn fail_path_via_timer_expired() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::WaitForTimer).unwrap();
    let state = apply(state, TransitionEvent::TimerExpired).unwrap();
    assert_eq!(state, LifecycleState::Failed);
}

// ========================================================================
// 3.4 Cancel Path from Various States
// ========================================================================

#[test]
fn cancel_path_from_pending() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::Cancel).unwrap();
    assert_eq!(state, LifecycleState::Cancelled);
}

#[test]
fn cancel_path_from_running_decision() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::Cancel).unwrap();
    assert_eq!(state, LifecycleState::Cancelled);
}

#[test]
fn cancel_path_from_step_scheduled() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::Cancel).unwrap();
    assert_eq!(state, LifecycleState::Cancelled);
}

#[test]
fn cancel_path_from_step_executing() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::Cancel).unwrap();
    assert_eq!(state, LifecycleState::Cancelled);
}

#[test]
fn cancel_path_from_waiting_for_timer() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::WaitForTimer).unwrap();
    let state = apply(state, TransitionEvent::Cancel).unwrap();
    assert_eq!(state, LifecycleState::Cancelled);
}

// ========================================================================
// 3.5 Recovery Path
// ========================================================================

#[test]
fn recovery_path() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::Fail).unwrap();
    assert_eq!(state, LifecycleState::Failed);
    let state = apply(state, TransitionEvent::InstanceResumed).unwrap();
    assert_eq!(state, LifecycleState::RunningDecision);
}

// ========================================================================
// 3.6 Timer Firing Path
// ========================================================================

#[test]
fn timer_firing_path() {
    let state = LifecycleState::Pending;
    let state = apply(state, TransitionEvent::AssignToNode).unwrap();
    let state = apply(state, TransitionEvent::StepScheduled).unwrap();
    let state = apply(state, TransitionEvent::ExecuteStep).unwrap();
    let state = apply(state, TransitionEvent::WaitForTimer).unwrap();
    let state = apply(state, TransitionEvent::TimerFired).unwrap();
    assert_eq!(state, LifecycleState::StepExecuting);
}
