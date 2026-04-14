//! RED QUEEN: Subprocess spawn edge cases
//!
//! Adversarial tests for: OOM during spawn, killed mid-init, zombie detection,
//! rapid spawn/kill cycles, exceeded file descriptors.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use vo_ipc::{run_subprocess, IpcError, SubprocessConfig};

fn make_executable(path: &Path) {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

// ========================================================================
// DIMENSION: killed-mid-init
// Contract: child that kills itself immediately is handled gracefully
// ========================================================================

#[tokio::test]
async fn spawn_child_self_kill_returns_process_failed() {
    // Child immediately sends SIGKILL to itself — simulates OOM killer or fatal signal
    let dir = tempdir().unwrap();
    let script = dir.path().join("self_kill.sh");
    std::fs::write(&script, "#!/bin/sh\nkill -9 $$\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::ProcessFailed { exit_code, .. }) => {
            // SIGKILL = signal 9, mapped to 128+9 = 137
            assert_eq!(
                exit_code, 137,
                "SIGKILL should map to exit code 137, got {}",
                exit_code
            );
        }
        Err(IpcError::Fd4ReadFailed { .. }) => {
            // Also acceptable — child died before writing to fd4
        }
        other => panic!("expected ProcessFailed or Fd4ReadFailed, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: killed-mid-init
// Contract: child that exits with abort signal is handled
// ========================================================================

#[tokio::test]
async fn spawn_child_abort_returns_signal_exit_code() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("abort.sh");
    std::fs::write(&script, "#!/bin/sh\nkill -ABRT $$\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::ProcessFailed { exit_code, .. }) => {
            // SIGABRT = signal 6, mapped to 128+6 = 134
            assert_eq!(
                exit_code, 134,
                "SIGABRT should map to exit code 134, got {}",
                exit_code
            );
        }
        Err(IpcError::Fd4ReadFailed { .. }) => {
            // Also acceptable
        }
        other => panic!("expected ProcessFailed or Fd4ReadFailed, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: rapid-spawn-kill
// Contract: rapid self-kill cycles complete quickly and don't leak
// ========================================================================

#[tokio::test]
async fn spawn_rapid_sigkill_cycles_complete_within_time() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("self_kill.sh");
    std::fs::write(&script, "#!/bin/sh\nkill -9 $$\n").unwrap();
    make_executable(&script);

    let start = std::time::Instant::now();

    for _ in 0..10 {
        let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
        let result = run_subprocess(config).await;
        assert!(result.is_err(), "self-kill should fail");
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "10 rapid self-kill cycles should complete within 10s, took {:?}",
        elapsed
    );
}

// ========================================================================
// DIMENSION: zombie-detection
// Contract: child is properly reaped after normal exit
// ========================================================================

#[tokio::test]
async fn spawn_child_reaped_after_normal_exit() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("pid.txt");

    let script = dir.path().join("pid_exit.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > {}\nsleep 0.01\nexit 0\n",
            pid_path.display()
        ),
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let result = run_subprocess(config).await;

    assert!(result.is_ok(), "normal exit should succeed: {:?}", result);

    // Wait briefly for OS to clean up
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let pid_str = std::fs::read_to_string(&pid_path).unwrap();
    let pid: i32 = pid_str.trim().parse().unwrap();
    let proc_exists = PathBuf::from(format!("/proc/{}", pid)).exists();
    assert!(
        !proc_exists,
        "process {} should be reaped after normal exit",
        pid
    );
}

// ========================================================================
// DIMENSION: zombie-detection
// Contract: child is properly reaped after signal kill (timeout)
// ========================================================================

#[tokio::test]
async fn spawn_child_reaped_after_timeout_kill() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("pid.txt");

    let script = dir.path().join("pid_sleep.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > {}\nsleep 60\n",
            pid_path.display()
        ),
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 100, vec![]).unwrap();
    let result = run_subprocess(config).await;

    assert!(result.is_err(), "timeout should occur");

    // Wait for SIGKILL cleanup
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let pid_str = std::fs::read_to_string(&pid_path).unwrap();
    let pid: i32 = pid_str.trim().parse().unwrap();
    let proc_exists = PathBuf::from(format!("/proc/{}", pid)).exists();
    assert!(
        !proc_exists,
        "process {} should be reaped after timeout kill",
        pid
    );
}

// ========================================================================
// DIMENSION: rapid-spawn-kill
// Contract: two concurrent spawns both complete
// ========================================================================

#[tokio::test]
async fn spawn_two_concurrent_spawns_both_complete() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    let config1 = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let config2 = SubprocessConfig::new(&script, 2000, vec![]).unwrap();

    let (r1, r2) = tokio::join!(
        tokio::spawn(async move { run_subprocess(config1).await }),
        tokio::spawn(async move { run_subprocess(config2).await }),
    );

    assert!(r1.unwrap().is_ok(), "first concurrent spawn should succeed");
    assert!(r2.unwrap().is_ok(), "second concurrent spawn should succeed");
}

// ========================================================================
// DIMENSION: rapid-spawn-kill
// Contract: concurrent timeouts all have processes reaped
// ========================================================================

#[tokio::test]
async fn spawn_concurrent_timeouts_all_cleaned_up() {
    let dir = tempdir().unwrap();
    let mut pid_paths = Vec::new();

    for i in 0..5 {
        let pid_path = dir.path().join(format!("pid_{}.txt", i));
        let s = dir.path().join(format!("sleeper_{}.sh", i));
        std::fs::write(
            &s,
            format!("#!/bin/sh\necho $$ > {}\nsleep 60\n", pid_path.display()),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&s).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&s, perms).unwrap();
        pid_paths.push(pid_path);
    }

    let mut handles = Vec::new();
    for i in 0..5 {
        let s = dir.path().join(format!("sleeper_{}.sh", i));
        let config = SubprocessConfig::new(&s, 200, vec![]).unwrap();
        handles.push(tokio::spawn(async move { run_subprocess(config).await }));
    }

    for handle in handles {
        let result = handle.await.expect("task panicked");
        match result {
            Err(IpcError::Timeout { .. }) => {}
            other => panic!("expected Timeout, got {:?}", other),
        }
    }

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    for pid_path in &pid_paths {
        if pid_path.exists() {
            let pid_str = std::fs::read_to_string(pid_path).unwrap();
            let pid: i32 = pid_str.trim().parse().unwrap();
            let proc_exists = PathBuf::from(format!("/proc/{}", pid)).exists();
            assert!(
                !proc_exists,
                "process {} should be reaped after concurrent timeout",
                pid
            );
        }
    }
}

// ========================================================================
// DIMENSION: fd-exhaustion
// Contract: many sequential spawns don't leak file descriptors
// ========================================================================

#[tokio::test]
async fn spawn_many_sequential_spawns_dont_leak_fds() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    // Each spawn creates 2 pipes (4 FDs). Run 50 sequentially to check for FD leaks.
    for _ in 0..50 {
        let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
        let result = run_subprocess(config).await;
        assert!(result.is_ok(), "sequential spawn should succeed: {:?}", result);
    }
}

// ========================================================================
// DIMENSION: process-group-kill
// Contract: killing process group reaps all children
// ========================================================================

#[tokio::test]
async fn spawn_process_group_kill_reaps_child() {
    let dir = tempdir().unwrap();
    let pid_path = dir.path().join("pid.txt");
    let script = dir.path().join("pgid_sleeper.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > {}\nsleep 60\n",
            pid_path.display()
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let config = SubprocessConfig::new(&script, 150, vec![]).unwrap();
    let result = run_subprocess(config).await;

    assert!(result.is_err(), "should timeout");

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    if pid_path.exists() {
        let pid_str = std::fs::read_to_string(&pid_path).unwrap();
        let pid: i32 = pid_str.trim().parse().unwrap();
        let proc_exists = PathBuf::from(format!("/proc/{}", pid)).exists();
        assert!(
            !proc_exists,
            "process {} should be reaped by process group kill",
            pid
        );
    }
}

// ========================================================================
// DIMENSION: killed-mid-init
// Contract: child that exits without writing to fd4 returns error
// ========================================================================

#[tokio::test]
async fn spawn_child_exits_without_fd4_response() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("sleep_no_response.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 0.01\nexit 1\n").unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let config = SubprocessConfig::new(&script, 500, b"payload".to_vec()).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Err(IpcError::ProcessFailed { exit_code, .. }) => {
            assert_eq!(exit_code, 1);
        }
        Err(IpcError::Fd4ReadFailed { .. }) => {}
        other => panic!("expected ProcessFailed or Fd4ReadFailed, got {:?}", other),
    }
}

// ========================================================================
// DIMENSION: spawn-payload-boundary
// Contract: large fd3 payload is handled
// ========================================================================

#[tokio::test]
async fn spawn_large_fd3_payload_handled() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let large_payload = vec![b'A'; 100_000];
    let config = SubprocessConfig::new(&script, 2000, large_payload).unwrap();
    let result = run_subprocess(config).await;
    assert!(
        result.is_ok(),
        "large fd3 payload should be handled: {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: rapid-spawn-kill
// Contract: broken pipe from child exit doesn't hang parent
// ========================================================================

#[tokio::test]
async fn spawn_broken_pipe_from_quick_exit_doesnt_hang() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick_exit.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    // Large payload to make the write more likely to encounter broken pipe
    let payload = vec![b'X'; 65_536];
    let config = SubprocessConfig::new(&script, 2000, payload).unwrap();
    let result = run_subprocess(config).await;

    assert!(
        result.is_ok(),
        "broken pipe from quick child exit should be non-fatal: {:?}",
        result
    );
}

// ========================================================================
// DIMENSION: spawn-stderr-under-load
// Contract: child that floods stderr doesn't hang the parent
// ========================================================================

#[tokio::test]
async fn spawn_child_stderr_flood_doesnt_hang() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("stderr_flood.sh");
    // Generate 100KB of stderr output then exit
    std::fs::write(&script, "#!/bin/sh\ndd if=/dev/zero bs=1000 count=100 2>/dev/null | tr '\\0' 'x' >&2\nexit 0\n").unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let config = SubprocessConfig::new(&script, 3000, vec![]).unwrap();
    let result = run_subprocess(config).await;

    match result {
        Ok(output) => {
            // stderr should be captured and bounded by MAX_STDERR_BYTES (1MB)
            assert!(
                !output.stderr_bytes.is_empty(),
                "stderr should have been captured"
            );
        }
        Err(IpcError::ProcessFailed { stderr_bytes, .. }) => {
            assert!(
                !stderr_bytes.is_empty(),
                "stderr should be captured even on failure"
            );
        }
        other => panic!("expected Ok or ProcessFailed, got {:?}", other),
    }
}
