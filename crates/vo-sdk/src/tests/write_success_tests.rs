//! Tests for `write_success_inner` (FD4 success writing).

use std::io::Write;

use serde_json::{json, Value};

use super::write_success_inner_with_state as write_success_inner;
use crate::SdkError;

#[test]
fn write_success_valid_output() {
    let output = json!({"result": 42});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    assert!(is_written, "guard must be set after write");

    let written: Value = serde_json::from_slice(&buf[4..]).expect("written bytes should be valid JSON");
    assert_eq!(written["status"], "success");
    assert_eq!(written["output"], json!({"result": 42}));
}

#[test]
fn write_success_double_write_returns_already_written() {
    let output = json!("ok");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Err(SdkError::AlreadyWritten));
}

#[test]
fn write_success_oversized_output_returns_write_error() {
    let big_string = "x".repeat(10 * 1024 * 1024 + 1);
    let output = json!(big_string);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard is set even on size-rejection");
}

#[test]
fn write_success_null_output() {
    let output = json!(null);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["status"], "success");
    assert_eq!(written["output"], json!(null));
}

#[test]
fn write_success_string_output() {
    let output = json!("hello world");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"], json!("hello world"));
}

#[test]
fn write_success_number_output() {
    let output = json!(42);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"], json!(42));
}

#[test]
fn write_success_bool_output() {
    let output = json!(true);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"], json!(true));
}

#[test]
fn write_success_nested_object_output() {
    let output = json!({
        "user": {"id": 1, "name": "Alice"},
        "items": [{"sku": "abc", "qty": 2}]
    });
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"]["user"]["name"], json!("Alice"));
    assert_eq!(written["output"]["items"][0]["sku"], json!("abc"));
}

#[test]
fn write_success_array_output() {
    let output = json!([1, 2, 3]);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"], json!([1, 2, 3]));
}

#[test]
fn write_success_envelope_has_exactly_two_fields() {
    let output = json!(42);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    let obj = written.as_object().expect("should be object");
    assert_eq!(
        obj.len(),
        2,
        "envelope should have exactly status and output"
    );
    assert!(obj.contains_key("status"));
    assert!(obj.contains_key("output"));
}

#[test]
fn write_success_guard_set_even_when_writer_fails() {
    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "fail"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "fail"))
        }
    }

    let mut writer = FailingWriter;
    let mut is_written = false;

    let result = write_success_inner(&mut writer, &json!(1), &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard must be set even when writer fails");
}

#[test]
fn write_success_empty_object_output() {
    let output = json!({});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"], json!({}));
}

#[test]
fn write_success_unicode_output() {
    let output = json!("日本語テスト");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["output"], json!("日本語テスト"));
}

#[test]
fn envelope_status_is_string_not_identifier() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &json!(1), &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert!(
        written["status"].is_string(),
        "status must be a JSON string, got {:?}",
        written["status"]
    );
    assert_eq!(written["status"].as_str(), Some("success"));
}

#[test]
fn envelope_output_is_not_mutated_by_serialization() {
    let output = json!({"nested": {"deep": [1, 2, {"x": null}]}});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], output);
}

#[test]
fn envelope_json_is_compact_no_trailing_newline() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &json!(42), &mut is_written).unwrap();

    assert!(
        !buf.ends_with(b"\n"),
        "envelope must not have trailing newline, got {:?}",
        buf.last()
    );
    assert!(
        !buf.ends_with(b"\r"),
        "envelope must not have trailing carriage return"
    );
}

#[test]
fn envelope_roundtrip_preserves_structure() {
    let output = json!({"key": "value", "num": 99, "arr": [true, false, null]});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let parsed: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let reparsed: Value =
        serde_json::from_slice(&serde_json::to_vec(&parsed).unwrap()).expect("roundtrip");
    assert_eq!(parsed, reparsed, "round-trip must be lossless");
}

#[test]
fn envelope_output_with_special_json_characters() {
    let output = json!("contains \"quotes\" and \\backslashes\\ and /slashes/");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(
        written["output"],
        json!("contains \"quotes\" and \\backslashes\\ and /slashes/")
    );
}

#[test]
fn envelope_output_with_newlines_in_string() {
    let output = json!("line1\nline2\ttab\rreturn");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!("line1\nline2\ttab\rreturn"));
}

#[test]
fn envelope_output_with_empty_array() {
    let output = json!([]);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!([]));
    assert!(written["output"].is_array());
    assert_eq!(written["output"].as_array().unwrap().len(), 0);
}

#[test]
fn envelope_output_with_deeply_nested_structure() {
    let output = json!({"a": {"b": {"c": {"d": {"e": "deep"}}}}});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"]["a"]["b"]["c"]["d"]["e"], json!("deep"));
}

#[test]
fn envelope_output_with_mixed_types_in_array() {
    let output = json!([1, "two", true, null, {"key": "val"}, [1, 2]]);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], output);
}

#[test]
fn envelope_output_with_large_number() {
    let output = json!(i64::MAX);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!(i64::MAX));
}

#[test]
fn envelope_output_with_negative_number() {
    let output = json!(-42);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!(-42));
}

#[test]
fn envelope_output_with_float() {
    let output = json!(3.14159);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert!(written["output"].is_f64());
}

#[test]
fn envelope_output_with_false_bool() {
    let output = json!(false);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!(false));
    assert!(written["output"].is_boolean());
}

#[test]
fn envelope_keys_are_exactly_status_and_output() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &json!("x"), &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let keys: std::collections::BTreeSet<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let expected: std::collections::BTreeSet<&str> =
        ["output", "status"].into_iter().collect();
    assert_eq!(keys, expected, "envelope keys must be exactly {{status, output}}");
}

#[test]
fn envelope_output_with_empty_string() {
    let output = json!("");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!(""));
    assert!(written["output"].is_string());
    assert_eq!(written["output"].as_str().unwrap().len(), 0);
}

#[test]
fn envelope_output_with_zero() {
    let output = json!(0);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!(0));
}

#[test]
fn envelope_byte_content_is_valid_utf8() {
    let output = json!("test");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    assert!(
        std::str::from_utf8(&buf).is_ok(),
        "envelope bytes must be valid UTF-8"
    );
}
