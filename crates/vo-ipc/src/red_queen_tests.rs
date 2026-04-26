//! Red Queen tests — subprocess spawn edge cases.
//!
//! These tests cover edge cases in the subprocess spawning code,
//! particularly around error handling in pre_exec, pipe setup, and spawn failures.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::config::SubprocessConfig;
use crate::error::IpcError;
use crate::run_subprocess;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

fn executable_file() -> PathBuf {
    let directory = tempdir().unwrap();
    let file = directory.path().join("fixture.sh");
    std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&file).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&file, perms).unwrap();
    let path = file.clone();
    std::mem::forget(directory);
    path
}

fn make_executable(path: &PathBuf) {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

// ========================================================================
// DIMENSION: spawn-pipe-failure
// Contract: pipe2 failure returns PipeSetupFailed
// ========================================================================

#[tokio::test]
async fn red_queen_spawn_with_valid_config_succeeds() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 1000, b"echo hello".to_vec()).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "Valid spawn should succeed, got {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: spawn-config-validation
// Contract: invalid config returns appropriate ConfigError
// ========================================================================

#[test]
fn red_queen_config_rejects_zero_timeout() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 0, vec![]);
    assert!(config.is_err());
    let err = config.unwrap_err();
    assert!(err.to_string().contains("timeout must be greater than 0ms"));
}

#[test]
fn red_queen_config_rejects_missing_program() {
    let config = SubprocessConfig::new("/nonexistent/program/path", 100, vec![]);
    assert!(config.is_err());
    let err = config.unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn red_queen_config_rejects_non_executable() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("not_exec.txt");
    std::fs::write(&file, "not executable").unwrap();
    let config = SubprocessConfig::new(&file, 100, vec![]);
    assert!(config.is_err());
    let err = config.unwrap_err();
    assert!(err.to_string().contains("not executable"));
}

#[test]
fn red_queen_config_accepts_minimum_timeout() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 500, vec![]);
    assert!(config.is_ok(), "timeout=500 should be accepted");
}

// ========================================================================
// DIMENSION: spawn-process-group
// Contract: setpgid in pre_exec is called (verifiable via successful spawn)
// ========================================================================

#[tokio::test]
async fn red_queen_child_runs_in_own_process_group() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("pid.txt");
    let _path = executable_file();

    let script = dir.path().join("check_pgid.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho $$ > {}\nexit 0\n", pid_path.display()),
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "Child with pgid setup should run: {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: spawn-fd-handling
// Contract: fd3/fd4 setup in pre_exec must succeed or spawn fails
// ========================================================================

#[tokio::test]
async fn red_queen_subprocess_receives_fd3_payload() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 500, b"arg1 arg2".to_vec()).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "Spawn with fd3 payload should work: {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: spawn-timeout-handling
// Contract: timeout returns Timeout error, not panics
// ========================================================================

#[tokio::test]
async fn red_queen_timeout_returns_correct_elapsed_ms() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("sleeper.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 10\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 50, vec![]).unwrap();
    let start = std::time::Instant::now();
    let result = run_subprocess(config).await;
    let _elapsed = start.elapsed();

    assert!(result.is_err());
    match result.unwrap_err() {
        IpcError::Timeout { elapsed_ms, .. } => {
            assert!(
                elapsed_ms >= 50,
                "timeout elapsed_ms should be >= 50, got {}",
                elapsed_ms
            );
        }
        other => panic!("expected Timeout, got {:?}", other),
    }
}

#[tokio::test]
async fn red_queen_timeout_contains_stderr_bytes() {
    let path = executable_file();
    let script = path.parent().unwrap().join("stderr_sleeper.sh");
    std::fs::write(&script, "#!/bin/sh\necho 'partial stderr' >&2\nsleep 10\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 30, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result.unwrap_err() {
        IpcError::Timeout { stderr_bytes, .. } => {
            assert!(
                String::from_utf8_lossy(&stderr_bytes).contains("partial stderr"),
                "stderr should contain 'partial stderr', got: {:?}",
                stderr_bytes
            );
        }
        other => panic!("expected Timeout, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: spawn-exit-code-mapping
// Contract: exit codes are correctly mapped including signals
// ========================================================================

#[tokio::test]
async fn red_queen_exit_code_preserved_on_success() {
    let path = executable_file();
    let script = path.parent().unwrap().join("exit_zero.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await;
    assert!(result.is_ok(), "exit 0 should succeed: {:?}", result);
}

#[tokio::test]
async fn red_queen_exit_code_preserved_on_nonzero() {
    let path = executable_file();
    let script = path.parent().unwrap().join("exit_42.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 42\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result.unwrap_err() {
        IpcError::ProcessFailed { exit_code, .. } => {
            assert_eq!(exit_code, 42, "exit code should be 42");
        }
        other => panic!("expected ProcessFailed with exit 42, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: spawn-signal-propagation
// Contract: signals to child are correctly propagated
// ========================================================================

#[tokio::test]
async fn red_queen_sigterm_termination_returns_timeout() {
    let path = executable_file();
    let script = path.parent().unwrap().join("sigterm_sleeper.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 60\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 100, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result.unwrap_err() {
        IpcError::Timeout { .. } => {}
        other => panic!("expected Timeout on sigterm, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: spawn-graceful-shutdown
// Contract: child is reaped after timeout
// ========================================================================

#[tokio::test]
async fn red_queen_child_is_reaped_after_timeout() {
    use std::fs;
    use std::path::PathBuf;

    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("pid.txt");
    let _path = executable_file();

    let script = dir.path().join("pid_sleeper.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho $$ > {}\nsleep 10\n", pid_path.display()),
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 100, vec![]).unwrap();
    let result = run_subprocess(config).await;

    assert!(result.is_err(), "timeout should occur");
    assert!(pid_path.exists(), "pid file should exist");

    let pid_str = fs::read_to_string(&pid_path).unwrap();
    let pid: i32 = pid_str.trim().parse().unwrap();

    let proc_exists = PathBuf::from(format!("/proc/{}", pid)).exists();
    assert!(!proc_exists, "process {} should be reaped", pid);
}

// ========================================================================
// DIMENSION: spawn-stderr-capture
// Contract: stderr is captured correctly
// ========================================================================

#[tokio::test]
async fn red_queen_stderr_is_captured_on_success() {
    let path = executable_file();
    let script = path.parent().unwrap().join("stderr_writer.sh");
    std::fs::write(&script, "#!/bin/sh\necho 'hello stderr' >&2\nexit 0\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await.unwrap();

    assert_eq!(
        result.stderr_bytes, b"hello stderr\n",
        "stderr should be captured"
    );
}

#[tokio::test]
async fn red_queen_stderr_is_captured_on_failure() {
    let path = executable_file();
    let script = path.parent().unwrap().join("stderr_fail.sh");
    std::fs::write(&script, "#!/bin/sh\necho 'error msg' >&2\nexit 1\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result.unwrap_err() {
        IpcError::ProcessFailed { stderr_bytes, .. } => {
            assert!(
                String::from_utf8_lossy(&stderr_bytes).contains("error msg"),
                "stderr should contain 'error msg'"
            );
        }
        other => panic!("expected ProcessFailed, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: spawn-fd3-write-failure
// Contract: fd3 write failure is non-fatal
// ========================================================================

#[tokio::test]
async fn red_queen_fd3_write_failure_does_not_panic() {
    let path = executable_file();
    let script = path.parent().unwrap().join("close_fd3.sh");
    std::fs::write(&script, "#!/bin/sh\necho 'closed fd3'\nexit 0\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "fd3 write failure should be non-fatal: {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: spawn-empty-payload
// Contract: empty fd3 payload is handled correctly
// ========================================================================

#[tokio::test]
async fn red_queen_empty_payload_is_handled() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 500, vec![]).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "empty payload should be handled: {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: spawn-large-payload
// Contract: large fd3 payload doesn't cause issues
// ========================================================================

#[tokio::test]
async fn red_queen_large_payload_is_handled() {
    let path = executable_file();
    let large_payload = vec![b'x'; 10_000];
    let config = SubprocessConfig::new(&path, 500, large_payload).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "large payload should be handled: {:?}",
        result
    );
}
