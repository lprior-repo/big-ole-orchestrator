//! Tests for `read_input_inner` (FD3 reading).

use std::io::Cursor;

use serde_json::json;

use crate::read::read_input_inner;
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

    // First read succeeds.
    let _ = read_input_inner(&mut cursor, &mut is_read);
    assert!(is_read);

    // Second read is rejected.
    let mut cursor2 = Cursor::new(valid_envelope("key-2", &json!(null)));
    let result = read_input_inner(&mut cursor2, &mut is_read);

    assert_eq!(result, Err(SdkError::FdNotOpen));
}
