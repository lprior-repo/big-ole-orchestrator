//! Tests for `write_success_inner` (FD4 success writing).

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
    // Build a JSON value that serializes to > 10 MiB.
    let big_string = "x".repeat(10 * 1024 * 1024 + 1);
    let output = json!(big_string);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard is set even on size-rejection");
}
