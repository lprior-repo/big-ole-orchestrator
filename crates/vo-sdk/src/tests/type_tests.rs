//! Tests for domain types: TaskInput, TaskFailureKind, SdkError.

use std::io::Cursor;

use serde_json::json;

use crate::tests::read_input_inner_with_state as read_input_inner;
use crate::{SdkError, TaskFailureKind};

use super::valid_envelope;

#[test]
fn task_input_idempotency_key_accessor() {
    let payload = valid_envelope("my-key-123", &json!(null));
    let mut cursor = Cursor::new(payload);
    let mut is_read = false;

    let input = read_input_inner(&mut cursor, &mut is_read).expect("valid input");

    assert_eq!(input.idempotency_key().as_str(), "my-key-123");
}

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

#[test]
fn sdk_error_display_matches_debug() {
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

#[test]
fn sdk_error_is_std_error() {
    let _: &dyn std::error::Error = &SdkError::InvalidInput;
    let _: &dyn std::error::Error = &SdkError::FdNotOpen;
    let _: &dyn std::error::Error = &SdkError::AlreadyWritten;
    let _: &dyn std::error::Error = &SdkError::WriteError;
}

#[test]
fn sdk_error_all_variants_are_distinct() {
    let variants = [
        SdkError::InvalidInput,
        SdkError::FdNotOpen,
        SdkError::AlreadyWritten,
        SdkError::WriteError,
    ];
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            assert_ne!(variants[i], variants[j], "all variants must be distinct");
        }
    }
}

#[test]
fn sdk_error_partial_eq() {
    assert_eq!(SdkError::InvalidInput, SdkError::InvalidInput);
    assert_ne!(SdkError::InvalidInput, SdkError::FdNotOpen);
}

#[test]
fn task_failure_kind_debug_format() {
    assert_eq!(format!("{:?}", TaskFailureKind::User), "User");
    assert_eq!(format!("{:?}", TaskFailureKind::System), "System");
    assert_eq!(format!("{:?}", TaskFailureKind::Timeout), "Timeout");
}

#[test]
fn task_failure_kind_partial_eq() {
    assert_eq!(TaskFailureKind::User, TaskFailureKind::User);
    assert_ne!(TaskFailureKind::User, TaskFailureKind::System);
    assert_ne!(TaskFailureKind::System, TaskFailureKind::Timeout);
}

#[test]
fn task_failure_kind_clone() {
    let kind = TaskFailureKind::User;
    let cloned = kind;
    assert_eq!(kind, cloned);
}

#[test]
fn task_failure_kind_copy() {
    let kind = TaskFailureKind::System;
    let copied = kind;
    assert_eq!(kind, copied);
}

#[test]
fn task_failure_kind_exhaustive_match() {
    let kinds = [
        TaskFailureKind::User,
        TaskFailureKind::System,
        TaskFailureKind::Timeout,
    ];
    let mut strs = std::collections::HashSet::new();
    for k in &kinds {
        strs.insert(k.as_str());
    }
    assert_eq!(
        strs.len(),
        3,
        "all three kinds should produce distinct strings"
    );
    assert!(strs.contains("User"));
    assert!(strs.contains("System"));
    assert!(strs.contains("Timeout"));
}
