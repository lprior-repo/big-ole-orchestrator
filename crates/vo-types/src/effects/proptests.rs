//! Proptest invariants for effect types (ADR-030).

use super::transitions::*;
use super::types::*;

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
