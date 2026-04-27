//! Proptest invariants for connector types (feature-gated).

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
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
            terminal_state in proptest::sample::select(&[ConnectorState::Succeeded, ConnectorState::Failed][..]),
            event in proptest::sample::select(ConnectorTransition::all_variants()),
        ) {
            let result = apply_connector_transition(*terminal_state, event);
            prop_assert!(result.is_err());
            if let Err(e) = result {
                prop_assert!(matches!(e, ConnectorTransitionError::TerminalStateTransition));
            }
        }

        /// INV: Ambiguous state only accepts reconciliation transitions.
        #[test]
        fn ambiguous_state_accepts_only_reconciliation_events(
            event in proptest::sample::select(ConnectorTransition::all_variants()),
        ) {
            let result = apply_connector_transition(ConnectorState::Ambiguous, event);
            match event {
                ConnectorTransition::ReconcileSucceeded
                | ConnectorTransition::ReconcileFailed
                | ConnectorTransition::ReconcileRetry => {
                    prop_assert!(result.is_ok(), "Reconciliation events should be accepted");
                }
                _ => {
                    prop_assert!(result.is_err(), "Non-reconciliation events should be rejected");
                }
            }
        }
    }
}
