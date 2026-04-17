//! Receipt type construction and validation tests.

use super::super::*;
use vo_types::EffectKind;

#[test]
fn receipt_constructs_with_valid_fields() {
    let receipt = ExecutionReceipt::new(
        "inst-1::fx-1".to_string(),
        "stripe-connector".to_string(),
        EffectKind::HttpCall,
        1713000000,
        "Success".to_string(),
    )
    .unwrap();

    assert_eq!(receipt.effect_id(), "inst-1::fx-1");
    assert_eq!(receipt.instance_id(), "stripe-connector");
    assert_eq!(receipt.kind(), EffectKind::HttpCall);
    assert_eq!(receipt.committed_at_ms(), 1713000000);
}

#[test]
fn receipt_constructs_without_payload() {
    let receipt = ExecutionReceipt::new(
        "inst-2::fx-2".to_string(),
        "s3-connector".to_string(),
        EffectKind::HttpCall,
        1713000001,
        "Success".to_string(),
    )
    .unwrap();

    assert_eq!(receipt.effect_id(), "inst-2::fx-2");
    assert_eq!(receipt.instance_id(), "s3-connector");
}

#[test]
fn receipt_rejects_empty_effect_id() {
    let result = ExecutionReceipt::new(
        "".to_string(),
        "conn".to_string(),
        EffectKind::HttpCall,
        0,
        "Success".to_string(),
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn receipt_rejects_empty_instance_id() {
    let result = ExecutionReceipt::new(
        "fx-1".to_string(),
        "".to_string(),
        EffectKind::HttpCall,
        0,
        "Success".to_string(),
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn receipt_rejects_both_empty() {
    let result = ExecutionReceipt::new(
        "".to_string(),
        "".to_string(),
        EffectKind::HttpCall,
        0,
        "Success".to_string(),
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn receipt_is_serde_round_trip() {
    let receipt = ExecutionReceipt::new(
        "inst-5::fx-5".to_string(),
        "kafka-connector".to_string(),
        EffectKind::HttpCall,
        1713000004,
        "Success".to_string(),
    )
    .unwrap();

    let json = serde_json::to_string(&receipt).unwrap();
    let decoded: ExecutionReceipt = serde_json::from_str(&json).unwrap();
    assert_eq!(receipt, decoded);
}
