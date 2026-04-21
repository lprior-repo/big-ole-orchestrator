//! Red Queen tests — subprocess spawn edge cases.
//!
//! These tests cover edge cases in the subprocess spawning code,
//! particularly around error handling in pre_exec, pipe setup, and spawn failures.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::config::SubprocessConfig;
use crate::error::IpcError;
use crate::run_subprocess;
use crate::stderr::{
    finalize_capture, read_bounded_stderr, update_capture, StderrCapture, MAX_STDERR_BYTES,
    TRUNCATION_MARKER,
};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;
use std::io::Cursor as StdCursor;
use tokio::io::AsyncWriteExt;

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
    let config = SubprocessConfig::new(&path, 1, vec![]);
    assert!(config.is_ok(), "timeout=1 should be accepted");
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

// ========================================================================
// DIMENSION: stderr-flood
// Contract: stderr overflow beyond 1MB is bounded, truncated, and marked
// ========================================================================

#[test]
fn rq_stderr_update_capture_accumulates_within_limit() {
    let initial = StderrCapture::empty();
    let chunk = vec![b'a'; 1000];
    let result = update_capture(initial, &chunk);
    assert_eq!(result.bytes.len(), 1000);
    assert!(!result.truncated);
    assert_eq!(result.observed_bytes, 1000);
}

#[test]
fn rq_stderr_update_capture_multi_chunk_within_limit() {
    let initial = StderrCapture::empty();
    let r1 = update_capture(initial, &vec![b'a'; 500_000]);
    let r2 = update_capture(r1, &vec![b'b'; 400_000]);
    let r3 = update_capture(r2, &vec![b'c'; 148_576]);
    assert_eq!(r3.bytes.len(), 500_000 + 400_000 + 148_576);
    assert!(!r3.truncated);
    assert_eq!(r3.observed_bytes, 500_000 + 400_000 + 148_576);
}

#[test]
fn rq_stderr_update_capture_truncates_at_exact_boundary() {
    let initial = StderrCapture {
        bytes: vec![b'a'; MAX_STDERR_BYTES - 1],
        truncated: false,
        observed_bytes: MAX_STDERR_BYTES - 1,
    };
    let chunk = vec![b'b'; 2];
    let result = update_capture(initial, &chunk);
    assert_eq!(result.bytes.len(), MAX_STDERR_BYTES);
    assert!(result.truncated);
    assert_eq!(result.observed_bytes, MAX_STDERR_BYTES + 1);
    assert_eq!(result.bytes[MAX_STDERR_BYTES - 1], b'b');
}

#[test]
fn rq_stderr_update_capture_rejects_all_after_full() {
    let initial = StderrCapture {
        bytes: vec![b'a'; MAX_STDERR_BYTES],
        truncated: false,
        observed_bytes: MAX_STDERR_BYTES,
    };
    let chunk = vec![b'b'; 999_999];
    let result = update_capture(initial, &chunk);
    assert_eq!(result.bytes.len(), MAX_STDERR_BYTES);
    assert!(result.truncated);
    assert_eq!(result.observed_bytes, MAX_STDERR_BYTES + 999_999);
}

#[test]
fn rq_stderr_update_capture_single_chunk_exceeds_limit() {
    let initial = StderrCapture::empty();
    let chunk = vec![b'x'; MAX_STDERR_BYTES + 500_000];
    let result = update_capture(initial, &chunk);
    assert_eq!(result.bytes.len(), MAX_STDERR_BYTES);
    assert!(result.truncated);
    assert_eq!(result.observed_bytes, MAX_STDERR_BYTES + 500_000);
}

#[test]
fn rq_stderr_update_capture_empty_chunk_no_change() {
    let initial = StderrCapture {
        bytes: vec![b'a'; 100],
        truncated: false,
        observed_bytes: 100,
    };
    let result = update_capture(initial, &[]);
    assert_eq!(result.bytes.len(), 100);
    assert!(!result.truncated);
    assert_eq!(result.observed_bytes, 100);
}

#[test]
fn rq_stderr_update_capture_preserves_truncated_flag() {
    let initial = StderrCapture {
        bytes: vec![b'a'; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 10,
    };
    let chunk = vec![b'b'; 1];
    let result = update_capture(initial, &chunk);
    assert!(result.truncated);
    assert_eq!(result.observed_bytes, MAX_STDERR_BYTES + 11);
}

#[test]
fn rq_stderr_finalize_appends_marker_when_truncated() {
    let capture = StderrCapture {
        bytes: vec![b'x'; MAX_STDERR_BYTES - 10],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES,
    };
    let result = finalize_capture(capture);
    assert!(result.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
    assert_eq!(
        result.bytes.len(),
        MAX_STDERR_BYTES - 10 + TRUNCATION_MARKER.len()
    );
}

#[test]
fn rq_stderr_finalize_idempotent_double_finalize() {
    let capture = StderrCapture {
        bytes: vec![b'x'; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 1,
    };
    let first = finalize_capture(capture);
    let second = finalize_capture(first.clone());
    assert_eq!(first.bytes, second.bytes);
    let marker = TRUNCATION_MARKER.as_bytes();
    let count = second
        .bytes
        .windows(marker.len())
        .filter(|w| *w == marker)
        .count();
    assert_eq!(count, 1);
}

#[test]
fn rq_stderr_finalize_no_marker_when_not_truncated() {
    let capture = StderrCapture {
        bytes: vec![b'h'; 100],
        truncated: false,
        observed_bytes: 100,
    };
    let result = finalize_capture(capture);
    assert_eq!(result.bytes, vec![b'h'; 100]);
    assert!(!result.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
}

#[test]
fn rq_stderr_finalize_empty_not_truncated() {
    let capture = StderrCapture::empty();
    let result = finalize_capture(capture);
    assert!(result.bytes.is_empty());
    assert!(!result.truncated);
}

// ========================================================================
// DIMENSION: stderr-async-stream
// Contract: read_bounded_stderr handles async readers correctly
// ========================================================================

#[tokio::test]
async fn rq_stderr_read_bounded_empty_reader() {
    let reader = tokio::io::BufReader::new(StdCursor::new(Vec::<u8>::new()));
    let capture = read_bounded_stderr(reader).await.unwrap();
    assert!(capture.bytes.is_empty());
    assert!(!capture.truncated);
    assert_eq!(capture.observed_bytes, 0);
}

#[tokio::test]
async fn rq_stderr_read_bounded_small_payload() {
    let data = b"hello world";
    let reader = tokio::io::BufReader::new(StdCursor::new(data.to_vec()));
    let capture = read_bounded_stderr(reader).await.unwrap();
    assert_eq!(capture.bytes, data.as_slice());
    assert!(!capture.truncated);
    assert_eq!(capture.observed_bytes, 11);
}

#[tokio::test]
async fn rq_stderr_read_bounded_exactly_at_limit() {
    let data = vec![b'z'; MAX_STDERR_BYTES];
    let reader = tokio::io::BufReader::new(StdCursor::new(data));
    let capture = read_bounded_stderr(reader).await.unwrap();
    assert_eq!(capture.bytes.len(), MAX_STDERR_BYTES);
    assert!(!capture.truncated);
    assert_eq!(capture.observed_bytes, MAX_STDERR_BYTES);
}

#[tokio::test]
async fn rq_stderr_read_beyond_limit_truncates() {
    let data = vec![b'f'; MAX_STDERR_BYTES + 100_000];
    let reader = tokio::io::BufReader::new(StdCursor::new(data));
    let capture = read_bounded_stderr(reader).await.unwrap();
    assert!(capture.truncated);
    assert!(capture.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
    assert_eq!(capture.bytes.len(), MAX_STDERR_BYTES + TRUNCATION_MARKER.len());
    assert_eq!(capture.observed_bytes, MAX_STDERR_BYTES + 100_000);
}

#[tokio::test]
async fn rq_stderr_read_multi_read_chunked() {
    let (mut writer, reader) = tokio::io::duplex(65536);
    let writer_handle = tokio::spawn(async move {
        for _ in 0..500 {
            writer.write_all(b"chunky-data-123456789\n").await.unwrap();
        }
        drop(writer);
    });

    let capture = read_bounded_stderr(reader).await.unwrap();
    writer_handle.await.unwrap();

    let total_written = 500 * 22;
    assert_eq!(capture.observed_bytes, total_written);
    if total_written > MAX_STDERR_BYTES {
        assert!(capture.truncated);
        assert_eq!(capture.bytes.len(), MAX_STDERR_BYTES);
    } else {
        assert!(!capture.truncated);
        assert_eq!(capture.bytes.len(), total_written);
    }
}

#[tokio::test]
async fn rq_stderr_read_massive_flood_5mb() {
    let data = vec![b'X'; 5 * 1024 * 1024];
    let reader = tokio::io::BufReader::new(StdCursor::new(data));
    let capture = read_bounded_stderr(reader).await.unwrap();
    assert!(capture.truncated);
    assert!(capture.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
    assert_eq!(capture.bytes.len(), MAX_STDERR_BYTES + TRUNCATION_MARKER.len());
    assert_eq!(capture.observed_bytes, 5 * 1024 * 1024);
}

#[tokio::test]
async fn rq_stderr_read_single_byte_stream() {
    let (mut writer, reader) = tokio::io::duplex(1);
    let writer_handle = tokio::spawn(async move {
        for i in 0..10000u16 {
            writer.write_all(&[i as u8]).await.unwrap();
        }
        drop(writer);
    });

    let capture = read_bounded_stderr(reader).await.unwrap();
    writer_handle.await.unwrap();
    assert_eq!(capture.observed_bytes, 10000);
    assert_eq!(capture.bytes.len(), 10000);
    assert!(!capture.truncated);
}

// ========================================================================
// DIMENSION: stderr-end-to-end-flood
// Contract: subprocess stderr flood is bounded at 1MB with truncation marker
// ========================================================================

#[tokio::test]
async fn rq_stderr_e2e_flood_beyond_1mb_is_bounded() {
    let path = executable_file();
    let script = path.parent().unwrap().join("stderr_flood.sh");
    let flood_cmd = format!("dd if=/dev/urandom bs=1024 count=2048 2>/dev/null | base64 >&2\nexit 0\n");
    std::fs::write(&script, format!("#!/bin/sh\n{}", flood_cmd)).unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Ok(output) => {
            assert!(
                output.stderr_bytes.len() <= MAX_STDERR_BYTES + TRUNCATION_MARKER.len(),
                "stderr should be bounded: got {} bytes",
                output.stderr_bytes.len()
            );
            assert!(
                output.stderr_truncated,
                "stderr should be marked truncated after 1MB flood"
            );
            let marker_pos = output
                .stderr_bytes
                .windows(TRUNCATION_MARKER.len())
                .position(|w| w == TRUNCATION_MARKER.as_bytes());
            assert!(
                marker_pos.is_some(),
                "stderr should contain truncation marker"
            );
        }
        Err(e) => panic!("expected success with bounded stderr, got: {:?}", e),
    }
}

#[tokio::test]
async fn rq_stderr_e2e_flood_on_failure_path() {
    let path = executable_file();
    let script = path.parent().unwrap().join("stderr_fail_flood.sh");
    let script_body = "#!/bin/sh\ndd if=/dev/urandom bs=1024 count=2048 2>/dev/null | base64 >&2\nexit 1\n";
    std::fs::write(&script, script_body).unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::ProcessFailed {
            stderr_bytes,
            stderr_truncated,
            ..
        }) => {
            assert!(
                stderr_bytes.len() <= MAX_STDERR_BYTES + TRUNCATION_MARKER.len(),
                "stderr should be bounded: got {} bytes",
                stderr_bytes.len()
            );
            assert!(stderr_truncated);
        }
        other => panic!("expected ProcessFailed with truncated stderr, got: {:?}", other),
    }
}

#[tokio::test]
async fn rq_stderr_e2e_flood_on_timeout_path() {
    let path = executable_file();
    let script = path.parent().unwrap().join("stderr_timeout_flood.sh");
    let script_body = "#!/bin/sh\nwhile true; do dd if=/dev/urandom bs=4096 count=256 2>/dev/null | base64 >&2; done\n";
    std::fs::write(&script, script_body).unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 200, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::Timeout {
            stderr_bytes,
            stderr_truncated,
            ..
        }) => {
            assert!(
                stderr_bytes.len() <= MAX_STDERR_BYTES + TRUNCATION_MARKER.len(),
                "stderr should be bounded: got {} bytes",
                stderr_bytes.len()
            );
            if stderr_bytes.len() >= MAX_STDERR_BYTES {
                assert!(stderr_truncated);
            }
        }
        other => panic!("expected Timeout with bounded stderr, got: {:?}", other),
    }
}

#[tokio::test]
async fn rq_stderr_e2e_small_stderr_no_truncation() {
    let path = executable_file();
    let script = path.parent().unwrap().join("small_stderr.sh");
    std::fs::write(&script, "#!/bin/sh\necho 'small' >&2\nexit 0\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 500, vec![]).unwrap();
    let result = run_subprocess(config).await.unwrap();
    assert_eq!(result.stderr_bytes, b"small\n");
    assert!(!result.stderr_truncated);
}

#[tokio::test]
async fn rq_stderr_e2e_empty_stderr_no_truncation() {
    let path = executable_file();
    let config = SubprocessConfig::new(&path, 500, vec![]).unwrap();
    let result = run_subprocess(config).await.unwrap();
    assert!(result.stderr_bytes.is_empty());
    assert!(!result.stderr_truncated);
}

#[tokio::test]
async fn rq_stderr_e2e_exact_1mb_boundary() {
    let path = executable_file();
    let script = path.parent().unwrap().join("exact_1mb.sh");
    let byte_count = 1024 * 1024;
    let script_body = format!(
        "#!/bin/sh\nhead -c {} /dev/zero >&2\nexit 0\n",
        byte_count
    );
    std::fs::write(&script, script_body).unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let result = run_subprocess(config).await.unwrap();
    assert!(
        !result.stderr_truncated,
        "exactly 1MB should not be truncated"
    );
    assert_eq!(result.stderr_bytes.len(), byte_count);
}
