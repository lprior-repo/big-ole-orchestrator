//! Proptest invariants for transaction coordinator types.

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;

    proptest::proptest! {
        /// INV: Serde round-trip preserves TransactionState equality for all variants.
        #[test]
        fn transaction_state_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(TransactionState::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: TransactionState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves ParticipantStatus equality for all variants.
        #[test]
        fn participant_status_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(ParticipantStatus::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: ParticipantStatus = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves CoordinatorDecision equality for all variants.
        #[test]
        fn coordinator_decision_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(CoordinatorDecision::all_variants())
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: CoordinatorDecision = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: TransactionRecord::new rejects empty transaction_id.
        #[test]
        fn transaction_record_rejects_empty_transaction_id(
            state in proptest::sample::select(TransactionState::all_variants())
        ) {
            let result = TransactionRecord::new(
                String::new(),
                state,
                None,
                vec![],
                None,
                None,
                None,
            );
            prop_assert!(result.is_none());
        }

        /// INV: ParticipantRecord::new rejects empty participant_id.
        #[test]
        fn participant_record_rejects_empty_participant_id(
            status in proptest::sample::select(ParticipantStatus::all_variants())
        ) {
            let result = ParticipantRecord::new(
                String::new(),
                status,
                None,
            );
            prop_assert!(result.is_none());
        }

        /// INV: apply_coordinator_transition never panics — all valid transitions handled.
        #[test]
        fn apply_coordinator_transition_never_panics(
            state_idx in 0usize..10,
            event_idx in 0usize..12,
        ) {
            let states = TransactionState::all_variants();
            let events = CoordinatorTransition::all_variants();
            let current = states[state_idx % states.len()];
            let evt = events[event_idx % events.len()];

            // Must not panic — all combinations handled
            let _ = apply_coordinator_transition(current, evt);
        }
    }
}
