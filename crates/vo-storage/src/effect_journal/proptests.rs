#![allow(clippy::unwrap_used)]
use super::*;
use proptest::prelude::*;

proptest! {
    /// INV-EJ-PROP-001: EffectId serde round-trip preserves equality.
    #[test]
    fn effectid_serde_roundtrip_preserves_equality(
        intent_id in "[a-zA-Z0-9_-]{1,100}"
    ) {
        let iid = InstanceId::from_bytes([1u8; 16]);
        let eid = EffectId::new(&iid, &intent_id).unwrap();
        let json = serde_json::to_string(&eid).unwrap();
        let recovered: EffectId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(eid, recovered);
    }

    /// INV-EJ-PROP-002: encode/decode key round-trip.
    #[test]
    fn encode_decode_key_roundtrip(
        intent_id in "[a-zA-Z0-9_-]{1,100}"
    ) {
        let iid = InstanceId::from_bytes([1u8; 16]);
        let eid = EffectId::new(&iid, &intent_id).unwrap();
        let bytes = encode_effect_key(&eid);
        let recovered = decode_effect_key(&bytes).unwrap();
        prop_assert_eq!(eid, recovered);
    }

    /// INV-EJ-PROP-003: encode/decode record round-trip for all kinds/statuses.
    #[test]
    fn encode_decode_record_roundtrip(
        intent_id in "[a-zA-Z0-9_-]{1,100}",
        kind_idx in 0usize..3,
        status_idx in 0usize..3,
    ) {
        let kinds = [EffectKind::HttpCall, EffectKind::SqlQuery, EffectKind::BlobWrite];
        let statuses = [
            EffectIntent::Prepared,
            EffectIntent::Committed,
            EffectIntent::RolledBack,
        ];
        let record = EffectRecord::new(
            intent_id,
            kinds[kind_idx],
            serde_json::json!({"test": true}),
            statuses[status_idx],
            None,
        )
        .unwrap();
        let bytes = encode_effect_record(&record).unwrap();
        let recovered = decode_effect_record(&bytes).unwrap();
        prop_assert_eq!(record, recovered);
    }

    /// INV-EJ-PROP-004: Different intent_ids produce different EffectIds.
    #[test]
    fn effectid_injectivity(
        a in "[a-zA-Z0-9]{1,50}",
        b in "[a-zA-Z0-9]{1,50}"
    ) {
        if a != b {
            let iid = InstanceId::from_bytes([1u8; 16]);
            let ea = EffectId::new(&iid, &a).unwrap();
            let eb = EffectId::new(&iid, &b).unwrap();
            prop_assert_ne!(ea, eb);
        }
    }
}
