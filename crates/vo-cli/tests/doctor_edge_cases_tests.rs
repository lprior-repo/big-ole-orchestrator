use std::fs;
use std::path::PathBuf;
use vo_cli::commands::doctor::DoctorConfig;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::doctor_checks::*;
use vo_cli::run_doctor;

fn setup_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let vo_dir = dir.path().join(".vo");
    fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    dir
}

#[test]
fn doctor_error_display_not_initialized() {
    let err = DoctorError::NotInitialized {
        path: PathBuf::from("/my/project"),
    };
    assert!(err.to_string().contains("not initialized"));
    assert!(err.to_string().contains("/my/project"));
}

#[test]
fn doctor_error_display_io() {
    let err = DoctorError::Io {
        path: PathBuf::from("/some/path"),
        reason: "disk full".into(),
        source: std::io::Error::other("oops"),
    };
    let msg = err.to_string();
    assert!(msg.contains("I/O error"));
    assert!(msg.contains("/some/path"));
    assert!(msg.contains("disk full"));
}

#[test]
fn doctor_config_equality() {
    let c1 = DoctorConfig {
        project_dir: PathBuf::from("/a"),
    };
    let c2 = DoctorConfig {
        project_dir: PathBuf::from("/a"),
    };
    assert_eq!(c1, c2);
}

#[test]
fn doctor_config_inequality() {
    let c1 = DoctorConfig {
        project_dir: PathBuf::from("/a"),
    };
    let c2 = DoctorConfig {
        project_dir: PathBuf::from("/b"),
    };
    assert_ne!(c1, c2);
}

#[test]
fn doctor_config_clone() {
    let c = DoctorConfig {
        project_dir: PathBuf::from("/tmp"),
    };
    let cloned = c.clone();
    assert_eq!(c, cloned);
}

#[test]
fn doctor_report_with_all_five_categories() {
    let dir = setup_project();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let report = run_doctor(&config).unwrap();
    assert_eq!(report.categories.len(), 5);
}

#[test]
fn doctor_report_categories_match_enum() {
    let dir = setup_project();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let report = run_doctor(&config).unwrap();
    let expected = [
        CheckCategory::Workspace,
        CheckCategory::LockState,
        CheckCategory::SubprocessLiveness,
        CheckCategory::StorageIntegrity,
        CheckCategory::ConfigValidation,
    ];
    for (i, expected_cat) in expected.iter().enumerate() {
        assert_eq!(report.categories[i].category, *expected_cat);
    }
}

#[test]
fn doctor_workspace_with_multiple_workflows() {
    let dir = setup_project();
    fs::write(dir.path().join(".vo/workflows/wf1"), b"\x7FELFtestdata").unwrap();
    fs::write(dir.path().join(".vo/workflows/wf2"), b"\x7FELFtestdata2").unwrap();
    let report = check_workspace(dir.path(), &dir.path().join(".vo"));
    let bin_check = report
        .checks
        .iter()
        .find(|c| c.check == "workflows-dir")
        .unwrap();
    assert!(bin_check.message.contains("2 binaries"));
}

#[test]
fn doctor_workspace_with_runtime_pid_files() {
    let dir = setup_project();
    let runtime_dir = dir.path().join(".vo/runtime");
    fs::create_dir_all(&runtime_dir).unwrap();
    let current_pid = std::process::id();
    fs::write(runtime_dir.join("engine.pid"), current_pid.to_string()).unwrap();
    let report = check_workspace(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "stale-pid-files" && c.severity == Severity::Info));
}

#[test]
fn doctor_lockstate_hash_mismatch_detected() {
    let dir = setup_project();
    let content = b"my-workflow-content";
    fs::write(dir.path().join(".vo/workflows/my-wf"), content).unwrap();
    let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    fs::write(dir.path().join("vo.lock"), format!("my-wf {bad_hash}\n")).unwrap();
    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lock-integrity" && c.severity == Severity::Error));
}

#[test]
fn doctor_storage_probe_file_cleaned_up() {
    let dir = setup_project();
    let _report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(!dir.path().join(".vo/storage/.doctor-probe").exists());
}

#[test]
fn doctor_config_with_toml_parse_error() {
    let dir = setup_project();
    fs::write(dir.path().join("config.toml"), "{{{invalid toml}}").unwrap();
    let report = check_config_validation(dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "config-parseable" && c.severity == Severity::Error));
}

#[test]
fn doctor_config_with_missing_engine_url_field() {
    let dir = setup_project();
    fs::write(
        dir.path().join("config.toml"),
        "[engine]\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let report = check_config_validation(dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "config-engine-url" && c.severity == Severity::Warn));
}

#[test]
fn doctor_config_with_missing_storage_path_field() {
    let dir = setup_project();
    fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\n",
    )
    .unwrap();
    let report = check_config_validation(dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "config-storage-path" && c.severity == Severity::Warn));
}

#[test]
fn format_report_with_all_healthy() {
    let dir = setup_project();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let report = run_doctor(&config).unwrap();
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains(dir.path().to_str().unwrap()));
    if report.is_healthy() {
        assert!(stdout.contains("All checks passed"));
    }
}

#[test]
fn format_report_json_roundtrip() {
    let dir = setup_project();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let report = run_doctor(&config).unwrap();
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["healthy"].is_boolean());
    assert!(parsed["categories"].is_array());
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 5);
}

#[test]
fn category_report_push_adds_checks() {
    let mut report = CategoryReport::new(CheckCategory::Workspace);
    assert!(report.checks.is_empty());
    report.push("test-check", Severity::Info, "message".into());
    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].check, "test-check");
    assert_eq!(report.checks[0].severity, Severity::Info);
    assert_eq!(report.checks[0].message, "message");
}

#[test]
fn category_report_warnings_filters_correctly() {
    let mut report = CategoryReport::new(CheckCategory::LockState);
    report.push("c1", Severity::Info, "info".into());
    report.push("c2", Severity::Warn, "warn".into());
    report.push("c3", Severity::Error, "err".into());
    report.push("c4", Severity::Warn, "warn2".into());
    let warns: Vec<_> = report.warnings().collect();
    assert_eq!(warns.len(), 2);
    assert!(warns.iter().all(|c| c.severity == Severity::Warn));
}

#[test]
fn check_result_equality_same() {
    let r1 = CheckResult {
        check: "a",
        severity: Severity::Info,
        message: "msg".into(),
    };
    let r2 = CheckResult {
        check: "a",
        severity: Severity::Info,
        message: "msg".into(),
    };
    assert_eq!(r1, r2);
}

#[test]
fn check_result_inequality_different_message() {
    let r1 = CheckResult {
        check: "a",
        severity: Severity::Info,
        message: "msg1".into(),
    };
    let r2 = CheckResult {
        check: "a",
        severity: Severity::Info,
        message: "msg2".into(),
    };
    assert_ne!(r1, r2);
}

#[test]
fn severity_ord_chain() {
    assert!(Severity::Info < Severity::Warn);
    assert!(Severity::Warn < Severity::Error);
    assert!(Severity::Info < Severity::Error);
}

#[test]
fn check_category_display_all_variants() {
    assert_eq!(format!("{}", CheckCategory::Workspace), "workspace");
    assert_eq!(format!("{}", CheckCategory::LockState), "lock-state");
    assert_eq!(
        format!("{}", CheckCategory::SubprocessLiveness),
        "subprocess-liveness"
    );
    assert_eq!(
        format!("{}", CheckCategory::StorageIntegrity),
        "storage-integrity"
    );
    assert_eq!(
        format!("{}", CheckCategory::ConfigValidation),
        "config-validation"
    );
}

#[test]
fn doctor_not_initialized_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_doctor(&config);
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn doctor_storage_with_journal_suffix() {
    let dir = setup_project();
    fs::write(dir.path().join(".vo/storage/data-journal"), "j").unwrap();
    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-wal" && c.severity == Severity::Warn));
}

#[test]
fn doctor_storage_with_wal_suffix() {
    let dir = setup_project();
    fs::write(dir.path().join(".vo/storage/data-wal"), "w").unwrap();
    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-wal" && c.severity == Severity::Warn));
}
