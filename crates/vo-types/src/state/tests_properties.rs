//! Property-based tests for state machine invariants.
//!
//! These tests verify exhaustive properties of the state machine transition matrix.
//! Covers behaviors 191-198 from the test plan.

use super::*;
use proptest::prelude::*;

proptest::proptest! {
    // ========================================================================
    // 4.1 Exhaustive Transition Matrix Property
    // ========================================================================

    #[test]
    fn apply_returns_deterministic_result_for_all_state_event_pairs(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
            Just(LifecycleState::Completed),
            Just(LifecycleState::Failed),
            Just(LifecycleState::Cancelled),
        ],
        event in prop_oneof![
            Just(TransitionEvent::AssignToNode),
            Just(TransitionEvent::Cancel),
            Just(TransitionEvent::StepScheduled),
            Just(TransitionEvent::Fail),
            Just(TransitionEvent::ExecuteStep),
            Just(TransitionEvent::WaitForTimer),
            Just(TransitionEvent::CompleteStep),
            Just(TransitionEvent::TimerFired),
            Just(TransitionEvent::TimerExpired),
            Just(TransitionEvent::InstanceResumed),
        ]
    ) {
        let result1 = apply(state, event);
        let result2 = apply(state, event);
        prop_assert_eq!(result1, result2, "apply() must be deterministic");
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.2 Superstate Consistency Property
    // ========================================================================

    #[test]
    fn superstate_consistency_after_transition(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
        ],
        event in prop_oneof![
            Just(TransitionEvent::AssignToNode),
            Just(TransitionEvent::Cancel),
            Just(TransitionEvent::StepScheduled),
            Just(TransitionEvent::Fail),
            Just(TransitionEvent::ExecuteStep),
            Just(TransitionEvent::WaitForTimer),
            Just(TransitionEvent::CompleteStep),
            Just(TransitionEvent::TimerFired),
            Just(TransitionEvent::TimerExpired),
        ]
    ) {
        let initial_superstate = state.superstate();
        match apply(state, event) {
            Ok(new_state) => {
                let new_superstate = new_state.superstate();
                match initial_superstate {
                    crate::lifecycle_superstate::LifecycleSuperstate::Active => {
                        prop_assert!(
                            new_superstate == crate::lifecycle_superstate::LifecycleSuperstate::Active
                            || new_superstate == crate::lifecycle_superstate::LifecycleSuperstate::Suspended
                            || new_superstate == crate::lifecycle_superstate::LifecycleSuperstate::Terminal,
                            "Active states should transition to Active, Suspended, or Terminal"
                        );
                    }
                    crate::lifecycle_superstate::LifecycleSuperstate::Suspended => {
                        prop_assert!(
                            new_superstate == crate::lifecycle_superstate::LifecycleSuperstate::Suspended
                            || new_superstate == crate::lifecycle_superstate::LifecycleSuperstate::Active
                            || new_superstate == crate::lifecycle_superstate::LifecycleSuperstate::Terminal,
                            "Suspended states should transition to Suspended, Active, or Terminal"
                        );
                    }
                    _ => {}
                }
            }
            Err(_) => {
                prop_assert!(!state.is_terminal(), "Non-terminal states should accept valid events");
            }
        }
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.3 Terminal State Absorbing Property
    // ========================================================================

    #[test]
    fn terminal_states_absorb_all_events(
        terminal_state in prop_oneof![
            Just(LifecycleState::Completed),
            Just(LifecycleState::Failed),
            Just(LifecycleState::Cancelled),
        ],
        event in prop_oneof![
            Just(TransitionEvent::AssignToNode),
            Just(TransitionEvent::Cancel),
            Just(TransitionEvent::StepScheduled),
            Just(TransitionEvent::Fail),
            Just(TransitionEvent::ExecuteStep),
            Just(TransitionEvent::WaitForTimer),
            Just(TransitionEvent::CompleteStep),
            Just(TransitionEvent::TimerFired),
            Just(TransitionEvent::TimerExpired),
            Just(TransitionEvent::InstanceResumed),
        ]
    ) {
        let result = apply(terminal_state, event);
        prop_assert!(
            result.is_err(),
            "Terminal state {:?} should reject all events, but accepted {:?}",
            terminal_state,
            event
        );
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.4 OperationalStatus Consistency Property
    // ========================================================================

    #[test]
    fn operational_status_consistency_with_state_classification(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
            Just(LifecycleState::Completed),
            Just(LifecycleState::Failed),
            Just(LifecycleState::Cancelled),
        ]
    ) {
        let status = state.get_operational_status();
        let is_terminal = state.is_terminal();

        match status {
            OperationalStatus::Healthy => {
                prop_assert!(
                    !is_terminal,
                    "Healthy status should only be for non-terminal states"
                );
            }
            OperationalStatus::Blocked(_) => {
                prop_assert!(
                    is_terminal,
                    "Blocked status should only be for terminal states"
                );
            }
            OperationalStatus::Recovering => {
                prop_assert_eq!(
                    state,
                    LifecycleState::Failed,
                    "Recovering status should only be for Failed state"
                );
            }
        }
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.5 No Invalid Self-Transitions Property
    // ========================================================================

    #[test]
    fn no_state_transitions_to_itself_via_valid_event(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
        ]
    ) {
        let valid_events = state.get_valid_transitions();
        for event in valid_events {
            let result = apply(state, event);
            match result {
                Ok(new_state) => {
                    prop_assert_ne!(
                        new_state, state,
                        "State {:?} should not transition to itself via {:?}",
                        state, event
                    );
                }
                Err(_) => {
                    prop_assert!(
                        false,
                        "Valid event {:?} should not be rejected from state {:?}",
                        event, state
                    );
                }
            }
        }
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.6 Valid Transitions Completeness Property
    // ========================================================================

    #[test]
    fn get_valid_transitions_returns_exhaustive_events(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
            Just(LifecycleState::Completed),
            Just(LifecycleState::Failed),
            Just(LifecycleState::Cancelled),
        ]
    ) {
        let valid_events = state.get_valid_transitions();
        let all_events = TransitionEvent::all_variants();

        for event in valid_events {
            let result = apply(state, event);
            prop_assert!(
                result.is_ok(),
                "get_valid_transitions() listed {:?} as valid for {:?}, but apply() returned {:?}",
                event, state, result
            );
        }

        for event in all_events {
            if !valid_events.contains(&event) {
                let result = apply(state, event);
                prop_assert!(
                    result.is_err(),
                    "get_valid_transitions() did not list {:?} for {:?}, but apply() returned {:?}",
                    event, state, result
                );
            }
        }
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.7 InstanceResumed Only From Failed Property
    // ========================================================================

    #[test]
    fn instance_resumed_only_valid_from_failed(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
            Just(LifecycleState::Completed),
            Just(LifecycleState::Cancelled),
        ]
    ) {
        let result = apply(state, TransitionEvent::InstanceResumed);
        prop_assert!(
            result.is_err(),
            "InstanceResumed should be invalid from {:?}, but got {:?}",
            state, result
        );
    }
}

proptest::proptest! {
    // ========================================================================
    // 4.8 Cancel Universality Property
    // ========================================================================

    #[test]
    fn cancel_accepted_from_all_non_terminal_states(
        state in prop_oneof![
            Just(LifecycleState::Pending),
            Just(LifecycleState::RunningDecision),
            Just(LifecycleState::StepScheduled),
            Just(LifecycleState::StepExecuting),
            Just(LifecycleState::WaitingForTimer),
        ]
    ) {
        let result = apply(state, TransitionEvent::Cancel);
        prop_assert_eq!(
            result,
            Ok(LifecycleState::Cancelled),
            "Cancel should be accepted from all non-terminal states, but {:?} returned error",
            state
        );
    }
}
