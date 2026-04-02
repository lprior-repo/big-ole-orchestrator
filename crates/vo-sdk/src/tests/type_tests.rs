//! Tests for domain types: TaskInput, TaskFailureKind, SdkError.

use std::io::Cursor;

use serde_json::json;

use crate::read::read_input_inner;
use crate::{SdkError, TaskFailureKind};

use super::valid_envelope;

// ---------------------------------------------------------------------------
// TaskInput::idempotency_key() accessor
// ---------------------------------------------------------------------------

#[test]
fn task_input_idempotency_key_accessor() {
    let payload = valid_envelope("my-key-123", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("valid input");

    assert_eq!(input.idempotency_key().as_str(), "my-key-123");
}

// ---------------------------------------------------------------------------
// TaskFailureKind::as_str()
// ---------------------------------------------------------------------------

#[test]
fn task_failure_kind_as_str_user() {
    assert_eq!(TaskFailureKind::User.as_str(), "User");
}

#[test]
fn task_failure_kind_as_str_system() {
    assert_eq!(TaskFailureKind::System.as_str(), "System");
}

#[test]
fn task_failure_kind_as_str_timeout() {
    assert_eq!(TaskFailureKind::Timeout.as_str(), "Timeout");
}

// ---------------------------------------------------------------------------
// SdkError Display impl
// ---------------------------------------------------------------------------

#[test]
fn sdk_error_display_matches_debug() {
    // The Display impl uses `{self:?}`, so Display == Debug for all variants.
    let variants = [
        SdkError::InvalidInput,
        SdkError::FdNotOpen,
        SdkError::AlreadyWritten,
        SdkError::WriteError,
    ];

    for err in &variants {
        assert_eq!(
            format!("{err}"),
            format!("{err:?}"),
            "Display and Debug must match for {err:?}"
        );
    }
}

#[test]
fn sdk_error_display_invalid_input() {
    assert_eq!(SdkError::InvalidInput.to_string(), "InvalidInput");
}

#[test]
fn sdk_error_display_fd_not_open() {
    assert_eq!(SdkError::FdNotOpen.to_string(), "FdNotOpen");
}

#[test]
fn sdk_error_display_already_written() {
    assert_eq!(SdkError::AlreadyWritten.to_string(), "AlreadyWritten");
}

#[test]
fn sdk_error_display_write_error() {
    assert_eq!(SdkError::WriteError.to_string(), "WriteError");
}
