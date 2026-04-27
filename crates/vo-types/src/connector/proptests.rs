//! Proptest invariants for connector types (feature-gated).

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
    use crate::connector::runtime::ReconciliationResult;
    use crate::connector::transition::apply_connector_transition;
    use crate::connector::types::*;
    use proptest::prelude::*;

    proptest::proptest! {
        /// INV: Serde round-trip preserves ConnectorState equality for all variants.
        #[test]
        fn connector_state_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ConnectorState::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ConnectorState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves ConnectorResult equality for all variants.
        #[test]
        fn connector_result_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ConnectorResult::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ConnectorResult = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves ReconcileAction equality for all variants.
        #[test]
        fn reconcile_action_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ReconcileAction::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ReconcileAction = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves ConnectorTransition equality for all variants.
        #[test]
        fn connector_transition_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ConnectorTransition::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ConnectorTransition = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: apply_connector_transition never panics for any (state, event) combination.
        #[test]
        fn apply_connector_transition_never_panics_for_any_combination(
            state_idx in 0usize..7,
            event_idx in 0usize..9,
        ) {
            let states = ConnectorState::all_variants();
            let events = ConnectorTransition::all_variants();
            let state = states[state_idx];
            let event = events[event_idx];
            let _ = apply_connector_transition(state, event);
        }

        /// INV: Terminal states (Succeeded, Failed) reject all transitions.
        #[test]
        fn terminal_states_reject_all_transitions(
            event_idx in 0usize..9,
        ) {
            let events = ConnectorTransition::all_variants();
            let event = events[event_idx];

            let result_succeeded = apply_connector_transition(ConnectorState::Succeeded, event);
            prop_assert!(result_succeeded.is_err(), "Succeeded must reject all events");

            let result_failed = apply_connector_transition(ConnectorState::Failed, event);
            prop_assert!(result_failed.is_err(), "Failed must reject all events");
        }

        /// INV: Ambiguous state only accepts reconciliation events.
        #[test]
        fn ambiguous_state_only_accepts_reconciliation_events(
            non_reconcile_event in proptest::sample::select(&[
                ConnectorTransition::Prepare,
                ConnectorTransition::Prepared,
                ConnectorTransition::Commit,
                ConnectorTransition::Succeed,
                ConnectorTransition::Fail,
                ConnectorTransition::Ambiguate,
            ][..])
        ) {
            let result = apply_connector_transition(ConnectorState::Ambiguous, non_reconcile_event);
            prop_assert!(result.is_err(), "Ambiguous must reject non-reconciliation events");
        }

        /// INV: Valid transitions follow ADR-041 durability sequence.
        #[test]
        fn valid_transitions_follow_durability_sequence(
            state_idx in 0usize..5,
            event_idx in 0usize..6,
        ) {
            let states = &[ConnectorState::Idle, ConnectorState::Preparing, ConnectorState::Prepared, ConnectorState::Executing, ConnectorState::Ambiguous][..];
            let events = &[ConnectorTransition::Prepare, ConnectorTransition::Prepared, ConnectorTransition::Commit, ConnectorTransition::Succeed, ConnectorTransition::Fail, ConnectorTransition::Ambiguate][..];

            let state = states[state_idx];
            let event = events[event_idx];
            let result = apply_connector_transition(state, event);

            let is_valid_transition = matches!(
                (state, event),
                (ConnectorState::Idle, ConnectorTransition::Prepare) |
                (ConnectorState::Preparing, ConnectorTransition::Prepared) |
                (ConnectorState::Prepared, ConnectorTransition::Commit) |
                (ConnectorState::Executing, ConnectorTransition::Succeed) |
                (ConnectorState::Executing, ConnectorTransition::Fail) |
                (ConnectorState::Executing, ConnectorTransition::Ambiguate) |
                (ConnectorState::Ambiguous, ConnectorTransition::ReconcileSucceeded) |
                (ConnectorState::Ambiguous, ConnectorTransition::ReconcileFailed) |
                (ConnectorState::Ambiguous, ConnectorTransition::ReconcileRetry)
            );

            if is_valid_transition {
                prop_assert!(result.is_ok(), "Valid transition {:?} -> {:?} must succeed", state, event);
            }
        }

        /// INV: Reconciliation transitions only valid from Ambiguous state.
        #[test]
        fn reconciliation_transitions_only_from_ambiguous(
            state_idx in 0usize..7,
        ) {
            let states = ConnectorState::all_variants();
            let state = states[state_idx];

            let reconcile_events = [
                ConnectorTransition::ReconcileSucceeded,
                ConnectorTransition::ReconcileFailed,
                ConnectorTransition::ReconcileRetry,
            ];

            for event in &reconcile_events {
                let result = apply_connector_transition(state, event);
                if state == ConnectorState::Ambiguous {
                    prop_assert!(result.is_ok(), "Ambiguous must accept reconciliation events");
                } else {
                    prop_assert!(result.is_err(), "Non-Ambiguous states must reject reconciliation events");
                }
            }
        }
    }
}
