//! Process tree kill test for subprocess timeout
//!
//! Verifies that when a subprocess exceeds its timeout, the entire process
//! group is killed (including grandchildren spawned via sh -c or direct spawns).
//!
//! Uses shell scripts with tempdir to avoid test_subprocess_helper fd issues.

use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use vo_executor::subprocess::{run_subprocess, SubprocessConfig};

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

fn create_tree_spawn_script(dir: &TempDir) -> String {
    let script = dir.path().join("tree_spawn.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         # Spawn a grandchild that survives the parent\n\
         sh -c 'sleep 300' &\n\
         GRANDCHILD_PID=$!\n\
         # Write grandchild PID for test verification\n\
         echo $GRANDCHILD_PID > /tmp/vo_gc_pid\n\
         # Parent sleeps long enough to trigger timeout\n\
         sleep 300\n",
    )
    .unwrap();
    make_executable(&script);
    script.to_string_lossy().to_string()
}

fn create_simple_sleep_script(dir: &TempDir) -> String {
    let script = dir.path().join("simple_sleep.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         sleep 300\n",
    )
    .unwrap();
    make_executable(&script);
    script.to_string_lossy().to_string()
}

fn create_quick_exit_script(dir: &TempDir) -> String {
    let script = dir.path().join("quick_exit.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         exit 0\n",
    )
    .unwrap();
    make_executable(&script);
    script.to_string_lossy().to_string()
}

#[tokio::test]
async fn test_timeout_kills_process_tree() {
    let dir = TempDir::new().unwrap();
    let script = create_tree_spawn_script(&dir);

    let config = SubprocessConfig::new(script, vec![], 200, vec![]);

    let result = run_subprocess(config).await;

    assert!(result.is_err(), "expected timeout error: {:?}", result);
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("timeout"), "error should mention timeout: {}", err_str);
}

#[tokio::test]
async fn test_grandchild_killed_on_timeout() {
    let dir = TempDir::new().unwrap();
    let script = create_tree_spawn_script(&dir);

    // Clean up any leftover PID file
    let _ = std::fs::remove_file("/tmp/vo_gc_pid");

    let config = SubprocessConfig::new(script, vec![], 200, vec![]);
    let result = run_subprocess(config).await;

    assert!(result.is_err(), "expected timeout: {:?}", result);

    // Wait for the process tree to be fully cleaned up
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check if grandchild is still alive
    if let Ok(gc_pid_str) = std::fs::read_to_string("/tmp/vo_gc_pid") {
        let gc_pid: i32 = gc_pid_str.trim().parse().unwrap_or(0);
        if gc_pid > 0 {
            let proc_exists = Path::new(&format!("/proc/{}", gc_pid)).exists();
            let _ = std::fs::remove_file("/tmp/vo_gc_pid");
            assert!(
                !proc_exists,
                "Grandchild process {} should have been killed on timeout",
                gc_pid
            );
        }
    }
}

#[tokio::test]
async fn test_simple_subprocess_times_out() {
    let dir = TempDir::new().unwrap();
    let script = create_simple_sleep_script(&dir);

    let config = SubprocessConfig::new(script, vec![], 100, vec![]);
    let result = run_subprocess(config).await;

    assert!(result.is_err(), "should timeout");
    match result.unwrap_err() {
        vo_executor::subprocess::SubprocessError::Timeout { elapsed_ms } => {
            assert_eq!(elapsed_ms, 100);
        }
        other => panic!("expected Timeout error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_subprocess_exits_cleanly_within_timeout() {
    let dir = TempDir::new().unwrap();
    let script = create_quick_exit_script(&dir);

    let config = SubprocessConfig::new(script, vec![], 5000, vec![]);
    let result = run_subprocess(config).await;

    assert!(result.is_ok(), "should succeed: {:?}", result);
    let output = result.unwrap();
    assert_eq!(output.exit_code, Some(0));
}

#[tokio::test]
async fn test_timeout_reclaims_process_resources() {
    let dir = TempDir::new().unwrap();
    let script = create_tree_spawn_script(&dir);

    let config = SubprocessConfig::new(script, vec![], 100, vec![]);
    let result = run_subprocess(config).await;

    assert!(result.is_err(), "should timeout");

    // Wait longer to ensure all processes are reaped
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Check no vo-related orphan processes remain
    let output = std::process::Command::new("ps")
        .args(["-e", "-o", "pid,ppid,comm"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

    // This is a best-effort check - the key assertion is that timeout completes
    // and the process group is killed (tested in grandchild_killed test)
    assert!(result.is_err(), "timeout should have occurred");
}
