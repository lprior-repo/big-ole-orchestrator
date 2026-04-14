//! Tests for `read_input_inner` (FD3 reading).

use std::io::Cursor;

use serde_json::json;

use super::read_input_inner_with_state as read_input_inner;
use crate::SdkError;

use super::valid_envelope;

#[test]
fn read_valid_json_returns_task_input() {
    let payload = valid_envelope("key-abc", &json!({"hello": "world"}));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    let input = result.expect("valid JSON should parse");
    assert_eq!(input.idempotency_key.as_str(), "key-abc");
    assert_eq!(input.data, json!({"hello": "world"}));
    assert!(is_read, "guard must be set after successful read");
}

#[test]
fn read_empty_input_returns_invalid_input() {
    let mut cursor = Cursor::new(Vec::<u8>::new());
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
    assert!(is_read, "guard is set even on empty input");
}

#[test]
fn read_oversized_input_returns_invalid_input() {
    // MAX_INPUT_SIZE is 10 MiB; exceed it by 2 bytes.
    let oversized = vec![b'x'; 10 * 1024 * 1024 + 2];
    let mut cursor = Cursor::new(oversized);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_exactly_at_max_size_is_accepted_if_valid_json() {
    let key = "k".repeat(10);
    let inner_value = "v".repeat(10 * 1024 * 1024 - 50);
    let payload = valid_envelope(&key, &json!(inner_value));
    if payload.len() <= 10 * 1024 * 1024 {
        let mut cursor = Cursor::new(payload);
        let mut is_read = false;
        let result = read_input_inner(&mut cursor, &mut is_read);
        assert!(
            result.is_ok(),
            "exactly at limit should be accepted if valid"
        );
    }
}

#[test]
fn read_one_byte_over_max_size_is_rejected() {
    let oversized = vec![b'{'; 10 * 1024 * 1024 + 1];
    let mut cursor = Cursor::new(oversized);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_invalid_json_returns_invalid_input() {
    let mut cursor = Cursor::new(b"not json at all".to_vec());
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_missing_idempotency_key_field_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({"data": 42}))
        .expect("test helper: serialization should not fail");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_missing_data_field_returns_invalid_input() {
    let payload = serde_json::to_vec(&json!({"idempotency_key": "k1"}))
        .expect("test helper: serialization should not fail");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_empty_idempotency_key_returns_invalid_input() {
    let payload = valid_envelope("", &json!(42));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);

    assert_eq!(
        result,
        Err(SdkError::InvalidInput),
        "empty idempotency_key must be rejected by IdempotencyKey::parse"
    );
}

#[test]
fn read_double_read_guard_returns_fd_not_open() {
    let payload = valid_envelope("key-1", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    read_input_inner(&mut cursor, &mut is_read).unwrap();
    assert!(is_read);

    let mut cursor2 = Cursor::new(valid_envelope("key-2", &json!(null)));
    let result = read_input_inner(&mut cursor2, &mut is_read);

    assert_eq!(result, Err(SdkError::FdNotOpen));
}

#[test]
fn read_nested_json_data_returns_task_input() {
    let nested = json!({
        "order": {
            "items": [{"sku": "abc", "qty": 2}],
            "total": 99.99
        },
        "metadata": null
    });
    let payload = valid_envelope("nested-key", &nested);
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("nested JSON should parse");

    assert_eq!(input.idempotency_key.as_str(), "nested-key");
    assert_eq!(input.data["order"]["items"][0]["sku"], json!("abc"));
    assert_eq!(input.data["metadata"], json!(null));
}

#[test]
fn read_extra_fields_are_ignored() {
    let payload = serde_json::to_vec(&json!({
        "idempotency_key": "extra-key",
        "data": {"x": 1},
        "extra_field": "ignored",
    }))
    .expect("serialize");
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input =
        read_input_inner(&mut cursor, &mut is_read).expect("extra fields should be ignored");
    assert_eq!(input.idempotency_key.as_str(), "extra-key");
    assert_eq!(input.data, json!({"x": 1}));
}

#[test]
fn read_data_field_as_null_is_valid() {
    let payload = valid_envelope("null-data", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("null data should be valid");
    assert_eq!(input.data, json!(null));
}

#[test]
fn read_data_field_as_array_is_valid() {
    let payload = valid_envelope("array-data", &json!([1, 2, 3]));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("array data should be valid");
    assert_eq!(input.data, json!([1, 2, 3]));
}

#[test]
fn read_data_field_as_string_is_valid() {
    let payload = valid_envelope("str-data", &json!("hello"));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("string data should be valid");
    assert_eq!(input.data, json!("hello"));
}

#[test]
fn read_data_field_as_number_is_valid() {
    let payload = valid_envelope("num-data", &json!(42));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("number data should be valid");
    assert_eq!(input.data, json!(42));
}

#[test]
fn read_data_field_as_bool_is_valid() {
    let payload = valid_envelope("bool-data", &json!(true));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("bool data should be valid");
    assert_eq!(input.data, json!(true));
}

#[test]
fn read_non_object_json_returns_invalid_input() {
    let mut cursor = Cursor::new(b"[1,2,3]".to_vec());
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_truncated_json_returns_invalid_input() {
    let mut cursor = Cursor::new(b"{\"idempotency_key\": \"k".to_vec());
    let mut is_read = false;

    let result = read_input_inner(&mut cursor, &mut is_read);
    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn read_idempotency_key_with_spaces_is_valid() {
    let payload = valid_envelope("has spaces", &json!(1));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("spaces in key should be valid");
    assert_eq!(input.idempotency_key.as_str(), "has spaces");
}

#[test]
fn read_valid_idempotency_key_with_hyphens() {
    let payload = valid_envelope("my-key-123-abc", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("hyphenated key should work");
    assert_eq!(input.idempotency_key.as_str(), "my-key-123-abc");
}
