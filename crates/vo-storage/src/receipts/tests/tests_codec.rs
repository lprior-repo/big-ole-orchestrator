//! Codec round-trip tests for receipt keys and values.

use super::super::*;
use vo_types::EffectKind;

#[test]
fn encode_decode_receipt_key_round_trip() {
    let effect_id = "inst-1::fx-codec-1";
    let encoded = encode_receipt_key(effect_id);
    let decoded = decode_receipt_key(&encoded).unwrap();
    assert_eq!(effect_id, decoded);
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
    let receipt = ExecutionReceipt::new(
        "inst-1::fx-val".to_string(),
        "stripe-conn".to_string(),
        EffectKind::HttpCall,
        1713000000,
        "Success".to_string(),
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
    let effect_id = "inst-1::fx-key-test";
    let encoded = encode_receipt_key(effect_id);
    let as_str = std::str::from_utf8(&encoded);
    assert!(as_str.is_ok());
    assert_eq!(as_str.unwrap(), effect_id);
}
