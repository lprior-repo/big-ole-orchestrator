use std::path::PathBuf;
use vo_cli::commands::init::{InitConfig, run_init};
use vo_cli::commands::lock::{LockConfig, run_lock};
use vo_cli::commands::doctor::{DoctorConfig, DoctorError, run_doctor};
use vo_cli::{interpret_cli_from, Command, dispatch, map_error_to_exit_code, CliError};

fn setup_init_project(dir: &std::path::Path) {
    let config = InitConfig {
        project_dir: dir.to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config).expect("init");
}

// ============================================================
// Doctor command tests
// ============================================================

#[test]
fn doctor_passes_healthy_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let report = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() });
    assert!(report.is_ok());
    assert!(report.expect("ok").is_healthy());
}

#[test]
fn doctor_fails_uninit_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() });
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn doctor_detects_missing_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let report = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() }).expect("ok");
    assert!(!report.is_healthy());
    assert!(report.errors().any(|c| c.message.contains("config")));
}

#[test]
#[test]
fn doctor_detects_missing_workflows_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n"
    ).expect("write");
    let report = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() }).expect("ok");
    assert!(report.warnings().any(|c| c.message.contains("workflows")));
}

#[test]
fn doctor_validates_lockfile_hashes() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let wf = dir.path().join(".vo").join("workflows").join("mywf");
    std::fs::write(&wf, b"original").expect("write");
    run_lock(&LockConfig { project_dir: dir.path().to_path_buf() }).expect("lock");
    std::fs::write(&wf, b"tampered").expect("tamper");
    let report = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() }).expect("ok");
    assert!(!report.is_healthy());
    assert!(report.errors().any(|c| c.message.contains("hash") || c.message.contains("mismatch")));
}

#[test]
fn doctor_detects_binary_missing_from_lockfile() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let wf = dir.path().join(".vo").join("workflows").join("wf-a");
    std::fs::write(&wf, b"data-a").expect("write");
    run_lock(&LockConfig { project_dir: dir.path().to_path_buf() }).expect("lock");
    std::fs::remove_file(&wf).expect("remove");
    let report = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() }).expect("ok");
    assert!(!report.is_healthy());
    assert!(report.errors().any(|c| c.message.contains("missing")));
}

#[test]
fn doctor_passes_with_no_lockfile() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let report = run_doctor(&DoctorConfig { project_dir: dir.path().to_path_buf() }).expect("ok");
    assert!(report.is_healthy());
}

#[test]
fn doctor_error_display() {
    let err = DoctorError::NotInitialized { path: PathBuf::from("/tmp/test") };
    assert!(err.to_string().contains("not initialized"));
}

// ============================================================
// CLI integration: init/lock/doctor parsing + dispatch
// ============================================================

#[test]
fn interpret_cli_parses_init_subcommand() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { project_dir, engine_url, .. } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://localhost:3000");
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn interpret_cli_parses_init_with_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", "/tmp/myproject"]).expect("parse");
    assert!(matches!(cli.command, Command::Init { .. }));
}

#[test]
fn interpret_cli_parses_lock_subcommand() {
    let cli = interpret_cli_from(vec!["vo", "lock"]).expect("parse");
    assert!(matches!(cli.command, Command::Lock { .. }));
}

#[test]
fn interpret_cli_parses_doctor_subcommand() {
    let cli = interpret_cli_from(vec!["vo", "doctor"]).expect("parse");
    assert!(matches!(cli.command, Command::Doctor { .. }));
}

#[test]
fn interpret_cli_parses_doctor_with_dir() {
    let cli = interpret_cli_from(vec!["vo", "doctor", "--project-dir", "/tmp/proj"]).expect("parse");
    match cli.command {
        Command::Doctor { project_dir } => assert_eq!(project_dir, PathBuf::from("/tmp/proj")),
        _ => panic!("expected Doctor"),
    }
}

#[tokio::test]
async fn dispatch_init_creates_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = interpret_cli_from(vec![
        "vo", "init", "--project-dir", dir.path().to_str().expect("path"),
    ]).expect("parse");
    assert!(dispatch(cli).await.is_ok());
    assert!(dir.path().join(".vo").is_dir());
}

#[tokio::test]
async fn dispatch_lock_on_empty_workflows_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let init_cli = interpret_cli_from(vec![
        "vo", "init", "--project-dir", dir.path().to_str().expect("path"),
    ]).expect("parse");
    dispatch(init_cli).await.expect("init");
    let lock_cli = interpret_cli_from(vec![
        "vo", "lock", "--project-dir", dir.path().to_str().expect("path"),
    ]).expect("parse");
    assert!(dispatch(lock_cli).await.is_err());
}

#[tokio::test]
async fn dispatch_doctor_on_healthy_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().to_str().expect("path");
    dispatch(interpret_cli_from(vec!["vo", "init", "--project-dir", p]).expect("parse")).await.expect("init");
    let result = dispatch(interpret_cli_from(vec!["vo", "doctor", "--project-dir", p]).expect("parse")).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn dispatch_init_lock_doctor_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().to_str().expect("path");
    dispatch(interpret_cli_from(vec!["vo", "init", "--project-dir", p]).expect("parse")).await.expect("init");
    std::fs::write(dir.path().join(".vo").join("workflows").join("mywf"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00]).expect("write");
    dispatch(interpret_cli_from(vec!["vo", "lock", "--project-dir", p]).expect("parse")).await.expect("lock");
    let result = dispatch(interpret_cli_from(vec!["vo", "doctor", "--project-dir", p]).expect("parse")).await;
    assert!(result.is_ok());
}

#[test]
fn map_error_returns_1_for_init_error() {
    assert_eq!(map_error_to_exit_code(&CliError::Init(
        vo_cli::InitError::DirNotFound { path: PathBuf::from("/tmp/x") }
    )), 1);
}

#[test]
fn map_error_returns_1_for_lock_error() {
    assert_eq!(map_error_to_exit_code(&CliError::Lock(
        vo_cli::LockError::NotInitialized { path: PathBuf::from("/tmp/x") }
    )), 1);
}

#[test]
fn map_error_returns_1_for_doctor_error() {
    assert_eq!(map_error_to_exit_code(&CliError::Doctor(
        DoctorError::NotInitialized { path: PathBuf::from("/tmp/x") }
    )), 1);
}

#[test]
fn lock_multiple_binaries_sorted_output() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let wf_dir = dir.path().join(".vo").join("workflows");
    std::fs::write(wf_dir.join("zebra"), b"z").expect("write");
    std::fs::write(wf_dir.join("alpha"), b"a").expect("write");
    let map = run_lock(&LockConfig { project_dir: dir.path().to_path_buf() }).expect("lock");
    let names: Vec<_> = map.keys().collect();
    assert_eq!(names[0], "alpha");
    assert_eq!(names[1], "zebra");
}

#[test]
fn doctor_report_empty_categories_is_healthy() {
    use vo_cli::DoctorReport;
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![],
    };
    assert!(report.is_healthy());
}
