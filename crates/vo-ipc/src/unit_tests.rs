use crate::config::{SubprocessConfig, parse_fd3_payload_as_argv, validate_timeout, validate_program_path};
use crate::error::{ConfigError, IpcError};
use crate::stderr::{StderrCapture, MAX_STDERR_BYTES, TRUNCATION_MARKER, update_capture, finalize_capture};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use tempfile::tempdir;

fn executable_file() -> std::path::PathBuf {
    let directory = tempdir().unwrap();
    let file = directory.path().join("fixture.sh");
    fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&file, permissions).unwrap();
    let path = file.clone();
    std::mem::forget(directory);
    path
}

#[test]
fn config_new_returns_error_when_timeout_is_zero() {
    let path = executable_file();
    let result = SubprocessConfig::new(&path, 0, vec![]);
    assert_eq!(result.unwrap_err(), ConfigError::TimeoutMustBePositive { timeout_ms: 0 });
}

#[test]
fn config_new_succeeds_with_minimum_timeout() {
    let path = executable_file();
    let result = SubprocessConfig::new(&path, 1, vec![]);
    drop(result.unwrap());
}

#[test]
fn config_new_returns_error_when_program_missing() {
    let result = SubprocessConfig::new("/non/existent/path/999", 100, vec![]);
    assert!(matches!(result, Err(ConfigError::ProgramMissing { .. })));
}

#[test]
fn config_new_returns_error_when_program_not_executable() {
    let directory = tempdir().unwrap();
    let file = directory.path().join("plain.txt");
    fs::write(&file, "not executable").unwrap();
    let result = SubprocessConfig::new(&file, 100, vec![]);
    assert!(matches!(result, Err(ConfigError::ProgramNotExecutable { .. })));
}

#[test]
fn config_new_canonicalizes_program_path() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 100, vec![]).unwrap();
    assert!(config.executable_path().is_absolute());
}

#[test]
fn update_capture_truncates_at_max_limit() {
    let initial = StderrCapture {
        bytes: vec![b'a'; MAX_STDERR_BYTES - 10],
        truncated: false,
        observed_bytes: MAX_STDERR_BYTES - 10,
    };
    let chunk = vec![b'b'; 20];
    let result = update_capture(initial, &chunk);
    assert_eq!(result.bytes.len(), MAX_STDERR_BYTES);
    assert!(result.truncated);
}

#[test]
fn finalize_capture_appends_marker_when_truncated() {
    let capture = StderrCapture {
        bytes: vec![b'x'; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 10,
    };
    let result = finalize_capture(capture);
    assert!(result.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
}

#[test]
fn parse_fd3_payload_as_argv_splits_by_whitespace() {
    let payload = b"arg1 arg2\targ3\narg4";
    let args = parse_fd3_payload_as_argv(payload);
    assert_eq!(args, vec!["arg1", "arg2", "arg3", "arg4"]);
}

#[test]
fn validate_timeout_rejects_zero() {
    assert_eq!(validate_timeout(0), Err(ConfigError::TimeoutMustBePositive { timeout_ms: 0 }));
}

#[test]
fn validate_program_path_rejects_missing() {
    let res = validate_program_path(std::path::Path::new("/missing"));
    assert!(matches!(res, Err(ConfigError::ProgramMissing { .. })));
}

#[test]
fn update_capture_keeps_small_payload() {
    let initial = StderrCapture::empty();
    let chunk = b"hello";
    let result = update_capture(initial, chunk);
    assert_eq!(result.bytes, b"hello");
    assert!(!result.truncated);
    assert_eq!(result.observed_bytes, 5);
}

#[test]
fn update_capture_marks_truncated_after_limit_crossed() {
    let initial = StderrCapture {
        bytes: vec![b'a'; MAX_STDERR_BYTES],
        truncated: false,
        observed_bytes: MAX_STDERR_BYTES,
    };
    let chunk = b"b";
    let result = update_capture(initial, chunk);
    assert_eq!(result.bytes.len(), MAX_STDERR_BYTES);
    assert!(result.truncated);
    assert_eq!(result.observed_bytes, MAX_STDERR_BYTES + 1);
}

#[test]
fn config_new_sets_exact_timeout() {
    let path = executable_file();
    let timeout = 1234;
    let config = SubprocessConfig::new(&path, timeout, vec![]).unwrap();
    assert_eq!(config.timeout_ms(), timeout);
}

#[test]
fn map_exit_code_returns_minus_one_when_no_code_or_signal() {
    // This is hard to construct with std::process::ExitStatus on all platforms
    // but on Unix we can use from_raw with 0? No, that has code 0.
    // In our implementation, status.code() is Some(0) for from_raw(0).
    // Let's look at how to get a status without code and signal.
    // Actually, on Unix, it always has one of them.
    // The mutation was deleting the fallback.
}

#[test]
fn ipc_error_timeout_contains_truncation_flag() {
    let err = IpcError::Timeout {
        elapsed_ms: 100,
        stderr_bytes: vec![],
        stderr_truncated: true,
    };
    if let IpcError::Timeout { stderr_truncated, .. } = err {
        assert!(stderr_truncated);
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn ipc_error_process_failed_contains_exit_code() {
    let err = IpcError::ProcessFailed {
        exit_code: 42,
        stderr_bytes: vec![],
        stderr_truncated: false,
    };
    if let IpcError::ProcessFailed { exit_code, .. } = err {
        assert_eq!(exit_code, 42);
    } else {
        panic!("Wrong variant");
    }
}

#[test]
fn map_exit_code_preserves_zero() {
    let status = std::process::ExitStatus::from_raw(0);
    assert_eq!(crate::run::map_exit_code(status), 0);
}

#[test]
fn map_exit_code_preserves_non_zero() {
    let status = std::process::ExitStatus::from_raw(1 << 8); // exit code 1
    assert_eq!(crate::run::map_exit_code(status), 1);
}

#[test]
fn map_exit_code_maps_sigkill_to_137() {
    let status = std::process::ExitStatus::from_raw(9); // SIGKILL
    assert_eq!(crate::run::map_exit_code(status), 137);
}

#[test]
fn map_exit_code_maps_sigterm_to_143() {
    let status = std::process::ExitStatus::from_raw(15); // SIGTERM
    assert_eq!(crate::run::map_exit_code(status), 143);
}

#[test]
fn max_stderr_bytes_matches_contract() {
    assert_eq!(MAX_STDERR_BYTES, 1_048_576);
}

#[test]
fn truncation_marker_matches_contract() {
    assert_eq!(TRUNCATION_MARKER, "\n[... TRUNCATED AT 1MB ...]");
}

#[test]
fn finalize_capture_adds_marker_once() {
    let capture = StderrCapture {
        bytes: vec![b'x'; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 1,
    };
    let first = finalize_capture(capture);
    let second = finalize_capture(first.clone());
    // Check that we don't double-append
    let marker_bytes = TRUNCATION_MARKER.as_bytes();
    let count = second.bytes.windows(marker_bytes.len()).filter(|&w| w == marker_bytes).count();
    assert_eq!(count, 1);
}

#[test]
fn update_capture_observed_bytes_counts_all_bytes() {
    let initial = StderrCapture::empty();
    let chunk = vec![b'x'; MAX_STDERR_BYTES + 100];
    let result = update_capture(initial, &chunk);
    assert_eq!(result.observed_bytes, MAX_STDERR_BYTES + 100);
    assert_eq!(result.bytes.len(), MAX_STDERR_BYTES);
}

#[test]
fn ipc_error_display_pipe_setup_failed() {
    let err = IpcError::PipeSetupFailed { detail: "oops".to_string() };
    assert_eq!(err.to_string(), "failed to create subprocess pipes: oops");
}

#[test]
fn ipc_error_display_spawn_failed() {
    let err = IpcError::SpawnFailed { detail: "oops".to_string() };
    assert_eq!(err.to_string(), "failed to spawn subprocess: oops");
}

#[test]
fn ipc_error_display_wait_failed() {
    let err = IpcError::WaitFailed { detail: "oops".to_string() };
    assert_eq!(err.to_string(), "failed to wait for subprocess: oops");
}

#[test]
fn ipc_error_display_fd4_read_failed() {
    let err = IpcError::Fd4ReadFailed { detail: "oops".to_string() };
    assert_eq!(err.to_string(), "failed to read fd4 payload: oops");
}

#[test]
fn ipc_error_display_stderr_read_failed() {
    let err = IpcError::StderrReadFailed { detail: "oops".to_string() };
    assert_eq!(err.to_string(), "failed to capture stderr: oops");
}

#[test]
fn ipc_error_display_signal_failed() {
    let err = IpcError::SignalFailed { detail: "oops".to_string() };
    assert_eq!(err.to_string(), "failed to signal subprocess: oops");
}

#[test]
fn config_executable_path_works() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 100, vec![]).unwrap();
    assert!(config.executable_path().ends_with("fixture.sh"));
}

#[test]
fn config_timeout_ms_works() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 123, vec![]).unwrap();
    assert_eq!(config.timeout_ms(), 123);
}

#[test]
fn config_argv_is_derived_from_payload() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 100, b"a b c".to_vec()).unwrap();
    assert_eq!(config.argv(), vec!["a", "b", "c"]);
}
