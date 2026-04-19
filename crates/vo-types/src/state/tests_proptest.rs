//! Property-based tests for LeaseRecord immutability and lifecycle transitions.

use super::{apply, LeaseRecord, LifecycleState, TransitionError, TransitionEvent};

proptest! {
    #[test]
    fn leaserecord_immutability_proptest(i in ".*", s in ".*", t in 1u64..) {
        let instance = crate::InstanceId(i);
        let step = crate::string_types::StepId(s);
        let token = crate::integer_types::FenceToken(std::num::NonZeroU64::new(t).unwrap());

        let rec = LeaseRecord::new(instance.clone(), step.clone(), token);
        proptest::prop_assert_eq!(rec.instance_id(), &instance);
        proptest::prop_assert_eq!(rec.step_id(), &step);
        proptest::prop_assert_eq!(rec.token(), &token);
    }

    #[test]
    fn lifecycle_valid_transitions_are_deterministic(state: LifecycleState, event: TransitionEvent) {
        let valid_events = state.get_valid_transitions();
        if valid_events.contains(&event) {
            let result1 = apply(state, event);
            let result2 = apply(state, event);
            prop_assert_eq!(result1, result2, "apply() must be deterministic");
        }
    }

    #[test]
    fn lifecycle_terminal_states_reject_all_events(state: LifecycleState, event: TransitionEvent) {
        if state.is_terminal() {
            let result = apply(state, event);
            prop_assert!(result.is_err(), "terminal state {:?} should reject {:?}", state, event);
            if let Err(e) = result {
                prop_assert!(matches!(e, TransitionError::TerminalStateTransition),
                    "expected TerminalStateTransition, got {:?}", e);
            }
        }
    }

    #[test]
    fn lifecycle_pending_valid_transitions(transition_count: usize) {
        use std::collections::HashSet;
        let valid = LifecycleState::Pending.get_valid_transitions();
        prop_assume!(transition_count < 1000);
        let mut seen = HashSet::new();
        for _ in 0..transition_count {
            let result = apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
            prop_assert!(result.is_ok());
            seen.insert(result.unwrap());
        }
        prop_assert!(seen.len() <= 2);
    }

    #[test]
    fn lifecycle_running_decision_valid_transitions_proptest(event: TransitionEvent) {
        let valid_events = LifecycleState::RunningDecision.get_valid_transitions();
        let result = apply(LifecycleState::RunningDecision, event);
        if valid_events.contains(&event) {
            prop_assert!(result.is_ok(), "valid event {:?} should succeed", event);
        }
    }

    #[test]
    fn lifecycle_step_scheduled_valid_transitions_proptest(event: TransitionEvent) {
        let valid_events = LifecycleState::StepScheduled.get_valid_transitions();
        let result = apply(LifecycleState::StepScheduled, event);
        if valid_events.contains(&event) {
            prop_assert!(result.is_ok(), "valid event {:?} should succeed", event);
        }
    }

    #[test]
    fn lifecycle_step_executing_valid_transitions_proptest(event: TransitionEvent) {
        let valid_events = LifecycleState::StepExecuting.get_valid_transitions();
        let result = apply(LifecycleState::StepExecuting, event);
        if valid_events.contains(&event) {
            prop_assert!(result.is_ok(), "valid event {:?} should succeed", event);
        }
    }

    #[test]
    fn lifecycle_waiting_for_timer_valid_transitions_proptest(event: TransitionEvent) {
        let valid_events = LifecycleState::WaitingForTimer.get_valid_transitions();
        let result = apply(LifecycleState::WaitingForTimer, event);
        if valid_events.contains(&event) {
            prop_assert!(result.is_ok(), "valid event {:?} should succeed", event);
        }
    }

    #[test]
    fn lifecycle_transition_events_all_variants_iteration() {
        let all_events = TransitionEvent::all_variants();
        prop_assert!(all_events.len() >= 9, "should have at least 9 transition events");
    }
}
