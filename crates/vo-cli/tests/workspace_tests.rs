use vo_cli::commands::doctor_checks::{check_workspace, Severity};
use crate::helpers::{make_temp_dir, setup_project};

// ============================================================
// GAP: check_workspace with stale PID files in workspace check
// ============================================================

#[test]
fn workspace_detects_stale_pid_files() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::create_dir_all(dir.join(".vo/runtime")).unwrap();
    std::fs::write(dir.join(".vo/runtime/old.pid"), "999999999\n").unwrap();
    let report = check_workspace(&dir, &dir.join(".vo"));
    assert!(
        report.checks.iter().any(|c| c.check == "stale-pid-files"),
        "workspace check should detect stale PID files"
    );
}

#[test]
fn workspace_detects_alive_pid_files() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::create_dir_all(dir.join(".vo/runtime")).unwrap();
    let my_pid = std::process::id();
    std::fs::write(dir.join(".vo/runtime/self.pid"), format!("{my_pid}\n")).unwrap();
    let report = check_workspace(&dir, &dir.join(".vo"));
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "stale-pid-files" && c.severity == Severity::Info),
        "workspace check should report all alive PIDs"
    );
}

#[test]
fn workspace_readonly_vo_dir() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let vo_dir = dir.join(".vo");
    let mut perms = std::fs::metadata(&vo_dir).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&vo_dir, perms.clone()).unwrap();
    let report = check_workspace(&dir, &vo_dir);
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Error),
        "readonly .vo dir should be an error"
    );
    perms.set_readonly(false);
    std::fs::set_permissions(&vo_dir, perms).ok();
}

#[test]
fn workspace_missing_storage_dir_warns() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    let report = check_workspace(&dir, &dir.join(".vo"));
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "storage-dir" && c.severity == Severity::Warn),
        "missing storage dir should warn"
    );
}
