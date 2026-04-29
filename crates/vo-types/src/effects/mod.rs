//! Managed effect types for exact-once side effects (ADR-030).
//!
//! Architecture: Data (EffectIntent, EffectKind, EffectRecord, CompensationPolicy)
//!             → Calc (apply_effect_transition, is_terminal, all_variants).
//!
//! This module defines the type system for managed effects flowing through the Engine.
//! No I/O, no engine integration — pure types and state machine logic.

pub mod compensation;
pub mod intent;
pub mod lifecycle;

pub use compensation::CompensationPolicy;
pub use intent::{
    apply_effect_transition, EffectIntent, EffectTransitionError, EffectTransitionEvent,
};
pub use lifecycle::{
    validate_effect_against_schema, EffectCompressionError, EffectKind, EffectRecord,
    EffectValidationError, JsonType, Receipt, StepSchema,
};

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;

    proptest::proptest! {
        /// INV: Serde round-trip preserves EffectIntent equality for all variants.
        #[test]
        fn effectintent_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(&[
                EffectIntent::Prepared,
                EffectIntent::Committed,
                EffectIntent::RolledBack,
            ])
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: EffectIntent = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves EffectKind equality for all variants.
        #[test]
        fn effectkind_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(&[
                EffectKind::HttpCall,
                EffectKind::SqlQuery,
                EffectKind::BlobWrite,
            ])
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: EffectKind = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: EffectRecord field immutability — accessors return construction values.
        #[test]
        fn effectrecord_accessors_return_construction_values(
            id in "[a-zA-Z0-9_-]{1,100}",
            kind_idx in 0usize..3,
            status_idx in 0usize..3,
        ) {
            let kinds = [EffectKind::HttpCall, EffectKind::SqlQuery, EffectKind::BlobWrite];
            let statuses = [EffectIntent::Prepared, EffectIntent::Committed, EffectIntent::RolledBack];
            let kind = kinds[kind_idx];
            let status = statuses[status_idx];
            let params = serde_json::json!({"test": "value"});
            let ts = crate::types::TimestampMs(42);

            let record = EffectRecord::new(id.clone(), kind, params.clone(), status, Some(ts));
            prop_assert!(record.is_some());
            let r = record.unwrap();
            prop_assert_eq!(r.intent_id(), id);
            prop_assert_eq!(r.kind(), kind);
            prop_assert_eq!(r.status(), status);
        }
    }
}

#[cfg(kani)]
mod verification {
    use super::*;

    /// K-01: Verify apply_effect_transition exhaustiveness.
    /// All 3×2 = 6 combinations must be covered without panic.
    #[kani::proof]
    fn verify_effect_transition_exhaustiveness() {
        let state: u8 = kani::any();
        let event: u8 = kani::any();
        kani::assume(state < 3);
        kani::assume(event < 2);

        let current = match state {
            0 => EffectIntent::Prepared,
            1 => EffectIntent::Committed,
            _ => EffectIntent::RolledBack,
        };
        let evt = match event {
            0 => EffectTransitionEvent::Commit,
            _ => EffectTransitionEvent::Rollback,
        };

        let _ = apply_effect_transition(current, evt);
    }

    /// K-02: Verify EffectRecord::new rejects empty intent_id.
    #[kani::proof]
    fn verify_effect_record_rejects_empty_intent_id() {
        let intent_id = String::new();
        let result = EffectRecord::new(
            intent_id,
            EffectKind::HttpCall,
            serde_json::Value::Null,
            EffectIntent::Prepared,
            None,
        );
        assert!(result.is_none());
    }
}
