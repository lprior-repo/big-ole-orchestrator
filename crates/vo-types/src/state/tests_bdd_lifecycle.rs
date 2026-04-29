//! BDD tests for ADR-039 Hierarchical Lifecycle State Machine.
//!
//! Three scenario families:
//! 1. Given lifecycle state S, When invalid transition attempted, Then TransitionError
//!    with correct state and reason.
//! 2. Given hierarchical superstate, When substate query, Then correct parent returned.
//! 3. Given all valid transitions for each state, When exercised, Then state machine
//!    reaches terminal states correctly.

use crate::state::lifecycle::{LifecycleState, TransitionEvent};
use crate::state::transition::{apply, TransitionError};

// ============================================================================
// Scenario 1: Invalid transitions produce TransitionError with correct context
// ============================================================================

mod invalid_transitions {
    use super::*;

    #[test]
    fn given_completed_when_any_event_then_terminal_state_transition_error() {
        // Given a terminal Completed state
        let state = LifecycleState::Completed;

        for event in TransitionEvent::all_variants() {
            // EmitOutputRef is valid from Completed (post-publication emission)
            if *event == TransitionEvent::EmitOutputRef {
                continue;
            }
            // When any transition is attempted
            let result = apply(state, *event);

            // Then TerminalStateTransition error is returned
            assert_eq!(
                result,
                Err(TransitionError::TerminalStateTransition),
                "Completed + {:?} should return TerminalStateTransition",
                event
            );
        }
    }

    #[test]
    fn given_cancelled_when_any_event_then_terminal_state_transition_error() {
        // Given a terminal Cancelled state
        let state = LifecycleState::Cancelled;

        for event in TransitionEvent::all_variants() {
            // When any transition is attempted
            let result = apply(state, *event);

            // Then TerminalStateTransition error is returned
            assert_eq!(
                result,
                Err(TransitionError::TerminalStateTransition),
                "Cancelled + {:?} should return TerminalStateTransition",
                event
            );
        }
    }

    #[test]
    fn given_failed_when_non_resume_event_then_terminal_state_transition_error() {
        // Given a terminal Failed state
        let state = LifecycleState::Failed;

        for event in TransitionEvent::all_variants() {
            if *event == TransitionEvent::InstanceResumed {
                continue;
            }
            // When any non-resume transition is attempted
            let result = apply(state, *event);

            // Then TerminalStateTransition error is returned
            assert_eq!(
                result,
                Err(TransitionError::TerminalStateTransition),
                "Failed + {:?} should return TerminalStateTransition",
                event
            );
        }
    }

    #[test]
    fn given_pending_when_step_scheduled_then_invalid_transition_error() {
        // Given Pending state
        // When StepScheduled is attempted (must go through RunningDecision first)
        let result = apply(LifecycleState::Pending, TransitionEvent::StepScheduled);
        // Then InvalidTransition
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn given_running_decision_when_complete_step_then_invalid_transition_error() {
        // Must go through StepScheduled → StepExecuting first
        let result = apply(
            LifecycleState::RunningDecision,
            TransitionEvent::CompleteStep,
        );
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn given_pending_when_fail_then_invalid_transition_error() {
        // Pending can only AssignToNode or Cancel, not Fail
        let result = apply(LifecycleState::Pending, TransitionEvent::Fail);
        assert_eq!(result, Err(TransitionError::InvalidTransition));
    }

    #[test]
    fn terminal_state_transition_error_displays_correctly() {
        let err = TransitionError::TerminalStateTransition;
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("terminal"),
            "error message should mention 'terminal': {msg}"
        );
    }

    #[test]
    fn invalid_transition_error_displays_correctly() {
        let err = TransitionError::InvalidTransition;
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("invalid"),
            "error message should mention 'invalid': {msg}"
        );
    }
}

// ============================================================================
// Scenario 2: Hierarchical superstate returns correct parent
// ============================================================================

mod superstate_hierarchy {
    use super::*;
    use crate::lifecycle_superstate::LifecycleSuperstate;
    use crate::state::lifecycle::{BlockedReason, OperationalStatus};

    #[test]
    fn given_pending_when_superstate_then_active() {
        assert_eq!(
            LifecycleState::Pending.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn given_waiting_for_timer_when_superstate_then_suspended() {
        assert_eq!(
            LifecycleState::WaitingForTimer.superstate(),
            LifecycleSuperstate::Suspended
        );
    }

    #[test]
    fn given_pending_publication_when_superstate_then_suspended() {
        assert_eq!(
            LifecycleState::PendingPublication.superstate(),
            LifecycleSuperstate::Suspended
        );
    }

    #[test]
    fn given_completed_when_superstate_then_terminal() {
        assert_eq!(
            LifecycleState::Completed.superstate(),
            LifecycleSuperstate::Terminal
        );
    }

    #[test]
    fn given_failed_when_superstate_then_recovering() {
        assert_eq!(
            LifecycleState::Failed.superstate(),
            LifecycleSuperstate::Recovering
        );
    }

    #[test]
    fn given_cancelled_when_superstate_then_terminal() {
        assert_eq!(
            LifecycleState::Cancelled.superstate(),
            LifecycleSuperstate::Terminal
        );
    }

    #[test]
    fn given_any_state_when_superstate_is_terminal_then_is_terminal_agrees() {
        // Note: Failed is terminal but maps to Recovering superstate (it can be resumed).
        // Only Completed and Cancelled are in the Terminal superstate and are truly terminal.
        let terminal_states = [
            LifecycleState::Completed,
            LifecycleState::Cancelled,
        ];
        for state in &terminal_states {
            assert!(state.is_terminal());
            assert_eq!(state.superstate(), LifecycleSuperstate::Terminal);
        }
        // Failed is terminal but in Recovering superstate
        assert!(LifecycleState::Failed.is_terminal());
        assert_eq!(LifecycleState::Failed.superstate(), LifecycleSuperstate::Recovering);

        // Non-terminal states should not be in Terminal superstate
        let all_states = [
            LifecycleState::Pending,
            LifecycleState::RunningDecision,
            LifecycleState::StepScheduled,
            LifecycleState::StepExecuting,
            LifecycleState::PreparingEffect,
            LifecycleState::WaitingForTimer,
            LifecycleState::WaitingForSignal,
            LifecycleState::PendingPublication,
            LifecycleState::Hibernated,
            LifecycleState::Compensating,
            LifecycleState::Reconciling,
        ];
        for state in all_states {
            assert!(!state.is_terminal(), "{:?} should not be terminal", state);
            assert_ne!(
                state.superstate(),
                LifecycleSuperstate::Terminal,
                "{:?} should not be in Terminal superstate",
                state
            );
        }
    }

    #[test]
    fn given_active_superstate_when_operational_status_then_healthy() {
        let active_states = [
            LifecycleState::Pending,
            LifecycleState::RunningDecision,
            LifecycleState::StepScheduled,
            LifecycleState::StepExecuting,
        ];
        for state in active_states {
            assert_eq!(
                state.get_operational_status(),
                OperationalStatus::Healthy,
                "Active state {:?} should be Healthy",
                state
            );
        }
    }

    #[test]
    fn given_step_executing_when_superstate_then_active() {
        assert_eq!(
            LifecycleState::StepExecuting.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn given_step_scheduled_when_superstate_then_active() {
        assert_eq!(
            LifecycleState::StepScheduled.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn given_running_decision_when_superstate_then_active() {
        assert_eq!(
            LifecycleState::RunningDecision.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn given_failed_when_operational_status_then_recovering() {
        assert_eq!(
            LifecycleState::Failed.get_operational_status(),
            OperationalStatus::Recovering
        );
    }

    #[test]
    fn given_pending_publication_when_operational_status_then_blocked() {
        assert!(matches!(
            LifecycleState::PendingPublication.get_operational_status(),
            OperationalStatus::Blocked(BlockedReason::DependenciesPending)
        ));
    }
}

// ============================================================================
// Scenario 3: All valid transitions lead to terminal states correctly
// ============================================================================

mod terminal_reachability {
    use super::*;
    use crate::lifecycle_superstate::LifecycleSuperstate;

    #[test]
    fn given_pending_when_full_happy_path_then_completed() {
        let mut state = LifecycleState::Pending;

        state = apply(state, TransitionEvent::AssignToNode).unwrap();
        assert_eq!(state, LifecycleState::RunningDecision);

        state = apply(state, TransitionEvent::StepScheduled).unwrap();
        assert_eq!(state, LifecycleState::StepScheduled);

        state = apply(state, TransitionEvent::ExecuteStep).unwrap();
        assert_eq!(state, LifecycleState::StepExecuting);

        state = apply(state, TransitionEvent::CompleteStep).unwrap();
        assert_eq!(state, LifecycleState::Completed);

        // Then state is terminal
        assert!(state.is_terminal());
        assert_eq!(state.superstate(), LifecycleSuperstate::Terminal);
    }

    #[test]
    fn given_step_executing_when_timer_wait_and_fire_then_resumes_to_complete() {
        let mut state = LifecycleState::StepExecuting;

        state = apply(state, TransitionEvent::WaitForTimer).unwrap();
        assert_eq!(state, LifecycleState::WaitingForTimer);

        state = apply(state, TransitionEvent::TimerFired).unwrap();
        assert_eq!(state, LifecycleState::StepExecuting);

        state = apply(state, TransitionEvent::CompleteStep).unwrap();
        assert_eq!(state, LifecycleState::Completed);
        assert!(state.is_terminal());
    }

    #[test]
    fn given_waiting_for_timer_when_timer_expired_then_failed() {
        let next = apply(
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerExpired,
        )
        .unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
        assert_eq!(next.superstate(), LifecycleSuperstate::Recovering);
    }

    #[test]
    fn given_pending_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::Pending, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_running_decision_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::RunningDecision, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_step_scheduled_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::StepScheduled, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_step_executing_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::StepExecuting, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_waiting_for_timer_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::WaitingForTimer, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_running_decision_when_fail_then_failed() {
        let next = apply(LifecycleState::RunningDecision, TransitionEvent::Fail).unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_step_scheduled_when_fail_then_failed() {
        let next = apply(LifecycleState::StepScheduled, TransitionEvent::Fail).unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_step_executing_when_fail_then_failed() {
        let next = apply(LifecycleState::StepExecuting, TransitionEvent::Fail).unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_waiting_for_timer_when_fail_then_failed() {
        let next = apply(LifecycleState::WaitingForTimer, TransitionEvent::Fail).unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_failed_when_instance_resumed_then_running_decision() {
        let next = apply(LifecycleState::Failed, TransitionEvent::InstanceResumed).unwrap();
        assert_eq!(next, LifecycleState::RunningDecision);
        assert!(!next.is_terminal());
        assert_eq!(next.superstate(), LifecycleSuperstate::Active);
    }

    #[test]
    fn given_failed_when_resumed_and_completed_then_full_recovery_path() {
        let mut state = LifecycleState::Failed;

        // Recovery
        state = apply(state, TransitionEvent::InstanceResumed).unwrap();
        assert_eq!(state, LifecycleState::RunningDecision);

        // Full happy path
        state = apply(state, TransitionEvent::StepScheduled).unwrap();
        state = apply(state, TransitionEvent::ExecuteStep).unwrap();
        state = apply(state, TransitionEvent::CompleteStep).unwrap();
        assert_eq!(state, LifecycleState::Completed);
        assert!(state.is_terminal());
    }

    #[test]
    fn given_step_executing_when_multiple_timer_round_trips_then_state_remains_consistent() {
        let mut state = LifecycleState::StepExecuting;

        // Three round trips through timer
        for _ in 0..3 {
            state = apply(state, TransitionEvent::WaitForTimer).unwrap();
            assert_eq!(state, LifecycleState::WaitingForTimer);
            assert_eq!(state.superstate(), LifecycleSuperstate::Suspended);

            state = apply(state, TransitionEvent::TimerFired).unwrap();
            assert_eq!(state, LifecycleState::StepExecuting);
            assert_eq!(state.superstate(), LifecycleSuperstate::Active);
        }

        // Still reachable to terminal
        state = apply(state, TransitionEvent::CompleteStep).unwrap();
        assert_eq!(state, LifecycleState::Completed);
    }

    #[test]
    fn given_any_non_pending_pub_state_when_all_valid_transitions_applied_then_all_succeed() {
        let all_states = [
            LifecycleState::Pending,
            LifecycleState::RunningDecision,
            LifecycleState::StepScheduled,
            LifecycleState::StepExecuting,
            LifecycleState::PreparingEffect,
            LifecycleState::WaitingForTimer,
            LifecycleState::WaitingForSignal,
            LifecycleState::Completed,
            LifecycleState::Failed,
            LifecycleState::Cancelled,
        ];

        for state in all_states {
            let valid_events = state.get_valid_transitions();
            for event in valid_events {
                let result = apply(state, event);
                assert!(
                    result.is_ok(),
                    "apply({state:?}, {event:?}) should succeed but got {:?}",
                    result
                );
            }
        }
    }

    #[test]
    fn given_any_state_and_any_event_when_apply_succeeds_then_event_in_valid_transitions() {
        let all_states = [
            LifecycleState::Pending,
            LifecycleState::RunningDecision,
            LifecycleState::StepScheduled,
            LifecycleState::StepExecuting,
            LifecycleState::PreparingEffect,
            LifecycleState::WaitingForTimer,
            LifecycleState::WaitingForSignal,
            LifecycleState::Completed,
            LifecycleState::Failed,
            LifecycleState::Cancelled,
        ];

        for state in all_states {
            let valid = state.get_valid_transitions();
            for event in TransitionEvent::all_variants() {
                // EmitOutputRef is valid from Completed but not in get_valid_transitions()
                if state == LifecycleState::Completed && *event == TransitionEvent::EmitOutputRef {
                    continue;
                }
                let result = apply(state, *event);
                let should_succeed = valid.contains(event);
                assert_eq!(
                    result.is_ok(),
                    should_succeed,
                    "apply({state:?}, {event:?}) = {:?}, expected ok={should_succeed}, valid={valid:?}",
                    result
                );
            }
        }
    }

    // ========================================================================
    // Hibernation, Compensation, Reconciliation paths
    // ========================================================================

    #[test]
    fn given_running_decision_when_hibernate_then_hibernated() {
        let next = apply(LifecycleState::RunningDecision, TransitionEvent::Hibernate).unwrap();
        assert_eq!(next, LifecycleState::Hibernated);
        assert!(!next.is_terminal());
        assert_eq!(next.superstate(), LifecycleSuperstate::Suspended);
    }

    #[test]
    fn given_waiting_for_timer_when_hibernate_then_hibernated() {
        let next = apply(LifecycleState::WaitingForTimer, TransitionEvent::Hibernate).unwrap();
        assert_eq!(next, LifecycleState::Hibernated);
    }

    #[test]
    fn given_hibernated_when_wake_then_running_decision() {
        let next = apply(LifecycleState::Hibernated, TransitionEvent::WakeFromHibernation).unwrap();
        assert_eq!(next, LifecycleState::RunningDecision);
        assert_eq!(next.superstate(), LifecycleSuperstate::Active);
    }

    #[test]
    fn given_hibernated_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::Hibernated, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_step_executing_when_begin_compensation_then_compensating() {
        let next = apply(LifecycleState::StepExecuting, TransitionEvent::BeginCompensation).unwrap();
        assert_eq!(next, LifecycleState::Compensating);
        assert!(!next.is_terminal());
        assert_eq!(next.superstate(), LifecycleSuperstate::Compensating);
    }

    #[test]
    fn given_compensating_when_compensation_completed_then_completed() {
        let next = apply(LifecycleState::Compensating, TransitionEvent::CompensationCompleted).unwrap();
        assert_eq!(next, LifecycleState::Completed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_compensating_when_compensation_failed_then_failed() {
        let next = apply(LifecycleState::Compensating, TransitionEvent::CompensationFailed).unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_reconciling_when_reconciliation_completed_then_running_decision() {
        let next = apply(LifecycleState::Reconciling, TransitionEvent::ReconciliationCompleted).unwrap();
        assert_eq!(next, LifecycleState::RunningDecision);
        assert!(!next.is_terminal());
        assert_eq!(next.superstate(), LifecycleSuperstate::Active);
    }

    #[test]
    fn given_reconciling_when_reconciliation_failed_then_failed() {
        let next = apply(LifecycleState::Reconciling, TransitionEvent::ReconciliationFailed).unwrap();
        assert_eq!(next, LifecycleState::Failed);
        assert!(next.is_terminal());
    }

    #[test]
    fn given_compensating_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::Compensating, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
    }

    #[test]
    fn given_reconciling_when_cancel_then_cancelled() {
        let next = apply(LifecycleState::Reconciling, TransitionEvent::Cancel).unwrap();
        assert_eq!(next, LifecycleState::Cancelled);
    }

    #[test]
    fn full_hibernation_wake_path() {
        let mut state = LifecycleState::Pending;
        state = apply(state, TransitionEvent::AssignToNode).unwrap();
        state = apply(state, TransitionEvent::Hibernate).unwrap();
        assert_eq!(state, LifecycleState::Hibernated);
        state = apply(state, TransitionEvent::WakeFromHibernation).unwrap();
        assert_eq!(state, LifecycleState::RunningDecision);
        state = apply(state, TransitionEvent::StepScheduled).unwrap();
        state = apply(state, TransitionEvent::ExecuteStep).unwrap();
        state = apply(state, TransitionEvent::CompleteStep).unwrap();
        assert_eq!(state, LifecycleState::Completed);
    }

    #[test]
    fn full_compensation_path() {
        let mut state = LifecycleState::StepExecuting;
        state = apply(state, TransitionEvent::BeginCompensation).unwrap();
        assert_eq!(state, LifecycleState::Compensating);
        state = apply(state, TransitionEvent::CompensationCompleted).unwrap();
        assert_eq!(state, LifecycleState::Completed);
    }
}
