//! Receipt type construction and validation tests.

use super::super::*;
use vo_types::ConnectorResult;

#[test]
fn receipt_constructs_with_valid_fields() {
    let receipt = Receipt::new(
        "inst-1::fx-1".to_string(),
        "stripe-connector".to_string(),
        ConnectorResult::Success,
        1713000000,
        Some(serde_json::json!({"charge_id": "ch_123"})),
    )
    .unwrap();

    assert_eq!(receipt.effect_id(), "inst-1::fx-1");
    assert_eq!(receipt.connector_id(), "stripe-connector");
    assert_eq!(receipt.result(), ConnectorResult::Success);
    assert_eq!(receipt.committed_at_ms(), 1713000000);
    assert_eq!(
        receipt.payload_json().cloned(), Some(serde_json::json!({"charge_id": "ch_123"}))
    );
}

#[test]
fn receipt_constructs_without_payload() {
    let receipt = Receipt::new(
        "inst-2::fx-2".to_string(),
        "s3-connector".to_string(),
        ConnectorResult::Success,
        1713000001,
        None,
    )
    .unwrap();

    assert_eq!(receipt.effect_id(), "inst-2::fx-2");
    assert_eq!(receipt.connector_id(), "s3-connector");
    assert!(receipt.payload_json().is_none());
}

#[test]
fn receipt_rejects_empty_effect_id() {
    let result = Receipt::new(
        "".to_string(),
        "conn".to_string(),
        ConnectorResult::Success,
        0,
        None,
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn receipt_rejects_empty_connector_id() {
    let result = Receipt::new(
        "fx-1".to_string(),
        "".to_string(),
        ConnectorResult::Success,
        0,
        None,
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn receipt_rejects_both_empty() {
    let result = Receipt::new(
        "".to_string(),
        "".to_string(),
        ConnectorResult::Success,
        0,
        None,
    );
    assert_eq!(result, Err(ReceiptStoreError::InvalidArgument));
}

#[test]
fn receipt_allows_ambiguous_result() {
    let receipt = Receipt::new(
        "inst-3::fx-3".to_string(),
        "http-connector".to_string(),
        ConnectorResult::Ambiguous,
        1713000002,
        None,
    )
    .unwrap();

    assert_eq!(receipt.result(), ConnectorResult::Ambiguous);
}

#[test]
fn receipt_allows_failure_result() {
    let receipt = Receipt::new(
        "inst-4::fx-4".to_string(),
        "sql-connector".to_string(),
        ConnectorResult::Failure,
        1713000003,
        Some(serde_json::json!({"error": "constraint violation"})),
    )
    .unwrap();

    assert_eq!(receipt.result(), ConnectorResult::Failure);
    assert_eq!(
        receipt.payload_json().cloned(), Some(serde_json::json!({"error": "constraint violation"}))
    );
}

#[test]
fn receipt_is_serde_round_trip() {
    let receipt = Receipt::new(
        "inst-5::fx-5".to_string(),
        "kafka-connector".to_string(),
        ConnectorResult::Success,
        1713000004,
        Some(serde_json::json!({"offset": 42})),
    )
    .unwrap();

    let json = serde_json::to_string(&receipt).unwrap();
    let decoded: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(receipt, decoded);
}
