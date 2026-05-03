//!
 //! Property-based tests for vo-actor lifecycle state machine (hibernation transitions).
 //!
 //! These tests verify invariants for ActorLifecycleState transitions covering
 //! the full hibernation lifecycle (ADR-005 + ADR-039):
 //! - All valid state transitions produce expected next states
 //! - Terminal states (Stopped, Failed) reject all transitions
 //! - is_valid_transition is consistent with compute_next_state
 //! - valid_transitions() returns exactly the set of valid transitions
 //! - Transition determinism: same (state, transition) always yields same result

 use proptest::prelude::*;
 use vo_actor::lifecycle::{
     compute_next_state, is_valid_transition, ActorLifecycleState, LifecycleTransition,
 };

 fn all_states() -> Vec<ActorLifecycleState> {
     vec![
         ActorLifecycleState::Pending,
         ActorLifecycleState::Running,
         ActorLifecycleState::Stopping,
         ActorLifecycleState::Stopped,
         ActorLifecycleState::Failed,
     ]
 }

 fn all_transitions() -> Vec<LifecycleTransition> {
     vec![
         LifecycleTransition::Start,
         LifecycleTransition::Stop,
         LifecycleTransition::Fail,
         LifecycleTransition::ChildStopped,
         LifecycleTransition::AllChildrenStopped,
     ]
 }

 // =============================================================================
 // Invariant 1: Determinism — same (state, transition) always yields same result
 // =============================================================================

 proptest! {
     #[test]
     fn compute_next_state_deterministic(
         state in prop::sample::select(all_states()),
         transition in prop::sample::select(all_transitions()),
         // Run multiple times to catch non-deterministic behavior
         _run in 0u8..10,
     ) {
         let result1 = compute_next_state(state, transition);
         let result2 = compute_next_state(state, transition);
         prop_assert_eq!(
             result1, result2,
             "compute_next_state({}, {:?}) should be deterministic",
             state, transition
         );
     }

     #[test]
     fn is_valid_transition_deterministic(
         state in prop::sample::select(all_states()),
         transition in prop::sample::select(all_transitions()),
         _run in 0u8..10,
     ) {
         let result1 = is_valid_transition(state, transition);
         let result2 = is_valid_transition(state, transition);
         prop_assert_eq!(
             result1, result2,
             "is_valid_transition({}, {:?}) should be deterministic",
             state, transition
         );
     }
 }

 // =============================================================================
 // Invariant 2: Consistency — is_valid_transition matches compute_next_state
 // =============================================================================

 proptest! {
     #[test]
     fn is_valid_transition_matches_compute_next_state(
         state in prop::sample::select(all_states()),
         transition in prop::sample::select(all_transitions()),
     ) {
         let next = compute_next_state(state, transition);
         let valid = is_valid_transition(state, transition);
         prop_assert_eq!(
             next.is_some(), valid,
             "is_valid_transition({}, {:?}) = {} should match compute_next_state.is_some() = {}",
             state, transition, valid, next.is_some()
         );
     }
 }

 // =============================================================================
 // Invariant 3: Terminal states reject ALL transitions
 // =============================================================================

 proptest! {
     #[test]
     fn terminal_states_reject_all_transitions(
         terminal_state in prop::sample::select(vec![
             ActorLifecycleState::Stopped,
             ActorLifecycleState::Failed,
         ]),
         transition in prop::sample::select(all_transitions()),
     ) {
         let next = compute_next_state(terminal_state, transition);
         prop_assert!(
             next.is_none(),
             "Terminal state {:?} should reject {:?} but got {:?}",
             terminal_state, transition, next
         );
     }

     #[test]
     fn terminal_states_all_transitions_invalid(
         terminal_state in prop::sample::select(vec![
             ActorLifecycleState::Stopped,
             ActorLifecycleState::Failed,
         ]),
     ) {
         for transition in all_transitions() {
             prop_assert!(
                 !is_valid_transition(terminal_state, transition),
                 "Terminal state {:?} should reject {:?}",
                 terminal_state, transition
             );
         }
     }
 }

 // =============================================================================
 // Invariant 4: Non-terminal states accept exactly their valid transitions
 // =============================================================================

 proptest! {
     #[test]
     fn non_terminal_states_have_valid_transitions(
         state in prop::sample::select(vec![
             ActorLifecycleState::Pending,
             ActorLifecycleState::Running,
             ActorLifecycleState::Stopping,
         ]),
     ) {
         // Each non-terminal state must have at least one valid transition
         let valid_count = all_transitions()
             .iter()
             .filter(|t| is_valid_transition(state, *t))
             .count();

         prop_assert!(
             valid_count > 0,
             "Non-terminal state {:?} must have at least one valid transition",
             state
         );
     }

     #[test]
     fn pending_accepts_only_start_and_fail(
         transition in prop::sample::select(all_transitions()),
     ) {
         let valid = is_valid_transition(ActorLifecycleState::Pending, transition);
         let expected = matches!(
             transition,
             LifecycleTransition::Start | LifecycleTransition::Fail
         );
         prop_assert_eq!(
             valid, expected,
             "Pending should {} {:?} transition",
             if expected { "accept" } else { "reject" },
             transition
         );
     }

     #[test]
     fn running_accepts_only_stop_and_fail(
         transition in prop::sample::select(all_transitions()),
     ) {
         let valid = is_valid_transition(ActorLifecycleState::Running, transition);
         let expected = matches!(
             transition,
             LifecycleTransition::Stop | LifecycleTransition::Fail
         );
         prop_assert_eq!(
             valid, expected,
             "Running should {} {:?} transition",
             if expected { "accept" } else { "reject" },
             transition
         );
     }

     #[test]
     fn stopping_accepts_only_child_stopped_and_all_children_stopped(
         transition in prop::sample::select(all_transitions()),
     ) {
         let valid = is_valid_transition(ActorLifecycleState::Stopping, transition);
         let expected = matches!(
             transition,
             LifecycleTransition::ChildStopped | LifecycleTransition::AllChildrenStopped
         );
         prop_assert_eq!(
             valid, expected,
             "Stopping should {} {:?} transition",
             if expected { "accept" } else { "reject" },
             transition
         );
     }
 }

 // =============================================================================
 // Invariant 5: valid_transitions() returns exactly the valid set
 // =============================================================================

 proptest! {
     #[test]
     fn valid_transitions_method_matches_is_valid_transition(
         state in prop::sample::select(all_states()),
     ) {
         let valid_from_method = state.valid_transitions();

         for transition in all_transitions() {
             let is_valid = is_valid_transition(state, transition);
             let in_method_list = valid_from_method.contains(&transition);

             prop_assert_eq!(
                 is_valid, in_method_list,
                 "valid_transitions() for {:?} should {} transition {:?}",
                 state,
                 if is_valid { "include" } else { "exclude" },
                 transition
             );
         }
     }

     #[test]
     fn valid_transitions_no_duplicates(
         state in prop::sample::select(all_states()),
     ) {
         let valid = state.valid_transitions();
         let mut sorted = valid.clone();
         sorted.sort();
         sorted.dedup();
         prop_assert_eq!(
             valid.len(), sorted.len(),
             "valid_transitions() for {:?} should have no duplicates, got {:?}",
             state, valid
         );
     }
 }

 // =============================================================================
 // Invariant 6: Specific transition outcomes are deterministic
 // =============================================================================

 proptest! {
     #[test]
     fn pending_start_always_yields_running() {
         let next = compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Start);
         prop_assert_eq!(
             next,
             Some(ActorLifecycleState::Running),
             "Pending + Start should yield Running"
         );
     }

     #[test]
     fn pending_fail_always_yields_failed() {
         let next = compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Fail);
         prop_assert_eq!(
             next,
             Some(ActorLifecycleState::Failed),
             "Pending + Fail should yield Failed"
         );
     }

     #[test]
     fn running_stop_always_yields_stopping() {
         let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Stop);
         prop_assert_eq!(
             next,
             Some(ActorLifecycleState::Stopping),
             "Running + Stop should yield Stopping"
         );
     }

     #[test]
     fn running_fail_always_yields_failed() {
         let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Fail);
         prop_assert_eq!(
             next,
             Some(ActorLifecycleState::Failed),
             "Running + Fail should yield Failed"
         );
     }

     #[test]
     fn stopping_all_children_stopped_yields_stopped() {
         let next = compute_next_state(
             ActorLifecycleState::Stopping,
             LifecycleTransition::AllChildrenStopped,
         );
         prop_assert_eq!(
             next,
             Some(ActorLifecycleState::Stopped),
             "Stopping + AllChildrenStopped should yield Stopped"
         );
     }

     #[test]
     fn stopping_child_stopped_yields_stopping() {
         let next = compute_next_state(
             ActorLifecycleState::Stopping,
             LifecycleTransition::ChildStopped,
         );
         prop_assert_eq!(
             next,
             Some(ActorLifecycleState::Stopping),
             "Stopping + ChildStopped should yield Stopping (no transition)"
         );
     }
 }

 // =============================================================================
 // Invariant 7: is_terminal matches behavior
 // =============================================================================

 proptest! {
     #[test]
     fn is_terminal_consistency(state in prop::sample::select(all_states())) {
         let is_t = state.is_terminal();
         // A state is terminal iff it has zero valid transitions
         let valid_count = all_transitions()
             .iter()
             .filter(|t| is_valid_transition(state, *t))
             .count();
         prop_assert_eq!(
             is_t, valid_count == 0,
             "is_terminal({:?}) = {} but valid transition count = {}",
             state, is_t, valid_count
         );
     }
 }

 // =============================================================================
 // Invariant 8: is_stopping flag consistency
 // =============================================================================

 proptest! {
     #[test]
     fn is_stopping_consistency(state in prop::sample::select(all_states())) {
         let is_stopping = state.is_stopping();
         let expected = matches!(
             state,
             ActorLifecycleState::Stopping | ActorLifecycleState::Stopped
         );
         prop_assert_eq!(
             is_stopping, expected,
             "is_stopping({:?}) = {} but expected {}",
             state, is_stopping, expected
         );
     }
 }

 // =============================================================================
 // Invariant 9: can_spawn_child consistency
 // =============================================================================

 proptest! {
     #[test]
     fn can_spawn_child_consistency(state in prop::sample::select(all_states())) {
         let can_spawn = state.can_spawn_child();
         let expected = matches!(
             state,
             ActorLifecycleState::Pending | ActorLifecycleState::Running
         );
         prop_assert_eq!(
             can_spawn, expected,
             "can_spawn_child({:?}) = {} but expected {}",
             state, can_spawn, expected
         );
     }
 }

 // =============================================================================
 // Invariant 10: Round-trip through stopping state (hibernation lifecycle)
 // =============================================================================

 proptest! {
     #[test]
     fn full_hibernation_lifecycle_deterministic() {
         // Running -> Stop -> Stopping -> AllChildrenStopped -> Stopped
         let s1 = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Stop);
         prop_assert_eq!(s1, Some(ActorLifecycleState::Stopping));

         let stopping = s1.unwrap();
         let s2 = compute_next_state(stopping, LifecycleTransition::AllChildrenStopped);
         prop_assert_eq!(s2, Some(ActorLifecycleState::Stopped));

         // Stopped is terminal - no further transitions
         let stopped = s2.unwrap();
         for t in all_transitions() {
             prop_assert!(
                 compute_next_state(stopped, t).is_none(),
                 "Stopped should reject {:?} transition",
                 t
             );
         }
     }

     #[test]
     fn fail_from_pending_does_not_enter_stopping() {
         // Pending -> Fail -> Failed (not Stopping)
         let next = compute_next_state(ActorLifecycleState::Pending, LifecycleTransition::Fail);
         prop_assert_eq!(next, Some(ActorLifecycleState::Failed));
         prop_assert_ne!(next, Some(ActorLifecycleState::Stopping));
     }

     #[test]
     fn fail_from_running_does_not_enter_stopping() {
         // Running -> Fail -> Failed (not Stopping)
         let next = compute_next_state(ActorLifecycleState::Running, LifecycleTransition::Fail);
         prop_assert_eq!(next, Some(ActorLifecycleState::Failed));
         prop_assert_ne!(next, Some(ActorLifecycleState::Stopping));
     }
 }

 // =============================================================================
 // Invariant 11: ChildStopped in Stopping doesn't progress to terminal
 // =============================================================================

 proptest! {
     #[test]
     fn child_stopped_never_reaches_terminal_state() {
         // Multiple ChildStopped events in Stopping should still be Stopping
         let state1 = compute_next_state(
             ActorLifecycleState::Stopping,
             LifecycleTransition::ChildStopped,
         );
         prop_assert_eq!(state1, Some(ActorLifecycleState::Stopping));

         // Even after many ChildStopped events, still not terminal
         let mut current = ActorLifecycleState::Stopping;
         for _ in 0..100 {
             current = compute_next_state(current, LifecycleTransition::ChildStopped)
                 .unwrap_or(current);
         }
         prop_assert!(
             !current.is_terminal(),
             "ChildStopped events should never reach terminal state"
         );
     }
 }