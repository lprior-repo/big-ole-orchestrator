//! Tests for `write_failure_inner` (FD4 failure writing).

use std::io::Write;

use serde_json::Value;

use super::write_failure_inner_with_state as write_failure_inner;
use crate::{SdkError, TaskFailureKind};

#[test]
fn write_failure_user_kind() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(
        &mut buf,
        TaskFailureKind::User,
        "bad input",
        &mut is_written,
    );

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("written bytes should be valid JSON");
    assert_eq!(written["status"], "failure");
    assert_eq!(written["kind"], "User");
    assert_eq!(written["message"], "bad input");
}

#[test]
fn write_failure_system_kind() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(
        &mut buf,
        TaskFailureKind::System,
        "internal error",
        &mut is_written,
    );

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("written bytes should be valid JSON");
    assert_eq!(written["kind"], "System");
    assert_eq!(written["message"], "internal error");
}

#[test]
fn write_failure_timeout_kind() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(
        &mut buf,
        TaskFailureKind::Timeout,
        "timed out",
        &mut is_written,
    );

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("written bytes should be valid JSON");
    assert_eq!(written["kind"], "Timeout");
    assert_eq!(written["message"], "timed out");
}

#[test]
fn write_failure_double_write_returns_already_written() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_failure_inner(&mut buf, TaskFailureKind::User, "first", &mut is_written).unwrap();

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, "second", &mut is_written);

    assert_eq!(result, Err(SdkError::AlreadyWritten));
}

#[test]
fn write_failure_message_too_long_returns_invalid_input() {
    // MAX_MESSAGE_BYTES = 1024; exceed by 1.
    let long_msg = "a".repeat(1025);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, &long_msg, &mut is_written);

    assert_eq!(result, Err(SdkError::InvalidInput));
    assert!(
        is_written,
        "guard is set even when message limit is exceeded"
    );
}

#[test]
fn write_failure_message_exactly_at_limit_succeeds() {
    let exact_msg = "b".repeat(1024);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(
        &mut buf,
        TaskFailureKind::System,
        &exact_msg,
        &mut is_written,
    );

    assert_eq!(result, Ok(()), "1024 bytes should be accepted");
}

#[test]
fn write_failure_multibyte_message_exceeds_byte_limit() {
    // 'e with acute' is 2 bytes in UTF-8: 513 * 2 = 1026 bytes > 1024.
    let multibyte_msg = "\u{00e9}".repeat(513);
    assert!(multibyte_msg.len() > 1024);

    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(
        &mut buf,
        TaskFailureKind::User,
        &multibyte_msg,
        &mut is_written,
    );

    assert_eq!(
        result,
        Err(SdkError::InvalidInput),
        "multibyte message exceeding 1024 bytes must be rejected"
    );
}

#[test]
fn write_failure_empty_message_is_valid() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, "", &mut is_written);

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["message"], "");
}

#[test]
fn write_failure_envelope_has_exactly_three_fields() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_failure_inner(&mut buf, TaskFailureKind::User, "msg", &mut is_written).unwrap();

    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    let obj = written.as_object().expect("should be object");
    assert_eq!(obj.len(), 3, "envelope should have status, kind, message");
    assert!(obj.contains_key("status"));
    assert!(obj.contains_key("kind"));
    assert!(obj.contains_key("message"));
}

#[test]
fn write_failure_unicode_message() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(
        &mut buf,
        TaskFailureKind::System,
        "エラー発生",
        &mut is_written,
    );

    assert_eq!(result, Ok(()));
    let written: Value = serde_json::from_slice(&buf[4..]).expect("valid JSON");
    assert_eq!(written["message"], "エラー発生");
}

#[test]
fn write_failure_multibyte_message_exactly_at_byte_limit() {
    let msg = "\u{00e9}".repeat(512);
    assert_eq!(msg.len(), 1024);

    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::Timeout, &msg, &mut is_written);

    assert_eq!(result, Ok(()), "exactly 1024 bytes should be accepted");
}

#[test]
fn write_failure_guard_set_even_when_writer_fails() {
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

    let result = write_failure_inner(&mut writer, TaskFailureKind::User, "msg", &mut is_written);

    assert_eq!(result, Err(SdkError::WriteError));
    assert!(is_written, "guard must be set even when writer fails");
}

#[test]
fn write_failure_one_byte_over_limit() {
    let msg = "x".repeat(1025);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, &msg, &mut is_written);

    assert_eq!(result, Err(SdkError::InvalidInput));
}

#[test]
fn write_failure_one_byte_under_limit() {
    let msg = "x".repeat(1023);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner(&mut buf, TaskFailureKind::User, &msg, &mut is_written);

    assert_eq!(result, Ok(()));
}
