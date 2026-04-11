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
            // Must not panic — all 63 combinations handled
            let _ = apply_connector_transition(state, event);
        }
    }
}
