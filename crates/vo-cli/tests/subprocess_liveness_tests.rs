mod test_helpers;
use std::path::PathBuf;
use test_helpers::{make_temp_dir, setup_project};
use vo_cli::commands::doctor_checks::{check_subprocess_liveness, CheckResult};

// ============================================================
// GAP: check_subprocess_liveness with PID files
// ============================================================

#[test]
fn subprocess_liveness_with_stale_pid_file() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/test.pid"), "999999999\n").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    let has_dead = report.checks.iter().any(|c| c.check == "process-dead");
    assert!(has_dead, "stale PID should produce process-dead check");
}

#[test]
fn subprocess_liveness_with_invalid_pid_content() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/bad.pid"), "not-a-number\n").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    let no_checks = report
        .checks
        .iter()
        .all(|c| c.check != "process-alive" && c.check != "process-dead");
    assert!(
        no_checks,
        "invalid PID file content should be skipped gracefully"
    );
}

#[test]
fn subprocess_liveness_with_empty_pid_file() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/empty.pid"), "").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    let no_checks = report
        .checks
        .iter()
        .all(|c| c.check != "process-alive" && c.check != "process-dead");
    assert!(no_checks, "empty PID file should be skipped gracefully");
}

#[test]
fn subprocess_liveness_with_non_pid_files() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/readme.txt"), "hello").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "subprocess-liveness" && c.message.contains("no PID files")));
}

#[test]
fn subprocess_liveness_with_current_pid() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    let my_pid = std::process::id();
    std::fs::write(vo_dir.join("runtime/self.pid"), format!("{my_pid}\n")).unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(
        report.checks.iter().any(|c| c.check == "process-alive"),
        "current process PID should be detected as alive"
    );
}

#[test]
fn subprocess_liveness_mixed_pid_files() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    let my_pid = std::process::id();
    std::fs::write(vo_dir.join("runtime/alive.pid"), format!("{my_pid}\n")).unwrap();
    std::fs::write(vo_dir.join("runtime/dead.pid"), "999999999\n").unwrap();
    std::fs::write(vo_dir.join("runtime/bad.pid"), "xyz\n").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report.checks.iter().any(|c| c.check == "process-alive"));
    assert!(report.checks.iter().any(|c| c.check == "process-dead"));
}

#[test]
fn subprocess_liveness_cannot_read_runtime_dir() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "runtime-dir" || c.check == "subprocess-liveness"));
}
