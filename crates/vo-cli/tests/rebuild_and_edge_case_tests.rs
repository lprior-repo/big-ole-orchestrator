#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::{
    dispatch, interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command,
};
use vo_cli::commands::init::{InitConfig, InitError};
use vo_cli::commands::lock::{LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{
    RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};
use vo_cli::commands::doctor::{DoctorConfig, DoctorError};
use vo_cli::commands::doctor_checks::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};

// ============================================================
// Rebuild Command Parsing
// ============================================================

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).expect("parse");
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert!(projection_id.is_none());
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_projection_id() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--projection-id",
        "order-summary",
    ])
    .expect("parse");
    match cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id.as_deref(), Some("order-summary"));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_list_flag() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).expect("parse");
    match cli.command {
        Command::Rebuild { list_projections, .. } => {
            assert!(list_projections);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_force_flag() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--force"]).expect("parse");
    match cli.command {
        Command::Rebuild { force, .. } => {
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_all_flags() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        "/my/proj",
        "--projection-id",
        "p1",
        "--list",
        "--force",
    ])
    .expect("parse");
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/proj"));
            assert_eq!(projection_id.as_deref(), Some("p1"));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_custom_project_dir() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        "/data/app",
    ])
    .expect("parse");
    match cli.command {
        Command::Rebuild { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("/data/app"));
        }
        _ => panic!("expected Rebuild"),
    }
}

// ============================================================
// Rebuild Business Logic
// ============================================================

#[test]
fn rebuild_not_initialized_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_returns_empty_projections() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.expect("ok");
    assert!(matches!(report.status, RebuildStatus::Listed(ref v) if v.is_empty()));
}

#[test]
fn rebuild_without_projection_id_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_err());
}

#[test]
fn rebuild_with_projection_id_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("order-summary".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.expect("ok");
    assert_eq!(report.projection_id.as_deref(), Some("order-summary"));
    assert!(matches!(report.status, RebuildStatus::Completed));
}

// ============================================================
// RebuildReport format_progress
// ============================================================

#[test]
fn rebuild_report_format_progress_listed() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["p1".into(), "p2".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("Registered projections"));
    assert!(output.contains("p1"));
    assert!(output.contains("p2"));
}

#[test]
fn rebuild_report_format_progress_started() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("42"));
}

#[test]
fn rebuild_report_format_progress_in_progress() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::InProgress {
            progress_percent: 75,
            at_sequence: 3000,
        },
        events_applied: 3000,
        duration_ms: 200,
    };
    let output = report.format_progress();
    assert!(output.contains("75%"));
    assert!(output.contains("3000"));
}

#[test]
fn rebuild_report_format_progress_completed() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Completed,
        events_applied: 500,
        duration_ms: 120,
    };
    let output = report.format_progress();
    assert!(output.contains("completed"));
    assert!(output.contains("500 events"));
    assert!(output.contains("120ms"));
}

#[test]
fn rebuild_report_format_progress_failed() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Failed {
            reason: "disk full".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("failed"));
    assert!(output.contains("disk full"));
}

#[test]
fn rebuild_report_format_progress_noop() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("skipped"));
    assert!(output.contains("already up to date"));
}

// ============================================================
// RebuildError display
// ============================================================

#[test]
fn rebuild_error_not_initialized_display() {
    let err = RebuildError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    assert!(err.to_string().contains("/proj"));
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn rebuild_error_projection_not_found_display() {
    let err = RebuildError::ProjectionNotFound("my-proj".into());
    assert!(err.to_string().contains("my-proj"));
}

#[test]
fn rebuild_error_rebuild_failed_display() {
    let err = RebuildError::RebuildFailed("timeout".into());
    assert!(err.to_string().contains("timeout"));
}

#[test]
fn rebuild_error_unsupported_schema_version_display() {
    let err = RebuildError::UnsupportedSchemaVersion(99);
    assert!(err.to_string().contains("99"));
}

#[test]
fn rebuild_error_rebuild_in_progress_display() {
    let err = RebuildError::RebuildInProgress("p1".into());
    assert!(err.to_string().contains("p1"));
    assert!(err.to_string().contains("in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_display() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "abc".into(),
        actual: "def".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("abc"));
    assert!(msg.contains("def"));
    assert!(msg.contains("mismatch"));
}

#[test]
fn rebuild_error_engine_display() {
    let err = RebuildError::Engine("connection refused".into());
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn rebuild_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
    let err: RebuildError = io_err.into();
    assert!(err.to_string().contains("pipe broke"));
}

// ============================================================
// Init Edge Cases
// ============================================================

#[test]
fn init_rejects_symlink_project_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("real");
    std::fs::create_dir_all(&target).expect("mkdir");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");

    let config = InitConfig {
        project_dir: link.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::commands::init::run_init(&config);
    assert!(
        matches!(result, Err(InitError::SymlinkTarget { .. })),
        "expected SymlinkTarget, got {:?}",
        result
    );
}

#[test]
fn init_rejects_file_as_project_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("not_a_dir");
    std::fs::write(&file_path, b"content").expect("write");

    let config = InitConfig {
        project_dir: file_path.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::commands::init::run_init(&config);
    assert!(
        matches!(result, Err(InitError::NotDirectory { .. })),
        "expected NotDirectory, got {:?}",
        result
    );
}

#[test]
fn init_creates_workflows_subdir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config).expect("init");
    assert!(dir.path().join(".vo/workflows").is_dir());
}

#[test]
fn init_config_toml_has_newlines() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config).expect("init");
    let content = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(content.contains("\n\n"));
}

// ============================================================
// Lock Edge Cases
// ============================================================

#[test]
fn lock_file_format_is_name_space_hash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vo_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&vo_dir).expect("mkdir");
    std::fs::write(
        vo_dir.join("wf1"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00],
    )
    .expect("write");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = vo_cli::commands::lock::run_lock(&config).expect("lock");
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("wf1"));

    let lock_content = std::fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).expect("read");
    let parts: Vec<&str> = lock_content.trim().splitn(2, ' ').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "wf1");
    assert_eq!(parts[1].len(), 64);
}

#[test]
fn lock_with_multiple_workflows_sorts_by_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vo_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&vo_dir).expect("mkdir");
    for name in ["c_wf", "a_wf", "b_wf"] {
        std::fs::write(vo_dir.join(name), b"\x7fELF\x00\x00\x00\x00").expect("write");
    }

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = vo_cli::commands::lock::run_lock(&config).expect("lock");
    assert_eq!(lockmap.len(), 3);

    let keys: Vec<&String> = lockmap.keys().collect();
    assert_eq!(keys[0], "a_wf");
    assert_eq!(keys[1], "b_wf");
    assert_eq!(keys[2], "c_wf");
}

#[test]
fn lock_rejects_non_directory_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("a_file");
    std::fs::write(&file_path, b"not a dir").expect("write");

    let config = LockConfig {
        project_dir: file_path,
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_ignores_subdirectories_in_workflows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vo_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&vo_dir).expect("mkdir");
    std::fs::write(vo_dir.join("real_wf"), b"\x7fELF\x00").expect("write");
    std::fs::create_dir_all(vo_dir.join("subdir")).expect("mkdir");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = vo_cli::commands::lock::run_lock(&config).expect("lock");
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("real_wf"));
}

// ============================================================
// End-to-End Rebuild Dispatch
// ============================================================

#[tokio::test]
async fn e2e_rebuild_list_dispatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        path,
        "--list",
    ])
    .expect("parse");
    let result = dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_rebuild_not_initialized_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        path,
        "--projection-id",
        "p1",
    ])
    .expect("parse");
    let result = dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_rebuild_with_projection_id_dispatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        path,
        "--projection-id",
        "orders",
    ])
    .expect("parse");
    let result = dispatch(cli).await;
    assert!(result.is_ok());
}

// ============================================================
// parse_strict_numeric Additional Edge Cases
// ============================================================

#[test]
fn parse_strict_numeric_accepts_leading_zeros() {
    assert_eq!(parse_strict_numeric("007").unwrap(), 7);
    assert_eq!(parse_strict_numeric("000").unwrap(), 0);
}

#[test]
fn parse_strict_numeric_rejects_float() {
    assert!(parse_strict_numeric("3.14").is_err());
}

#[test]
fn parse_strict_numeric_rejects_binary() {
    assert!(parse_strict_numeric("0b1010").is_err());
}

#[test]
fn parse_strict_numeric_rejects_octal() {
    assert!(parse_strict_numeric("0o777").is_err());
}

#[test]
fn parse_strict_numeric_rejects_alphanumeric() {
    assert!(parse_strict_numeric("12abc34").is_err());
}

#[test]
fn parse_strict_numeric_rejects_tab() {
    assert!(parse_strict_numeric("\t42").is_err());
}

#[test]
fn parse_strict_numeric_rejects_newline() {
    assert!(parse_strict_numeric("42\n").is_err());
}

#[test]
fn parse_strict_numeric_u64_boundary_minus_one() {
    assert_eq!(parse_strict_numeric("18446744073709551614").unwrap(), u64::MAX - 1);
}

// ============================================================
// CLI Parsing: Unknown Subcommand, Extra Args
// ============================================================

#[test]
fn parse_unknown_subcommand_fails() {
    let result = interpret_cli_from(vec!["vo", "foobar"]);
    assert!(result.is_err());
}

#[test]
fn parse_gc_with_unknown_flag_fails() {
    let result = interpret_cli_from(vec!["vo", "gc", "--unknown-flag"]);
    assert!(result.is_err());
}

#[test]
fn parse_check_with_extra_positional_rejected() {
    let result = interpret_cli_from(vec!["vo", "check", "/tmp/a", "/tmp/b"]);
    assert!(result.is_err());
}

#[test]
fn parse_doctor_custom_project_dir() {
    let cli = interpret_cli_from(vec![
        "vo",
        "doctor",
        "--project-dir",
        "/custom/proj",
    ])
    .expect("parse");
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/custom/proj"));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_lock_custom_project_dir() {
    let cli = interpret_cli_from(vec![
        "vo",
        "lock",
        "--project-dir",
        "/my/workspace",
    ])
    .expect("parse");
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/my/workspace"));
        }
        _ => panic!("expected Lock"),
    }
}

// ============================================================
// Exit Code: RebuildError
// ============================================================

#[test]
fn exit_code_rebuild_error_is_1() {
    let err = CliError::Rebuild(RebuildError::NotInitialized {
        path: PathBuf::from("/p"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

// ============================================================
// Doctor Report Mixed Categories
// ============================================================

#[test]
fn doctor_report_mixed_severity_across_categories() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.checks.push(CheckResult {
        check: "ok",
        severity: Severity::Info,
        message: "fine".into(),
    });
    let mut cat2 = CategoryReport::new(CheckCategory::LockState);
    cat2.checks.push(CheckResult {
        check: "bad",
        severity: Severity::Error,
        message: "broken".into(),
    });
    let mut cat3 = CategoryReport::new(CheckCategory::StorageIntegrity);
    cat3.checks.push(CheckResult {
        check: "warn",
        severity: Severity::Warn,
        message: "careful".into(),
    });

    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![cat1, cat2, cat3],
    };

    assert!(!report.is_healthy());
    let errors: Vec<_> = report.errors().collect();
    let warnings: Vec<_> = report.warnings().collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(warnings.len(), 1);

    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("1 error(s)"));
    assert!(stderr.contains("1 warning(s)"));
}

#[test]
fn doctor_report_json_mixed_categories() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.checks.push(CheckResult {
        check: "e1",
        severity: Severity::Error,
        message: "err".into(),
    });
    let cat2 = CategoryReport::new(CheckCategory::ConfigValidation);

    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![cat1, cat2],
    };

    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(!parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 1);
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 2);
    assert!(!parsed["categories"][0]["healthy"].as_bool().unwrap());
    assert!(parsed["categories"][1]["healthy"].as_bool().unwrap());
}

// ============================================================
// E2E: Init → Lock → Doctor Full Cycle with Config Variants
// ============================================================

#[tokio::test]
async fn e2e_init_with_custom_engine_url_creates_correct_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        path,
        "--engine-url",
        "http://prod:9090",
        "--storage-path",
        "/data/storage",
    ])
    .expect("parse");
    dispatch(cli).await.expect("init");

    let config = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(config.contains("http://prod:9090"));
    assert!(config.contains("/data/storage"));
}

#[tokio::test]
async fn e2e_init_then_doctor_with_workflow_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli =
        interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    dispatch(init_cli).await.expect("init");

    std::fs::write(
        dir.path().join(".vo/workflows/app"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00],
    )
    .expect("write workflow");

    let lock_cli =
        interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).expect("parse");
    dispatch(lock_cli).await.expect("lock");

    let doctor_cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).expect("parse");
    let result = dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_doctor_report_after_corrupt_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli =
        interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    dispatch(init_cli).await.expect("init");

    std::fs::write(dir.path().join("vo.lock"), "corrupted-lock-content\n").expect("write");

    let doctor_cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).expect("parse");
    let result = dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

// ============================================================
// Command Debug Format
// ============================================================

#[test]
fn command_debug_all_variants() {
    let commands = vec![
        format!("{:?}", Command::Purge { instance: "i".into() }),
        format!("{:?}", Command::Check { path: PathBuf::from("/tmp") }),
        format!("{:?}", Command::Gc { engine_url: "u".into(), dry_run: false }),
        format!("{:?}", Command::Init {
            project_dir: PathBuf::from("."),
            engine_url: "u".into(),
            storage_path: PathBuf::from("s"),
        }),
        format!("{:?}", Command::Lock { project_dir: PathBuf::from(".") }),
        format!("{:?}", Command::Doctor { project_dir: PathBuf::from(".") }),
        format!("{:?}", Command::Rebuild {
            project_dir: PathBuf::from("."),
            projection_id: None,
            list_projections: false,
            force: false,
        }),
    ];
    assert_eq!(commands.len(), 7);
    for debug_str in &commands {
        assert!(!debug_str.is_empty());
    }
}
