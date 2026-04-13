//! Tests for `write_success_inner` (FD4 success writing).

use std::io::Write;

use serde_json::{json, Value};

use crate::write::write_success_inner;
use crate::SdkError;

#[test]
fn write_success_valid_output() {
    let output = json!({"result": 42});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    assert_eq!(is_written, true, "guard must be set after write");

    let written: Value = serde_json::from_slice(&buf).expect("written bytes should be valid JSON");
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
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
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
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!("hello world"));
}

#[test]
fn write_success_number_output() {
    let output = json!(42);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!(42));
}

#[test]
fn write_success_bool_output() {
    let output = json!(true);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
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
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
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
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!([1, 2, 3]));
}

#[test]
fn write_success_envelope_has_exactly_two_fields() {
    let output = json!(42);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
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
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!({}));
}

#[test]
fn write_success_unicode_output() {
    let output = json!("日本語テスト");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    assert_eq!(written["output"], json!("日本語テスト"));
}
