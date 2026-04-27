//! Integration tests for vo-sdk I/O through FD3/FD4.
//!
//! These tests exercise the public API (`read_input`, `write_success`, `write_failure`)
//! through the actual FD-based code paths.
//!
//! Note: Full FD mocking is complex due to Rust File ownership and atomic guards.
//! Tests that require setting up FD3/FD4 with dup2 spawn a subprocess helper binary
//! because the SDK uses process-level static guards (IS_READ/IS_WRITTEN) that can only
//! be triggered once per process. The existing unit tests (329 tests in --lib) provide
//! comprehensive coverage of the inner parsing/logic via the `*_inner_with_state` variants.

mod fd_mock;

use fd_mock::{create_invalid_json_envelope, create_missing_key_envelope, create_valid_envelope};
use serde_json::json;
use vo_sdk::{SdkError, TaskFailureKind};

#[test]
fn read_input_inner_with_state_valid_json_succeeds() {
    use vo_sdk::io::read_input_inner_with_state;

    let data = create_valid_envelope("test-key", &json!({"a": 1}));
    let mut reader = data.as_slice();
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut reader, &mut is_read);
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let input = result.unwrap();
    assert_eq!(input.idempotency_key.as_str(), "test-key");
    assert_eq!(input.data, json!({"a": 1}));
}

#[test]
fn read_input_inner_with_state_invalid_json_fails() {
    use vo_sdk::io::read_input_inner_with_state;

    let data = create_invalid_json_envelope();
    let mut reader = data.as_slice();
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut reader, &mut is_read);
    assert!(
        matches!(result, Err(SdkError::InvalidInput)),
        "Expected InvalidInput, got {:?}",
        result
    );
}

#[test]
fn read_input_inner_with_state_missing_key_fails() {
    use vo_sdk::io::read_input_inner_with_state;

    let data = create_missing_key_envelope();
    let mut reader = data.as_slice();
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut reader, &mut is_read);
    assert!(
        matches!(result, Err(SdkError::InvalidInput)),
        "Expected InvalidInput for missing idempotency_key, got {:?}",
        result
    );
}

#[test]
fn read_input_inner_with_state_double_read_blocked() {
    use vo_sdk::io::read_input_inner_with_state;

    let data = create_valid_envelope("test-key", &json!({"a": 1}));
    let mut reader = data.as_slice();
    let mut is_read = false;

    let first = read_input_inner_with_state(&mut reader, &mut is_read);
    assert!(first.is_ok(), "First read should succeed");

    let second_data = create_valid_envelope("test-key-2", &json!({"b": 2}));
    let mut second_reader = second_data.as_slice();
    let second = read_input_inner_with_state(&mut second_reader, &mut is_read);
    assert!(
        matches!(second, Err(SdkError::FdNotOpen)),
        "Second read should fail with FdNotOpen, got {:?}",
        second
    );
}

#[test]
fn write_success_inner_with_state_succeeds() {
    use vo_sdk::io::write_success_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;
    let output = json!({"result": "ok", "value": 42});

    let result = write_success_inner_with_state(&mut buf, &output, &mut is_written);
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);

    let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["output"], output);
}

#[test]
fn write_success_inner_with_state_double_write_blocked() {
    use vo_sdk::io::write_success_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;

    let first = write_success_inner_with_state(&mut buf, &json!({"first": true}), &mut is_written);
    assert!(first.is_ok(), "First write should succeed");

    let second =
        write_success_inner_with_state(&mut buf, &json!({"second": true}), &mut is_written);
    assert!(
        matches!(second, Err(SdkError::AlreadyWritten)),
        "Second write should fail with AlreadyWritten, got {:?}",
        second
    );
}

#[test]
fn write_failure_inner_with_state_succeeds() {
    use vo_sdk::io::write_failure_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;

    let result = write_failure_inner_with_state(
        &mut buf,
        TaskFailureKind::User,
        "test error",
        &mut is_written,
    );
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);

    let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(parsed["status"], "failure");
    assert_eq!(parsed["kind"], "User");
    assert_eq!(parsed["message"], "test error");
}

#[test]
fn write_failure_inner_with_state_double_write_blocked() {
    use vo_sdk::io::write_failure_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;

    let first =
        write_failure_inner_with_state(&mut buf, TaskFailureKind::User, "first", &mut is_written);
    assert!(first.is_ok(), "First write should succeed");

    let second =
        write_failure_inner_with_state(&mut buf, TaskFailureKind::User, "second", &mut is_written);
    assert!(
        matches!(second, Err(SdkError::AlreadyWritten)),
        "Second write should fail with AlreadyWritten, got {:?}",
        second
    );
}

#[test]
fn write_failure_rejects_message_exceeding_1024_bytes() {
    use vo_sdk::io::write_failure_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;
    let long_message = "x".repeat(1025);

    let result = write_failure_inner_with_state(
        &mut buf,
        TaskFailureKind::User,
        &long_message,
        &mut is_written,
    );
    assert!(
        matches!(result, Err(SdkError::InvalidInput)),
        "Expected InvalidInput for message > 1024 bytes, got {:?}",
        result
    );
}

#[test]
fn write_failure_accepts_message_at_1024_bytes() {
    use vo_sdk::io::write_failure_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;
    let message_at_limit = "x".repeat(1024);

    let result = write_failure_inner_with_state(
        &mut buf,
        TaskFailureKind::User,
        &message_at_limit,
        &mut is_written,
    );
    assert!(
        result.is_ok(),
        "Expected Ok at exactly 1024 bytes, got {:?}",
        result
    );
}

#[test]
fn write_success_rejects_output_exceeding_10mb() {
    use vo_sdk::io::write_success_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;
    let large_output = json!({"data": "x".repeat(10 * 1024 * 1024 + 1)});

    let result = write_success_inner_with_state(&mut buf, &large_output, &mut is_written);
    assert!(
        matches!(result, Err(SdkError::WriteError)),
        "Expected WriteError for output > 10MB, got {:?}",
        result
    );
}

#[test]
fn write_success_accepts_large_output() {
    use vo_sdk::io::write_success_inner_with_state;

    let mut buf = Vec::new();
    let mut is_written = false;
    let large_output = json!({"data": "x".repeat(1024 * 1024)});

    let result = write_success_inner_with_state(&mut buf, &large_output, &mut is_written);
    assert!(
        result.is_ok(),
        "Expected Ok for 1MB output, got {:?}",
        result
    );
}

#[test]
fn empty_input_returns_invalid_input_via_inner() {
    use vo_sdk::io::read_input_inner_with_state;

    let mut reader = &b""[..];
    let mut is_read = false;

    let result = read_input_inner_with_state(&mut reader, &mut is_read);
    assert!(
        matches!(result, Err(SdkError::InvalidInput)),
        "Expected InvalidInput for empty input, got {:?}",
        result
    );
}

#[test]
fn write_success_cannot_be_followed_by_write_failure() {
    use vo_sdk::io::{write_failure_inner_with_state, write_success_inner_with_state};

    let mut buf = Vec::new();
    let mut is_written = false;

    let first = write_success_inner_with_state(&mut buf, &json!({"ok": true}), &mut is_written);
    assert!(first.is_ok());

    let second = write_failure_inner_with_state(
        &mut buf,
        TaskFailureKind::User,
        "too late",
        &mut is_written,
    );
    assert!(
        matches!(second, Err(SdkError::AlreadyWritten)),
        "write_failure after write_success should fail with AlreadyWritten"
    );
}

#[test]
fn write_failure_cannot_be_followed_by_write_success() {
    use vo_sdk::io::{write_failure_inner_with_state, write_success_inner_with_state};

    let mut buf = Vec::new();
    let mut is_written = false;

    let first = write_failure_inner_with_state(
        &mut buf,
        TaskFailureKind::System,
        "failed",
        &mut is_written,
    );
    assert!(first.is_ok());

    let second = write_success_inner_with_state(&mut buf, &json!({"ok": true}), &mut is_written);
    assert!(
        matches!(second, Err(SdkError::AlreadyWritten)),
        "write_success after write_failure should fail with AlreadyWritten"
    );
}

fn run_fd_helper(test_name: &str) -> Result<String, String> {
    let manifest_path = std::env::var("CARGO_MANIFEST_PATH")
        .ok()
        .unwrap_or_else(|| "Cargo.toml".to_string());

    let crate_dir = std::path::Path::new(&manifest_path).parent().unwrap();
    let output = std::process::Command::new("cargo")
        .args(["run", "--example", "fd_test_helper", "--", test_name])
        .current_dir(crate_dir)
        .output()
        .map_err(|e| format!("failed to spawn cargo: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "Helper exited with {}: stdout={} stderr={}",
            output.status, stdout, stderr
        ));
    }

    Ok(stdout)
}

#[test]
fn read_input_with_fd3_mock_succeeds() {
    let result = run_fd_helper("read_input_with_fd3");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert!(result.unwrap().contains("PASS"));
}

#[test]
fn write_success_with_fd4_mock_succeeds() {
    let result = run_fd_helper("write_success_with_fd4");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert!(result.unwrap().contains("PASS"));
}

#[test]
fn write_failure_with_fd4_mock_succeeds() {
    let result = run_fd_helper("write_failure_with_fd4");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert!(result.unwrap().contains("PASS"));
}

#[test]
fn double_read_blocked_via_fd_mock() {
    let result = run_fd_helper("double_read_blocked_via_fd");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert!(result.unwrap().contains("PASS"));
}

#[test]
fn double_write_blocked_via_fd_mock() {
    let result = run_fd_helper("double_write_blocked_via_fd");
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    assert!(result.unwrap().contains("PASS"));
}
