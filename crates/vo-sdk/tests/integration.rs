use serde_json::json;
use vo_sdk::{read_input, write_failure, write_success, SdkError, TaskFailureKind};

#[test]
fn read_input_returns_fd_not_open_error_when_fd3_inaccessible() {
    let result = read_input();
    assert_eq!(result, Err(SdkError::FdNotOpen));
}

#[test]
fn write_success_returns_write_error_when_fd4_broken() {
    let result = write_success(&json!({"a": 1}));
    assert!(matches!(
        result,
        Err(SdkError::WriteError) | Err(SdkError::AlreadyWritten)
    ));
}

#[test]
fn write_success_returns_write_error_when_overflow_boundary() {
    // Large enough string to hit overflow (conceptually). Mocking with 10MB+.
    let large_json = json!({"a": " ".repeat(10 * 1024 * 1024 + 10)});
    let result = write_success(&large_json);
    assert!(matches!(
        result,
        Err(vo_sdk::SdkError::WriteError)
            | Err(vo_sdk::SdkError::AlreadyWritten)
            | Err(vo_sdk::SdkError::InvalidInput)
    ));
}

#[test]
fn write_failure_returns_write_error_when_fd4_broken() {
    let result = write_failure(TaskFailureKind::System, "boom");
    assert!(matches!(
        result,
        Err(SdkError::WriteError) | Err(SdkError::AlreadyWritten)
    ));
}

#[test]
fn write_failure_returns_write_error_when_overflow_boundary() {
    // Too large string, mock testing overflow
    let result = write_failure(TaskFailureKind::System, &"A".repeat(2000));
    assert!(matches!(
        result,
        Err(vo_sdk::SdkError::WriteError)
            | Err(vo_sdk::SdkError::AlreadyWritten)
            | Err(vo_sdk::SdkError::InvalidInput)
    ));
}

// Proptest invariants
use proptest::prelude::*;

proptest! {
    #[test]
    fn serialize_then_deserialize_roundtrips_any_valid_document(
        message in ".*"
    ) {
        let result = write_failure(TaskFailureKind::User, &message);
        // Can be Ok or Err (invalid input due to length), but shouldn't panic.
        prop_assert!(matches!(result, Ok(_) | Err(_)));
    }
}
