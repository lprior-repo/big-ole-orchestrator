//! Adversarial tests for vo-sdk (bead ve-z32z).
//!
//! DIMENSION: write_success_inner and write_failure_inner envelope & adversarial tests.

use std::io::{Cursor, Write};

use serde_json::{json, Value};

use crate::tests::{
    write_failure_inner_with_state as write_failure_inner,
    write_success_inner_with_state as write_success_inner,
};
use crate::{SdkError, TaskFailureKind};

#[test]
fn write_success_envelope_has_exact_keys() {
    let output = json!({"result": 42});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let keys: Vec<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        keys,
        vec!["output", "status"],
        "only status and output keys expected"
    );
}

#[test]
fn write_success_accepts_nested_json_output() {
    let output = json!({"users": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}], "total": 2});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_success_inner(&mut buf, &output, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["output"], output);
}

#[test]
fn write_success_io_failure_returns_write_error_and_sets_guard() {
    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = BrokenWriter;
    let mut is_written = false;

    let result = write_success_inner(&mut writer, &json!("ok"), &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard set before I/O attempt");
}

#[test]
fn write_failure_envelope_has_exact_keys() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_failure_inner(&mut buf, TaskFailureKind::User, "err", &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf).expect("valid JSON");
    let keys: Vec<&str> = written
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        keys,
        vec!["kind", "message", "status"],
        "only status, kind, message keys expected"
    );
}

#[test]
fn write_failure_empty_message_succeeds() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::System, "", &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "");
}

#[test]
fn write_failure_newline_in_message_succeeds() {
    let msg = "line1\nline2\nline3";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, msg, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "line1\nline2\nline3");
}

#[test]
fn write_failure_null_byte_in_message_succeeds() {
    let msg = "before\0after";
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, msg, &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(written["message"], "before\0after");
}

#[test]
fn write_failure_io_failure_returns_write_error_and_sets_guard() {
    struct BrokenWriter;
    impl Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "broken",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = BrokenWriter;
    let mut is_written = false;

    let result = write_failure_inner(&mut writer, TaskFailureKind::User, "msg", &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard set before I/O attempt");
}

#[test]
fn write_failure_after_success_returns_already_written() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner(&mut buf, &json!("ok"), &mut is_written).unwrap();

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, "err", &mut is_written);

    assert_eq!(result, Err(SdkError::AlreadyWritten));
}

#[test]
fn write_success_after_failure_returns_already_written() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_failure_inner(&mut buf, TaskFailureKind::User, "err", &mut is_written).unwrap();

    let result = write_success_inner(&mut buf, &json!("ok"), &mut is_written);

    assert_eq!(result, Err(SdkError::AlreadyWritten));
}