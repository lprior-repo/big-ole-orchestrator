//! Property-based tests for receipt key and value encoding (ADR-041).

#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use vo_types::EffectKind;

use super::super::*;

fn arb_effect_kind() -> impl Strategy<Value = EffectKind> {
    prop_oneof![
        Just(EffectKind::HttpCall),
        Just(EffectKind::SqlQuery),
        Just(EffectKind::BlobWrite),
    ]
}

fn arb_nonempty_string(max_len: usize) -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_:.-]{1,}".prop_map(move |s| s.chars().take(max_len).collect())
}

proptest! {
    #[test]
    fn receipt_key_roundtrip(effect_id in arb_nonempty_string(256)) {
        let encoded = encode_receipt_key(&effect_id);
        let decoded = decode_receipt_key(&encoded).unwrap();
        prop_assert_eq!(decoded, effect_id);
    }

    #[test]
    fn receipt_key_is_valid_utf8(effect_id in arb_nonempty_string(256)) {
        let encoded = encode_receipt_key(&effect_id);
        let as_str = std::str::from_utf8(&encoded);
        prop_assert!(as_str.is_ok());
        prop_assert_eq!(as_str.unwrap(), effect_id);
    }

    #[test]
    fn receipt_key_produces_nonempty_bytes(effect_id in arb_nonempty_string(256)) {
        let encoded = encode_receipt_key(&effect_id);
        prop_assert!(!encoded.is_empty());
        prop_assert_eq!(encoded.len(), effect_id.len());
    }

    #[test]
    fn receipt_value_roundtrip(
        effect_id in arb_nonempty_string(128),
        instance_id in arb_nonempty_string(128),
        kind in arb_effect_kind(),
        committed_at_ms in any::<u64>(),
        connector_result in "[a-zA-Z0-9 ]{0,256}",
    ) {
        let receipt = ExecutionReceipt::new(
            effect_id,
            instance_id,
            kind,
            committed_at_ms,
            connector_result,
        )
        .unwrap();
        let encoded = encode_receipt(&receipt).unwrap();
        let decoded = decode_receipt(&encoded).unwrap();
        prop_assert_eq!(decoded, receipt);
    }

    #[test]
    fn receipt_value_is_valid_json(
        effect_id in arb_nonempty_string(64),
        instance_id in arb_nonempty_string(64),
        kind in arb_effect_kind(),
        committed_at_ms in any::<u64>(),
        connector_result in "[a-zA-Z0-9 ]{0,128}",
    ) {
        let receipt = ExecutionReceipt::new(
            effect_id,
            instance_id,
            kind,
            committed_at_ms,
            connector_result,
        )
        .unwrap();
        let encoded = encode_receipt(&receipt).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        prop_assert!(parsed.is_object());
    }

    #[test]
    fn decode_receipt_key_rejects_empty(
        _v in any::<()>()
    ) {
        let result = decode_receipt_key(&[]);
        prop_assert!(result.is_err());
    }
}
