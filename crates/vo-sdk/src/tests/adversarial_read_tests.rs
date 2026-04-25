//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: read_input_inner boundary & adversarial tests.

use std::io::Cursor;

use serde_json::{json, Value};

use crate::tests::{
    read_input_inner_with_state as read_input_inner,
    write_failure_inner_with_state as write_failure_inner,
    write_success_inner_with_state as write_success_inner,
};
use crate::{SdkError, TaskFailureKind};

use super::valid_envelope;

#[test]
fn read_non_utf8_input_returns_invalid_input() {
    let raw = vec![0xFF, 0xFE, 0xFD];
    let mut cursor = Cursor::new(raw);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
    assert!(is_read, "guard must be set even for non-UTF-8 input");
}

#[test]
fn read_whitespace_in_idempotency_key_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "has spaces",
        "data": null
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_special_chars_in_idempotency_key_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "key!@#$%",
        "data": null
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_numeric_idempotency_key_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "12345",
        "data": null
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_accepts_any_json_value_as_data() {
    for data_val in [
        json!(null),
        json!([]),
        json!({"nested": {"deep": true}}),
        json!(42),
        json!("str"),
    ] {
        let payload = serde_json::to_vec(&json!({
            "idempotency_key": "key-ok",
            "data": data_val
        }))
        .expect("serialize");
        let mut cursor = Cursor::new(payload);
        let mut is_read = false;

        let result = read_input_inner(&mut cursor, &mut is_read);

        let input = result.expect("any valid JSON value should be accepted as data");
        assert_eq!(input.data, data_val);
    }
}

#[test]
fn read_at_max_input_size_boundary_succeeds() {
    let data = "x".repeat(10 * 1024 * 1024 - 200);
    let payload = valid_envelope("boundary-key", &json!({"big": data}));
    assert!(
        payload.len() <= 10 * 1024 * 1024,
        "payload must be at or under limit"
    );

    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert!(result.is_ok(), "input at max size should succeed");
    assert!(is_read);
}

#[test]
fn read_one_byte_over_max_input_size_returns_invalid_input() {
    let data = "x".repeat(10 * 1024 * 1024);
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "overflow-key",
        "data": data
    }))
    .expect("serialize");
    assert!(
        payload.len() > 10 * 1024 * 1024,
        "payload must exceed limit"
    );

    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_failed_parse_still_sets_guard() {
    let payload = b"{not valid json".to_vec();
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
    assert!(is_read, "guard must be set before parse attempt");
}

#[test]
fn read_partial_json_truncated_returns_invalid_input() {
    let payload = b"{\"idempotency_key\": \"k\", \"data\": ".to_vec();
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}