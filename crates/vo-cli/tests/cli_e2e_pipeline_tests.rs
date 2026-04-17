#![allow(clippy::redundant_pattern_matching)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use vo_cli::commands::check::{validate_binary_header, BinaryFormat, CheckError};
use vo_cli::commands::doctor::{run_doctor, DoctorConfig, DoctorError};
use vo_cli::commands::doctor_checks::{
    check_config_validation, check_lock_state, check_storage_integrity, check_workspace,
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
use vo_cli::commands::gc::{GcConfig, GcError, GcSummary};
use vo_cli::commands::history::{
    get_history, load_history, redo_command, save_history, undo_command, HistoryConfig,
    HistoryError,
};
use vo_cli::commands::init::{
    run_init, InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME, WORKFLOWS_DIR_NAME,
};
use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};
use vo_cli::utils::{file_hash, sha256_hex};
use vo_cli::{
    create_dispatcher_v2, dispatch, dispatch_v2, interpret_cli_from, map_error_to_exit_code,
    parse_strict_numeric, CliError, Command, CommandContext, CommandDispatcher,
    CommandDispatcherV2, DefaultDispatchContext, DispatchContext, HandlerRegistry,
    LoggingMiddlewareV2, MetricsMiddlewareV2, MiddlewareResult, MiddlewareV2,
};

fn setup_project(dir: &Path) {
    let vo_dir = dir.join(".vo");
    fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

fn create_elf_binary(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();
    path
}

fn create_workflow_binary(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let wf_dir = dir.join(".vo/workflows");
    fs::create_dir_all(&wf_dir).unwrap();
    let path = wf_dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

// ============================================================
// E2E Pipeline: init -> lock -> doctor -> check -> rebuild
// ============================================================

#[test]
fn e2e_full_pipeline_happy_path() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path();

    run_init(&InitConfig {
        project_dir: project_dir.to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    })
    .unwrap();

    assert!(project_dir.join(".vo").is_dir());
    assert!(project_dir.join(".vo/workflows").is_dir());
    assert!(project_dir.join(CONFIG_FILE_NAME).exists());

    create_workflow_binary(project_dir, "test-wf", &[0x7F, 0x45, 0x4C, 0x46, 0x01]);

    let lockmap = run_lock(&LockConfig {
        project_dir: project_dir.to_path_buf(),
    })
    .unwrap();
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("test-wf"));
    assert!(project_dir.join(LOCK_FILE_NAME).exists());

    let report = run_doctor(&DoctorConfig {
        project_dir: project_dir.to_path_buf(),
    })
    .unwrap();
    assert!(report.is_healthy());

    let rebuild_report = run_rebuild(&RebuildConfig {
        project_dir: project_dir.to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    })
    .unwrap();
    assert!(matches!(rebuild_report.status, RebuildStatus::Listed(_)));
}

#[test]
fn e2e_init_lock_tamper_doctor_catches_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path();

    run_init(&InitConfig {
        project_dir: project_dir.to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    })
    .unwrap();

    create_workflow_binary(project_dir, "my-wf", b"original content");

    run_lock(&LockConfig {
        project_dir: project_dir.to_path_buf(),
    })
    .unwrap();

    fs::write(project_dir.join(".vo/workflows/my-wf"), b"tampered content").unwrap();

    let report = run_doctor(&DoctorConfig {
        project_dir: project_dir.to_path_buf(),
    })
    .unwrap();
    assert!(!report.is_healthy());
    assert!(report.errors().any(|e| e.check == "lock-integrity"));
}

#[test]
fn e2e_init_idempotent_same_config() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path();

    let config = InitConfig {
        project_dir: project_dir.to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };

    let vo1 = run_init(&config).unwrap();
    let vo2 = run_init(&config).unwrap();
    assert_eq!(vo1, vo2);
}

#[test]
fn e2e_init_rejects_different_config() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path();

    let config1 = InitConfig {
        project_dir: project_dir.to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config1).unwrap();

    let config2 = InitConfig {
        project_dir: project_dir.to_path_buf(),
        engine_url: "http://different:9999".to_string(),
        storage_path: PathBuf::from(".vo/other"),
    };
    let result = run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn e2e_doctor_without_init_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    });
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn e2e_rebuild_without_init_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_rebuild(&RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("proj-1".to_string()),
        list_projections: false,
        force: false,
        schema_version: None,
    });
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn e2e_rebuild_requires_projection_id() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let result = run_rebuild(&RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    });
    assert!(matches!(result, Err(RebuildError::Engine(_))));
}

#[test]
fn e2e_lock_without_init_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = run_lock(&LockConfig {
        project_dir: dir.path().to_path_buf(),
    });
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn e2e_lock_with_empty_workflows_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path();
    fs::create_dir_all(project_dir.join(".vo/workflows")).unwrap();
    fs::write(project_dir.join(CONFIG_FILE_NAME), "").unwrap();

    let result = run_lock(&LockConfig {
        project_dir: project_dir.to_path_buf(),
    });
    assert!(matches!(result, Err(LockError::Empty { .. })));
}

#[test]
fn e2e_check_valid_elf_binary() {
    let dir = tempfile::tempdir().unwrap();
    let path = create_elf_binary(dir.path(), "test.bin");
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::Elf));
}

#[test]
fn e2e_check_nonexistent_file() {
    let result = validate_binary_header(Path::new("/tmp/nonexistent-vo-test-xyz"));
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

// ============================================================
// Command parsing: flag combinations
// ============================================================

#[test]
fn parse_init_all_defaults() {
    let cli = interpret_cli_from(vec!["vo", "init"]).unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_custom_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", "/custom/dir"]).unwrap();
    match cli.command {
        Command::Init { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("/custom/dir"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_all_custom() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/my/project",
        "--engine-url",
        "http://engine:8080",
        "--storage-path",
        "/data/vo",
    ])
    .unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/project"));
            assert_eq!(engine_url, "http://engine:8080");
            assert_eq!(storage_path, PathBuf::from("/data/vo"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).unwrap();
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
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "my-proj"]).unwrap();
    match cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id, Some("my-proj".to_string()));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_list_only() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).unwrap();
    match cli.command {
        Command::Rebuild {
            list_projections, ..
        } => {
            assert!(list_projections);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_force_with_projection_id() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--projection-id",
        "proj-1",
        "--force",
    ])
    .unwrap();
    match cli.command {
        Command::Rebuild {
            projection_id,
            force,
            ..
        } => {
            assert_eq!(projection_id, Some("proj-1".to_string()));
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
        "/tmp",
        "--projection-id",
        "p1",
        "--list",
        "--force",
    ])
    .unwrap();
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("/tmp"));
            assert_eq!(projection_id, Some("p1".to_string()));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_purge_basic() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", "inst-123"]).unwrap();
    assert_eq!(
        cli.command,
        Command::Purge {
            instance: "inst-123".to_string()
        }
    );
}

#[test]
fn parse_purge_empty_instance() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", ""]).unwrap();
    assert_eq!(
        cli.command,
        Command::Purge {
            instance: "".to_string()
        }
    );
}

#[test]
fn parse_purge_special_chars_instance() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", "inst-àéïôü-测试"]).unwrap();
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "inst-àéïôü-测试");
        }
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_lock_defaults() {
    let cli = interpret_cli_from(vec!["vo", "lock"]).unwrap();
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_lock_custom_dir() {
    let cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", "/my/project"]).unwrap();
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/my/project"));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_doctor_defaults() {
    let cli = interpret_cli_from(vec!["vo", "doctor"]).unwrap();
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_doctor_custom_dir() {
    let cli = interpret_cli_from(vec!["vo", "doctor", "--project-dir", "/health"]).unwrap();
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/health"));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_version_flag() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayVersion
    );
}

#[test]
fn parse_no_args_returns_missing_subcommand() {
    let result = interpret_cli_from(vec!["vo"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::MissingSubcommand
    );
}

#[test]
fn parse_unknown_subcommand() {
    let result = interpret_cli_from(vec!["vo", "foobar"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

#[test]
fn parse_check_path_with_spaces() {
    let cli = interpret_cli_from(vec!["vo", "check", "/path/with spaces/bin"]).unwrap();
    match cli.command {
        Command::Check { workflow: false, path } => {
            assert_eq!(path, PathBuf::from("/path/with spaces/bin"));
        }
        _ => panic!("expected Check"),
    }
}

// ============================================================
// Error display quality
// ============================================================

#[test]
fn history_error_display_all_variants() {
    let e1 = HistoryError::HistoryFileNotFound {
        path: PathBuf::from("/hist.json"),
    };
    assert!(e1.to_string().contains("/hist.json"));

    let e2 = HistoryError::ReadFailed {
        reason: "disk error".into(),
    };
    assert!(e2.to_string().contains("disk error"));

    let e3 = HistoryError::WriteFailed {
        reason: "permission".into(),
    };
    assert!(e3.to_string().contains("permission"));

    let e4 = HistoryError::InvalidFormat {
        reason: "bad json".into(),
    };
    assert!(e4.to_string().contains("bad json"));
}

#[test]
fn rebuild_error_display_all_variants() {
    let e1 = RebuildError::NotInitialized {
        path: PathBuf::from("/nope"),
    };
    assert!(e1.to_string().contains("/nope"));

    let e2 = RebuildError::ProjectionNotFound("proj-x".into());
    assert!(e2.to_string().contains("proj-x"));

    let e3 = RebuildError::RebuildFailed("OOM".into());
    assert!(e3.to_string().contains("OOM"));

    let e4 = RebuildError::UnsupportedSchemaVersion(99);
    assert!(e4.to_string().contains("99"));

    let e5 = RebuildError::RebuildInProgress("proj-y".into());
    assert!(e5.to_string().contains("proj-y"));

    let e6 = RebuildError::IdempotencyMismatch {
        expected: "abc".into(),
        actual: "def".into(),
    };
    assert!(e6.to_string().contains("abc"));
    assert!(e6.to_string().contains("def"));

    let e7 = RebuildError::Io {
        path: PathBuf::from("/io"),
        reason: "read".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e7.to_string().contains("/io"));

    let e8 = RebuildError::Engine("engine fail".into());
    assert!(e8.to_string().contains("engine fail"));
}

#[test]
fn init_error_display_all_variants() {
    let e1 = InitError::DirNotFound {
        path: PathBuf::from("/no-dir"),
    };
    assert!(e1.to_string().contains("/no-dir"));

    let e2 = InitError::NotDirectory {
        path: PathBuf::from("/file"),
    };
    assert!(e2.to_string().contains("/file"));

    let e3 = InitError::AlreadyInitialized {
        path: PathBuf::from("/exists"),
    };
    assert!(e3.to_string().contains("/exists"));

    let e4 = InitError::PermissionDenied {
        path: PathBuf::from("/denied"),
        reason: "nope".into(),
    };
    assert!(e4.to_string().contains("/denied"));

    let e5 = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    assert!(e5.to_string().contains("/link"));
    assert!(e5.to_string().contains("symlink"));
}

#[test]
fn lock_error_display_all_variants() {
    let e1 = LockError::NotInitialized {
        path: PathBuf::from("/no"),
    };
    assert!(e1.to_string().contains("/no"));

    let e2 = LockError::NoWorkflowsDir {
        path: PathBuf::from("/no-wf"),
    };
    assert!(e2.to_string().contains("/no-wf"));

    let e3 = LockError::Io {
        path: PathBuf::from("/io"),
        reason: "fail".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e3.to_string().contains("/io"));

    let e4 = LockError::LockWrite {
        reason: "disk full".into(),
    };
    assert!(e4.to_string().contains("disk full"));

    let e5 = LockError::Empty {
        path: PathBuf::from("/empty"),
    };
    assert!(e5.to_string().contains("/empty"));
}

#[test]
fn doctor_error_display() {
    let e1 = DoctorError::NotInitialized {
        path: PathBuf::from("/no-init"),
    };
    assert!(e1.to_string().contains("/no-init"));

    let e2 = DoctorError::Io {
        path: PathBuf::from("/io"),
        reason: "read err".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e2.to_string().contains("/io"));
}

#[test]
fn gc_error_display_all_variants() {
    let e1 = GcError::EngineUnreachable {
        url: "http://fail".into(),
        reason: "timeout".into(),
    };
    assert!(e1.to_string().contains("http://fail"));
    assert!(e1.to_string().contains("timeout"));

    let e2 = GcError::EngineHttpError {
        url: "http://err".into(),
        status: 500,
    };
    assert!(e2.to_string().contains("500"));

    let e3 = GcError::InvalidApiResponse {
        reason: "bad json".into(),
    };
    assert!(e3.to_string().contains("bad json"));

    let e4 = GcError::VersionsDirNotFound {
        path: PathBuf::from("/no-versions"),
    };
    assert!(e4.to_string().contains("/no-versions"));

    let e5 = GcError::DeleteFailed {
        path: PathBuf::from("/del"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope"),
    };
    assert!(e5.to_string().contains("/del"));
}

#[test]
fn cli_error_display_dispatch() {
    let e = CliError::Dispatch("something went wrong".to_string());
    assert!(e.to_string().contains("something went wrong"));
}

#[test]
fn cli_error_display_invalid_numeric() {
    let e = CliError::InvalidNumeric("bad number".to_string());
    assert!(e.to_string().contains("bad number"));
}

// ============================================================
// Output format correctness
// ============================================================

#[test]
fn format_report_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/empty"),
        categories: vec![],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_with_errors_and_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("check-a", Severity::Info, "all good".into());
    cat.push("check-b", Severity::Warn, "watch out".into());
    cat.push("check-c", Severity::Error, "broken".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/test"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("all good"));
    assert!(stderr.contains("watch out"));
    assert!(stderr.contains("broken"));
    assert!(stderr.contains("error(s)"));
    assert!(stderr.contains("warning(s)"));
}

#[test]
fn format_report_json_structure() {
    let mut cat = CategoryReport::new(CheckCategory::LockState);
    cat.push("lock", Severity::Info, "valid".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/json-test"),
        categories: vec![cat],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["project_dir"].as_str(), Some("/tmp/json-test"));
    assert!(parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 0);
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 1);

    let cat_val = &parsed["categories"][0];
    assert_eq!(cat_val["category"].as_str(), Some("lock-state"));
    assert!(cat_val["healthy"].as_bool().unwrap());
    assert_eq!(cat_val["checks"].as_array().unwrap().len(), 1);

    let check = &cat_val["checks"][0];
    assert_eq!(check["check"].as_str(), Some("lock"));
    assert_eq!(check["severity"].as_str(), Some("info"));
}

#[test]
fn format_report_json_severity_serialization() {
    for (sev, expected) in [
        (Severity::Info, "info"),
        (Severity::Warn, "warn"),
        (Severity::Error, "error"),
    ] {
        let mut cat = CategoryReport::new(CheckCategory::Workspace);
        cat.push("test", sev, "msg".into());
        let report = DoctorReport {
            project_dir: PathBuf::from("/tmp"),
            categories: vec![cat],
        };
        let json = format_report_json(&report);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["categories"][0]["checks"][0]["severity"].as_str(),
            Some(expected)
        );
    }
}

#[test]
fn rebuild_format_progress_all_statuses() {
    let listed = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["p1".into(), "p2".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(listed.format_progress().contains("p1"));
    assert!(listed.format_progress().contains("p2"));

    let started = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(started.format_progress().contains("42"));

    let in_progress = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::InProgress {
            progress_percent: 75,
            at_sequence: 999,
        },
        events_applied: 750,
        duration_ms: 100,
    };
    assert!(in_progress.format_progress().contains("75%"));
    assert!(in_progress.format_progress().contains("999"));

    let failed = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Failed {
            reason: "OOM".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(failed.format_progress().contains("OOM"));

    let noop = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(noop.format_progress().contains("already up to date"));
}

// ============================================================
// Config file loading edge cases
// ============================================================

#[test]
fn config_missing_engine_section() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine"));
}

#[test]
fn config_missing_storage_section() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-storage"));
}

#[test]
fn config_empty_engine_url() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine-url"));
}

#[test]
fn config_empty_storage_path() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \"\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-storage-path"));
}

#[test]
fn config_missing_url_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nport = 3000\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine-url"));
}

#[test]
fn config_missing_path_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\nmode = \"local\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-storage-path"));
}

#[test]
fn config_readonly_permissions() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let config_path = dir.path().join(CONFIG_FILE_NAME);
    let mut perms = fs::metadata(&config_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&config_path, perms).unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-perms"));
}

// ============================================================
// History module
// ============================================================

#[test]
fn history_load_nonexistent_returns_new() {
    let path = PathBuf::from("/tmp/vo-test-noexist-history.json");
    let _ = fs::remove_file(&path);
    let history = load_history(&path).unwrap();
    assert!(history.entries().is_empty());
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn history_save_and_reload_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");

    let mut history = vo_types::CommandHistory::new();
    let snapshot = vo_types::WorkflowSnapshot::new(
        "wf-test".into(),
        vec![vo_types::DagNode {
            node_name: vo_types::NodeName::parse("node-1").unwrap(),
            retry_policy: vo_types::RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        }],
        vec![],
    );
    history
        .save_undo_point(vo_types::CommandKind::NodeCreate, snapshot)
        .unwrap();

    save_history(&path, &history).unwrap();
    let loaded = load_history(&path).unwrap();
    assert_eq!(loaded.entries().len(), 1);
}

#[test]
fn history_output_format() {
    let history = vo_types::CommandHistory::new();
    let output = get_history(&history);
    assert!(!output.can_undo);
    assert!(!output.can_redo);
    assert_eq!(output.undo_stack_depth, 0);
    assert_eq!(output.redo_stack_depth, 0);
    assert!(output.entries.is_empty());
}

#[test]
fn history_undo_empty() {
    let mut history = vo_types::CommandHistory::new();
    let result = undo_command(&mut history);
    assert!(!result.success);
    assert_eq!(result.message, "Nothing to undo");
}

#[test]
fn history_redo_empty() {
    let mut history = vo_types::CommandHistory::new();
    let result = redo_command(&mut history);
    assert!(!result.success);
    assert_eq!(result.message, "Nothing to redo");
}

#[test]
fn history_config_default() {
    let config = HistoryConfig::default();
    assert_eq!(
        config.history_path,
        PathBuf::from(".vo/command_history.json")
    );
    assert_eq!(config.workflow_name, "default");
}

// ============================================================
// Utils
// ============================================================

#[test]
fn sha256_hex_pads_to_64_chars() {
    let result = sha256_hex("test");
    assert_eq!(result.len(), 64);
}

#[test]
fn sha256_hex_empty_input() {
    let result = sha256_hex("");
    assert_eq!(result.len(), 64);
    assert!(result.chars().all(|c| c == '0'));
}

#[test]
fn file_hash_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hash_test");
    fs::write(&path, b"hello world").unwrap();

    let h1 = file_hash(&path).unwrap();
    let h2 = file_hash(&path).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn file_hash_different_content() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a");
    let p2 = dir.path().join("b");
    fs::write(&p1, b"content a").unwrap();
    fs::write(&p2, b"content b").unwrap();

    let h1 = file_hash(&p1).unwrap();
    let h2 = file_hash(&p2).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn file_hash_nonexistent_file() {
    let result = file_hash(Path::new("/tmp/nonexistent-file-hash-test"));
    assert!(result.is_err());
}

// ============================================================
// Dispatch v2 middleware abort
// ============================================================

struct AbortMiddleware;

impl MiddlewareV2 for AbortMiddleware {
    fn name(&self) -> &'static str {
        "abort"
    }

    fn before(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = MiddlewareResult> + Send + '_>> {
        Box::pin(async {
            MiddlewareResult::Abort(CliError::Dispatch("aborted by middleware".into()))
        })
    }

    fn after(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }

    fn on_error(
        &self,
        _ctx: &dyn DispatchContext,
        _error: &CliError,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

#[tokio::test]
async fn dispatch_v2_abort_middleware_returns_error() {
    let dispatcher = CommandDispatcherV2::new().with_middleware(AbortMiddleware);
    let cli = vo_cli::Cli {
        command: Command::Check { workflow: false, path: PathBuf::from("/tmp"), },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
    match result {
        Err(CliError::Dispatch(msg)) => assert!(msg.contains("aborted by middleware")),
        _ => panic!("expected Dispatch error"),
    }
}

// ============================================================
// Registry comprehensive lookup
// ============================================================

#[test]
fn registry_lookups_all_commands() {
    let registry = HandlerRegistry::default();

    let cmds = vec![
        (
            Command::Purge {
                instance: "x".into(),
            },
            "purge",
        ),
        (
            Command::Check { workflow: false, path: PathBuf::from("/tmp"), },
            "check",
        ),
        (
            Command::Gc {
                engine_url: "http://x".into(),
                dry_run: false,
            },
            "gc",
        ),
        (
            Command::Init {
                project_dir: PathBuf::from("."),
                engine_url: "http://x".into(),
                storage_path: PathBuf::from(".vo/storage"),
            },
            "init",
        ),
        (
            Command::Lock {
                project_dir: PathBuf::from("."),
            },
            "lock",
        ),
        (
            Command::Doctor {
                project_dir: PathBuf::from("."),
            },
            "doctor",
        ),
        (
            Command::Rebuild {
                project_dir: PathBuf::from("."),
                projection_id: None,
                list_projections: false,
                force: false,
            },
            "rebuild",
        ),
    ];

    for (cmd, expected_name) in cmds {
        let cli = vo_cli::Cli { command: cmd };
        let handler = registry.get(&cli).unwrap_or_else(|| {
            panic!("handler not found for {expected_name}");
        });
        assert_eq!(handler.name(), expected_name);
    }
}

#[test]
fn registry_names_sorted() {
    let registry = HandlerRegistry::default();
    let mut names = registry.names();
    names.sort();
    assert_eq!(
        names,
        vec!["check", "compensate", "doctor", "gc", "init", "lock", "purge", "rebuild", "status"]
    );
}

// ============================================================
// Exit code mapping comprehensive
// ============================================================

#[test]
fn exit_code_for_all_clap_help_variants() {
    let kinds = [
        clap::error::ErrorKind::DisplayHelp,
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        clap::error::ErrorKind::DisplayVersion,
    ];
    for kind in kinds {
        let mut cmd = clap::Command::new("vo");
        let err = cmd.error(kind, "test");
        assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
    }
}

#[test]
fn exit_code_for_unknown_argument() {
    let result = interpret_cli_from(vec!["vo", "--unknown-flag"]);
    assert!(result.is_err());
    let err = CliError::Clap(result.unwrap_err());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn exit_code_for_invalid_numeric() {
    let err = CliError::InvalidNumeric("test".into());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn exit_code_for_each_command_error_type() {
    let errors: Vec<CliError> = vec![
        CliError::Dispatch("test".into()),
        CliError::Check(CheckError::FileNotFound {
            path: PathBuf::from("/x"),
        }),
        CliError::Gc(GcError::VersionsDirNotFound {
            path: PathBuf::from("/x"),
        }),
        CliError::Init(InitError::DirNotFound {
            path: PathBuf::from("/x"),
        }),
        CliError::Lock(LockError::NotInitialized {
            path: PathBuf::from("/x"),
        }),
        CliError::Doctor(DoctorError::NotInitialized {
            path: PathBuf::from("/x"),
        }),
        CliError::Rebuild(RebuildError::NotInitialized {
            path: PathBuf::from("/x"),
        }),
    ];
    for err in errors {
        assert_eq!(map_error_to_exit_code(&err), 1);
    }
}

// ============================================================
// Middleware V2 before/after/on_error
// ============================================================

#[tokio::test]
async fn logging_middleware_v2_on_error_captures_context() {
    let ctx = DefaultDispatchContext::new("failing-cmd");
    let mw = LoggingMiddlewareV2::new();
    let err = CliError::Dispatch("test failure".into());
    mw.on_error(&ctx, &err).await;
}

#[tokio::test]
async fn metrics_middleware_v2_on_error_captures_context() {
    let ctx = DefaultDispatchContext::new("failing-cmd");
    let mw = MetricsMiddlewareV2::new();
    let err = CliError::Dispatch("test failure".into());
    mw.on_error(&ctx, &err).await;
}

// ============================================================
// Doctor checks: deeper coverage
// ============================================================

#[test]
fn doctor_workspace_detects_readonly_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    let vo_dir = dir.path().join(".vo");
    fs::create_dir_all(&vo_dir).unwrap();

    let mut perms = fs::metadata(&vo_dir).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&vo_dir, perms).unwrap();

    let report = check_workspace(dir.path(), &vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Error));
}

#[test]
fn doctor_workspace_info_for_writable_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let report = check_workspace(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Info));
}

#[test]
fn doctor_lockstate_empty_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(dir.path().join(LOCK_FILE_NAME), "").unwrap();

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lockfile" && c.message.contains("empty")));
}

#[test]
fn doctor_lockstate_unreadable_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let lock_path = dir.path().join(LOCK_FILE_NAME);
    fs::write(&lock_path, "test hash123\n").unwrap();
    let mut perms = fs::metadata(&lock_path).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&lock_path, perms).unwrap();

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lockfile" && c.severity == Severity::Error));

    let mut perms = fs::metadata(&lock_path).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&lock_path, perms).unwrap();
}

#[test]
fn doctor_storage_wal_patterns() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");

    for pattern in &[
        "events.wal",
        "data.journal",
        "main-wal",
        "secondary-journal",
    ] {
        fs::write(storage.join(pattern), b"wal").unwrap();
    }

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    let wal_warnings: Vec<_> = report
        .warnings()
        .filter(|w| w.check == "storage-wal")
        .collect();
    assert!(wal_warnings.len() >= 3);
}

#[test]
fn doctor_storage_probe_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-rw" && c.severity == Severity::Info));

    assert!(!dir.path().join(".vo/storage/.doctor-probe").exists());
}

#[test]
fn doctor_storage_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".vo/storage")).unwrap();

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-contents" && c.message.contains("empty")));
}

// ============================================================
// Binary format display names
// ============================================================

#[test]
fn binary_format_display_names() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64BigEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64LittleEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
}

// ============================================================
// Command equality
// ============================================================

#[test]
fn command_equality() {
    let c1 = Command::Check { workflow: false, path: PathBuf::from("/tmp"), };
    let c2 = Command::Check { workflow: false, path: PathBuf::from("/tmp"), };
    let c3 = Command::Check { workflow: false, path: PathBuf::from("/other"), };
    assert_eq!(c1, c2);
    assert_ne!(c1, c3);
}

#[test]
fn command_clone() {
    let c = Command::Purge {
        instance: "test".to_string(),
    };
    let cloned = c.clone();
    assert_eq!(c, cloned);
}

// ============================================================
// Severity ordering
// ============================================================

#[test]
fn severity_ord_total() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Error > Severity::Info);
    assert!(Severity::Info <= Severity::Info);
    assert!(Severity::Warn >= Severity::Warn);
}

// ============================================================
// CheckCategory display
// ============================================================

#[test]
fn check_category_display_all() {
    assert_eq!(CheckCategory::Workspace.to_string(), "workspace");
    assert_eq!(CheckCategory::LockState.to_string(), "lock-state");
    assert_eq!(
        CheckCategory::SubprocessLiveness.to_string(),
        "subprocess-liveness"
    );
    assert_eq!(
        CheckCategory::StorageIntegrity.to_string(),
        "storage-integrity"
    );
    assert_eq!(
        CheckCategory::ConfigValidation.to_string(),
        "config-validation"
    );
}

// ============================================================
// Parse strict numeric additional edge cases
// ============================================================

#[test]
fn parse_strict_numeric_valid_numbers() {
    assert_eq!(parse_strict_numeric("0").unwrap(), 0);
    assert_eq!(parse_strict_numeric("1").unwrap(), 1);
    assert_eq!(parse_strict_numeric("999999").unwrap(), 999999);
    assert_eq!(
        parse_strict_numeric("18446744073709551615").unwrap(),
        u64::MAX
    );
}

#[test]
fn parse_strict_numeric_negative() {
    let result = parse_strict_numeric("-1");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("negative")));
}

#[test]
fn parse_strict_numeric_leading_plus() {
    let result = parse_strict_numeric("+42");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("plus")));
}

#[test]
fn parse_strict_numeric_empty() {
    let result = parse_strict_numeric("");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("empty")));
}

#[test]
fn parse_strict_numeric_letters() {
    let result = parse_strict_numeric("abc");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("invalid")));
}

#[test]
fn parse_strict_numeric_overflow() {
    let result = parse_strict_numeric("18446744073709551616");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("overflow")));
}

// ============================================================
// InitConfig defaults
// ============================================================

#[test]
fn init_config_default() {
    let config = InitConfig::default();
    assert_eq!(config.project_dir, PathBuf::from("."));
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.storage_path, PathBuf::from(".vo/storage"));
}

// ============================================================
// GcConfig defaults
// ============================================================

#[test]
fn gc_config_default() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}

// ============================================================
// CategoryReport methods
// ============================================================

#[test]
fn category_report_is_healthy_with_only_warnings() {
    let mut r = CategoryReport::new(CheckCategory::Workspace);
    r.push("test", Severity::Warn, "warning".into());
    assert!(r.is_healthy());
}

#[test]
fn category_report_warnings_iterator() {
    let mut r = CategoryReport::new(CheckCategory::Workspace);
    r.push("a", Severity::Info, "info".into());
    r.push("b", Severity::Warn, "warn1".into());
    r.push("c", Severity::Warn, "warn2".into());
    r.push("d", Severity::Error, "err".into());
    let warnings: Vec<_> = r.warnings().collect();
    assert_eq!(warnings.len(), 2);
}

// ============================================================
// DoctorReport methods
// ============================================================

#[test]
fn doctor_report_errors_and_warnings() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.push("e1", Severity::Error, "err".into());
    cat1.push("w1", Severity::Warn, "warn".into());

    let mut cat2 = CategoryReport::new(CheckCategory::LockState);
    cat2.push("e2", Severity::Error, "err2".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat1, cat2],
    };
    assert!(!report.is_healthy());
    assert_eq!(report.errors().count(), 2);
    assert_eq!(report.warnings().count(), 1);
}

#[test]
fn doctor_report_healthy_no_errors_no_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("ok", Severity::Info, "fine".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    assert!(report.is_healthy());
    assert_eq!(report.errors().count(), 0);
    assert_eq!(report.warnings().count(), 0);
}
