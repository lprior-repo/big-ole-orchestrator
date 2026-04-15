//! Codec round-trip tests for receipt keys and values.

use super::super::*;
use crate::effect_journal::EffectId;
use vo_types::ConnectorResult;
use vo_types::InstanceId;

fn sample_effect_id() -> EffectId {
    EffectId::new(&InstanceId::from_bytes([1u8; 16]), "fx-codec-1").unwrap()
}

#[test]
fn encode_decode_receipt_key_round_trip() {
    let eid = sample_effect_id();
    let encoded = encode_receipt_key(&eid);
    let decoded = decode_receipt_key(&encoded).unwrap();
    assert_eq!(eid, decoded);
}

#[test]
fn decode_receipt_key_rejects_empty_bytes() {
    let result = decode_receipt_key(&[]);
    assert!(result.is_err());
    assert!(matches!(result, Err(ReceiptStoreError::Codec { .. })));
}

#[test]
fn decode_receipt_key_rejects_invalid_utf8() {
    let bad_bytes: Vec<u8> = vec![0xFF, 0xFE, 0xFD];
    let result = decode_receipt_key(&bad_bytes);
    assert!(result.is_err());
    assert!(matches!(result, Err(ReceiptStoreError::Codec { .. })));
}

#[test]
fn encode_decode_receipt_value_round_trip() {
    let receipt = Receipt::new(
        "inst-1::fx-val".to_string(),
        "stripe-conn".to_string(),
        ConnectorResult::Success,
        1713000000,
        Some(serde_json::json!({"charge_id": "ch_abc"})),
    )
    .unwrap();

    let encoded = encode_receipt(&receipt).unwrap();
    let decoded = decode_receipt(&encoded).unwrap();
    assert_eq!(receipt, decoded);
}

#[test]
fn encode_decode_receipt_value_without_payload_round_trip() {
    let receipt = Receipt::new(
        "inst-2::fx-nopayload".to_string(),
        "s3-conn".to_string(),
        ConnectorResult::Failure,
        1713000001,
        None,
    )
    .unwrap();

    let encoded = encode_receipt(&receipt).unwrap();
    let decoded = decode_receipt(&encoded).unwrap();
    assert_eq!(receipt, decoded);
}

#[test]
fn decode_receipt_rejects_garbage_bytes() {
    let garbage = vec![0x00, 0x01, 0x02, 0x03];
    let result = decode_receipt(&garbage);
    assert!(result.is_err());
    assert!(matches!(result, Err(ReceiptStoreError::Codec { .. })));
}

#[test]
fn encode_receipt_key_produces_valid_utf8_key() {
    let eid = sample_effect_id();
    let encoded = encode_receipt_key(&eid);
    let as_str = std::str::from_utf8(&encoded);
    assert!(as_str.is_ok());
    assert_eq!(as_str.unwrap(), eid.as_str());
}
