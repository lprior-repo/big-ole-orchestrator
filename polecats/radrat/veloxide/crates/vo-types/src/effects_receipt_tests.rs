//! Tests for the Receipt type (ADR-041 §4: Receipts and Identity).

#[test]
fn receipt_new_returns_some_with_valid_inputs() {
    let ts = crate::types::TimestampMs(1700000000);
    let receipt = crate::effects::Receipt::new(
        "inst-1::fx-123".to_string(),
        "stripe".to_string(),
        "v1.2.0".to_string(),
        serde_json::json!({"charge_id": "ch_abc"}),
        ts,
    );
    assert!(receipt.is_some());
}

#[test]
fn receipt_new_returns_none_when_effect_id_is_empty() {
    let result = crate::effects::Receipt::new(
        String::new(),
        "stripe".to_string(),
        "v1".to_string(),
        serde_json::json!({}),
        crate::types::TimestampMs(0),
    );
    assert!(result.is_none());
}
