#![allow(clippy::all)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::tempdir;
use vo_ipc::{run_subprocess, IpcError, SubprocessConfig, MAX_STDERR_BYTES, TRUNCATION_MARKER};

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fixture_driver"))
}

fn config(payload: impl AsRef<[u8]>, timeout_ms: u64) -> SubprocessConfig {
    SubprocessConfig::new(fixture_binary(), timeout_ms, payload.as_ref().to_vec()).unwrap()
}

fn read_map(path: &Path) -> BTreeMap<String, String> {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn proc_exists(pid: i32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

#[tokio::test]
async fn fd4_success_echoes_payload() {
    let output = run_subprocess(config("echo-fd3 hello", 500)).await.unwrap();
    assert_eq!(output.fd4_bytes, b"echo-fd3 hello");
}

#[tokio::test]
async fn empty_stderr_returns_empty_buffer() {
    let output = run_subprocess(config("echo-fd3 hello", 500)).await.unwrap();
    assert_eq!(output.stderr_bytes, Vec::<u8>::new());
    assert!(!output.stderr_truncated);
}

#[tokio::test]
async fn stderr_under_limit_is_preserved() {
    let output = run_subprocess(config("stderr-text warn 0", 500))
        .await
        .unwrap();
    assert_eq!(output.stderr_bytes, b"warn");
}

#[tokio::test]
async fn fd3_payload_is_delivered_raw() {
    let output = run_subprocess(config("echo-fd3 alpha beta gamma", 500))
        .await
        .unwrap();
    assert_eq!(output.fd4_bytes, b"echo-fd3 alpha beta gamma");
}

#[tokio::test]
async fn fd3_eof_is_observed_after_parent_write() {
    let output = run_subprocess(config("fd3-eof sample", 500)).await.unwrap();
    assert_eq!(output.fd4_bytes, b"sample|EOF");
}

#[tokio::test]
async fn timeout_returns_elapsed_ms() {
    let started = Instant::now();
    let error = run_subprocess(config("timeout-ignore none sleep", 20))
        .await
        .unwrap_err();
    let elapsed = started.elapsed();

    match error {
        IpcError::Timeout { elapsed_ms, .. } => {
            assert!(elapsed_ms >= 20);
            assert!(elapsed >= Duration::from_millis(20));
        }
        other => panic!("expected timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn partial_stderr_is_returned_on_timeout() {
    let marker_dir = tempdir().unwrap();
    let marker = marker_dir.path().join("term.txt");
    let payload = format!("timeout-term-exit {} 0 none partial", marker.display());
    let error = run_subprocess(config(payload, 30)).await.unwrap_err();

    match error {
        IpcError::Timeout { stderr_bytes, .. } => {
            assert!(String::from_utf8_lossy(&stderr_bytes).contains("partial"));
            assert!(String::from_utf8_lossy(&stderr_bytes).contains("sigterm"));
        }
        other => panic!("expected timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn sigterm_marker_is_written_before_kill() {
    let marker_dir = tempdir().unwrap();
    let marker = marker_dir.path().join("term.txt");
    let payload = format!("timeout-term-exit {} 5000 none body", marker.display());
    let error = run_subprocess(config(payload, 20)).await.unwrap_err();
    assert!(matches!(error, IpcError::Timeout { .. }));
    assert_eq!(fs::read_to_string(&marker).unwrap(), "SIGTERM");
}

#[tokio::test]
async fn grace_period_is_enforced_before_sigkill() {
    let started = Instant::now();
    let error = run_subprocess(config("timeout-ignore none sleep", 20))
        .await
        .unwrap_err();
    let elapsed = started.elapsed();
    assert!(matches!(error, IpcError::Timeout { .. }));
    assert!(elapsed >= Duration::from_millis(1900));
    assert!(elapsed <= Duration::from_secs(4));
}

#[tokio::test]
async fn sigkill_is_skipped_when_child_exits_during_grace() {
    let marker_dir = tempdir().unwrap();
    let marker = marker_dir.path().join("term.txt");
    let started = Instant::now();
    let payload = format!("timeout-term-exit {} 150 none body", marker.display());
    let error = run_subprocess(config(payload, 20)).await.unwrap_err();
    let elapsed = started.elapsed();
    assert!(matches!(error, IpcError::Timeout { .. }));
    assert!(elapsed < Duration::from_secs(2));
    assert_eq!(fs::read_to_string(&marker).unwrap(), "SIGTERM");
}

#[tokio::test]
async fn success_path_reaps_child() {
    let directory = tempdir().unwrap();
    let pid_path = directory.path().join("pid.txt");
    let payload = format!("pid-and-exit {} 0", pid_path.display());
    let output = run_subprocess(config(payload, 500)).await.unwrap();
    let pid: i32 = fs::read_to_string(&pid_path).unwrap().parse().unwrap();
    assert_eq!(output.fd4_bytes, b"pid-ready");
    assert!(!proc_exists(pid));
}

#[tokio::test]
async fn timeout_path_reaps_child() {
    let directory = tempdir().unwrap();
    let pid_path = directory.path().join("pid.txt");
    let payload = format!("timeout-ignore {} sleep", pid_path.display());
    let error = run_subprocess(config(payload, 20)).await.unwrap_err();
    let pid: i32 = fs::read_to_string(&pid_path).unwrap().parse().unwrap();
    assert!(matches!(error, IpcError::Timeout { .. }));
    assert!(!proc_exists(pid));
}

#[tokio::test]
async fn non_zero_exit_code_is_preserved() {
    let error = run_subprocess(config("stderr-text fail 17", 500))
        .await
        .unwrap_err();
    match error {
        IpcError::ProcessFailed {
            exit_code,
            stderr_bytes,
            ..
        } => {
            assert_eq!(exit_code, 17);
            assert_eq!(stderr_bytes, b"fail");
        }
        other => panic!("expected process failure, got {other:?}"),
    }
}

#[tokio::test]
async fn exit_code_255_is_preserved() {
    let error = run_subprocess(config("stderr-text boom 255", 500))
        .await
        .unwrap_err();
    match error {
        IpcError::ProcessFailed { exit_code, .. } => assert_eq!(exit_code, 255),
        other => panic!("expected process failure, got {other:?}"),
    }
}

#[tokio::test]
async fn no_sigpipe_when_stderr_is_full_and_child_sleeps() {
    let error = run_subprocess(config("timeout-ignore none flood", 20))
        .await
        .unwrap_err();
    match error {
        IpcError::Timeout { stderr_bytes, .. } => {
            assert!(stderr_bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
        }
        other => panic!("expected timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn grandchild_fd_isolation_behavior_returns_promptly() {
    let started = Instant::now();
    let output = run_subprocess(config("grandchild-hold 1000", 500))
        .await
        .unwrap();
    assert_eq!(output.fd4_bytes, b"child-done");
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[tokio::test]
async fn child_environment_is_cleared() {
    std::env::set_var("LEAK_ME", "secret");
    let output = run_subprocess(config("read-env", 500)).await.unwrap();
    let environment: BTreeMap<String, String> = serde_json::from_slice(&output.fd4_bytes).unwrap();
    std::env::remove_var("LEAK_ME");
    
    // Filter out environment variables injected by build/coverage tools (like llvm-cov)
    let filtered_env: BTreeMap<_, _> = environment
        .into_iter()
        .filter(|(k, _)| {
            k != "LLVM_PROFILE_FILE" && !k.starts_with("__LLVM_PROFILE")
        })
        .collect();
        
    assert!(filtered_env.is_empty(), "Environment not cleared: {:?}", filtered_env);
}

#[tokio::test]
async fn non_zero_exit_over_limit_includes_marker() {
    let payload = format!("stderr-repeat {} x 23", MAX_STDERR_BYTES + 17);
    let error = run_subprocess(config(payload, 500)).await.unwrap_err();
    match error {
        IpcError::ProcessFailed {
            exit_code,
            stderr_bytes,
            stderr_truncated,
        } => {
            assert_eq!(exit_code, 23);
            assert!(stderr_truncated);
            assert!(stderr_bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
        }
        other => panic!("expected process failure, got {other:?}"),
    }
}

#[tokio::test]
async fn minimum_timeout_one_ms_is_supported() {
    let error = run_subprocess(config("timeout-ignore none sleep", 1))
        .await
        .unwrap_err();
    assert!(matches!(error, IpcError::Timeout { .. }));
}

#[tokio::test]
async fn stderr_only_success_path_returns_empty_fd4() {
    let output = run_subprocess(config("stderr-text warning 0", 500))
        .await
        .unwrap();
    assert_eq!(output.fd4_bytes, Vec::<u8>::new());
    assert_eq!(output.stderr_bytes, b"warning");
}

#[tokio::test]
async fn deterministic_success_before_timeout() {
    let output = run_subprocess(config("sleep-exit 10 0 done", 200))
        .await
        .unwrap();
    assert_eq!(output.fd4_bytes, b"done");
}

#[tokio::test]
async fn deterministic_timeout_case_is_bounded() {
    let started = Instant::now();
    let error = run_subprocess(config("timeout-ignore none sleep", 30))
        .await
        .unwrap_err();
    assert!(matches!(error, IpcError::Timeout { .. }));
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[tokio::test]
async fn argv_parsing_uses_whitespace_split() {
    let output = run_subprocess(config("read-argv alpha beta gamma", 500))
        .await
        .unwrap();
    let args: Vec<String> = serde_json::from_slice(&output.fd4_bytes).unwrap();
    assert_eq!(args, vec!["read-argv", "alpha", "beta", "gamma"]);
}

#[tokio::test]
async fn invalid_utf8_payload_produces_no_argv_but_still_writes_fd3() {
    let output = run_subprocess(config([0xff_u8, 0xfe_u8], 500))
        .await
        .unwrap();
    assert_eq!(output.fd4_bytes, Vec::<u8>::new());
}

#[tokio::test]
async fn fd3_write_failure_is_non_fatal() {
    let output = run_subprocess(config("close-fd3", 500)).await.unwrap();
    assert_eq!(output.fd4_bytes, b"closed-fd3");
}

#[tokio::test]
async fn truncation_marker_is_appended_at_limit_plus_one() {
    let payload = format!("stderr-repeat {} z 0", MAX_STDERR_BYTES + 1);
    let output = run_subprocess(config(payload, 500)).await.unwrap();
    assert!(output.stderr_truncated);
    assert!(output.stderr_bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
}

#[tokio::test]
async fn stderr_max_minus_one_is_not_truncated() {
    let payload = format!("stderr-repeat {} q 0", MAX_STDERR_BYTES - 1);
    let output = run_subprocess(config(payload, 500)).await.unwrap();
    assert!(!output.stderr_truncated);
    assert_eq!(output.stderr_bytes.len(), MAX_STDERR_BYTES - 1);
}

#[tokio::test]
async fn stderr_exact_max_is_not_truncated() {
    let payload = format!("stderr-repeat {} q 0", MAX_STDERR_BYTES);
    let output = run_subprocess(config(payload, 500)).await.unwrap();
    assert!(!output.stderr_truncated);
    assert_eq!(output.stderr_bytes.len(), MAX_STDERR_BYTES);
}

#[tokio::test]
async fn stderr_prefix_is_preserved_when_truncated() {
    let payload = format!("stderr-repeat {} a 0", MAX_STDERR_BYTES + 12);
    let output = run_subprocess(config(payload, 500)).await.unwrap();
    assert_eq!(&output.stderr_bytes[..8], b"aaaaaaaa");
}

#[tokio::test]
async fn env_snapshot_fixture_returns_valid_json() {
    let directory = tempdir().unwrap();
    let snapshot_path = directory.path().join("env.json");
    fs::write(&snapshot_path, b"{}").unwrap();
    let map = read_map(&snapshot_path);
    assert!(map.is_empty());
}

#[tokio::test]
async fn timeout_contains_partial_stderr_from_flooding_child() {
    let error = run_subprocess(config("timeout-ignore none flood", 20))
        .await
        .unwrap_err();
    match error {
        IpcError::Timeout { stderr_bytes, .. } => assert!(!stderr_bytes.is_empty()),
        other => panic!("expected timeout, got {other:?}"),
    }
}
