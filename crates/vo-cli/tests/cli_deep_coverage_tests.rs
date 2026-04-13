#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::commands::rebuild::{RebuildConfig, RebuildError, RebuildReport, RebuildStatus};
use vo_cli::{
    format_report, format_report_json, interpret_cli_from, map_error_to_exit_code,
    parse_strict_numeric, BinaryFormat, CategoryReport, CheckCategory, CheckError, CheckResult,
    CliError, Command, DoctorConfig, DoctorError, DoctorReport, GcConfig, GcError, GcSummary,
    InitConfig, InitError, LockError, Severity, CONFIG_FILE_NAME, LOCK_FILE_NAME, VO_DIR_NAME,
    WORKFLOWS_DIR_NAME,
};

fn setup_project(dir: &std::path::Path) {
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    std::fs::create_dir_all(vo_dir.join("storage")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

fn make_temp_dir() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().to_path_buf();
    std::mem::forget(dir);
    p
}

// ============================================================
// COMMAND PARSING EDGE CASES
// ============================================================

#[test]
fn parse_version_flag_returns_display_version_error() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

#[test]
fn parse_no_args_returns_help_error() {
    let result = interpret_cli_from(vec!["vo"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn parse_invalid_subcommand_returns_error() {
    let result = interpret_cli_from(vec!["vo", "nonexistent"]);
    assert!(result.is_err());
}

#[test]
fn parse_purge_without_instance_returns_error() {
    let result = interpret_cli_from(vec!["vo", "purge"]);
    assert!(result.is_err());
}

#[test]
fn parse_purge_with_empty_instance_succeeds() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", ""]).expect("parse");
    match &cli.command {
        Command::Purge { instance } => assert_eq!(instance, ""),
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_purge_with_special_chars_instance() {
    let cli =
        interpret_cli_from(vec!["vo", "purge", "--instance", "inst-123_abc.v2"]).expect("parse");
    match &cli.command {
        Command::Purge { instance } => assert_eq!(instance, "inst-123_abc.v2"),
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_init_with_all_custom_args() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/custom/project",
        "--engine-url",
        "http://engine:4000",
        "--storage-path",
        "/custom/storage",
    ])
    .expect("parse");
    match &cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, &PathBuf::from("/custom/project"));
            assert_eq!(engine_url, "http://engine:4000");
            assert_eq!(storage_path, &PathBuf::from("/custom/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_with_partial_custom_args() {
    let cli = interpret_cli_from(vec!["vo", "init", "--engine-url", "http://custom:5000"])
        .expect("parse");
    match &cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, &PathBuf::from("."));
            assert_eq!(engine_url, "http://custom:5000");
            assert_eq!(storage_path, &PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_lock_with_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "lock", "--project-dir", "/my/project"]).expect("parse");
    match &cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, &PathBuf::from("/my/project"));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_doctor_with_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", "/diag/path"]).expect("parse");
    match &cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, &PathBuf::from("/diag/path"));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).expect("parse");
    match &cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, &PathBuf::from("."));
            assert!(projection_id.is_none());
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_projection_id() {
    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "my-proj-42"]).expect("parse");
    match &cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id.as_deref(), Some("my-proj-42"));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_list_only() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).expect("parse");
    match &cli.command {
        Command::Rebuild {
            list_projections,
            projection_id,
            ..
        } => {
            assert!(*list_projections);
            assert!(projection_id.is_none());
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_force() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--force"]).expect("parse");
    match &cli.command {
        Command::Rebuild { force, .. } => assert!(*force),
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_all_flags_together() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        "/proj",
        "--projection-id",
        "proj-1",
        "--list",
        "--force",
    ])
    .expect("parse");
    match &cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, &PathBuf::from("/proj"));
            assert_eq!(projection_id.as_deref(), Some("proj-1"));
            assert!(*list_projections);
            assert!(*force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_gc_with_both_flags() {
    let cli = interpret_cli_from(vec![
        "vo",
        "gc",
        "--engine-url",
        "http://custom:9999",
        "--dry-run",
    ])
    .expect("parse");
    match &cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://custom:9999");
            assert!(*dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_check_with_relative_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "../bin/workflow"]).expect("parse");
    match &cli.command {
        Command::Check { path } => {
            assert_eq!(path, &PathBuf::from("../bin/workflow"));
        }
        _ => panic!("expected Check"),
    }
}

// ============================================================
// ERROR MESSAGE QUALITY
// ============================================================

#[test]
fn check_error_file_not_found_display() {
    let err = CheckError::FileNotFound {
        path: PathBuf::from("/missing/file"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/missing/file"));
    assert!(msg.contains("not found"));
}

#[test]
fn check_error_not_regular_file_display() {
    let err = CheckError::NotRegularFile {
        path: PathBuf::from("/dev/null"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/dev/null"));
    assert!(msg.contains("not a regular file"));
}

#[test]
fn check_error_file_too_small_display() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tiny"));
    assert!(msg.contains("too small"));
    assert!(msg.contains("4 bytes"));
}

#[test]
fn check_error_invalid_magic_display() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/bad/bin"),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = err.to_string();
    assert!(msg.contains("/bad/bin"));
    assert!(msg.contains("0xde"));
    assert!(msg.contains("0xbe"));
}

#[test]
fn check_error_permission_denied_display() {
    let err = CheckError::PermissionDenied {
        path: PathBuf::from("/secret/bin"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/secret/bin"));
    assert!(msg.contains("permission"));
}

#[test]
fn init_error_dir_not_found_display() {
    let err = InitError::DirNotFound {
        path: PathBuf::from("/no/such/dir"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/no/such/dir"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn init_error_not_directory_display() {
    let err = InitError::NotDirectory {
        path: PathBuf::from("/tmp/file"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/file"));
    assert!(msg.contains("not a directory"));
}

#[test]
fn init_error_already_initialized_display() {
    let err = InitError::AlreadyInitialized {
        path: PathBuf::from("/proj/.vo"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj/.vo"));
    assert!(msg.contains("already initialized"));
}

#[test]
fn init_error_permission_denied_display() {
    let err = InitError::PermissionDenied {
        path: PathBuf::from("/root"),
        reason: "read-only filesystem".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/root"));
    assert!(msg.contains("permission denied"));
    assert!(msg.contains("read-only"));
}

#[test]
fn init_error_symlink_target_display() {
    let err = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/link"));
    assert!(msg.contains("symlink"));
    assert!(msg.contains("refusing"));
}

#[test]
fn lock_error_not_initialized_display() {
    let err = LockError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("not initialized"));
}

#[test]
fn lock_error_no_workflows_dir_display() {
    let err = LockError::NoWorkflowsDir {
        path: PathBuf::from("/proj/.vo/workflows"),
    };
    let msg = err.to_string();
    assert!(msg.contains("workflows"));
    assert!(msg.contains("not found"));
}

#[test]
fn lock_error_empty_display() {
    let err = LockError::Empty {
        path: PathBuf::from("/proj/.vo/workflows"),
    };
    let msg = err.to_string();
    assert!(msg.contains("no workflow"));
}

#[test]
fn lock_error_lock_write_display() {
    let err = LockError::LockWrite {
        reason: "disk full".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("disk full"));
    assert!(msg.contains("lockfile"));
}

#[test]
fn doctor_error_not_initialized_display() {
    let err = DoctorError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("not initialized"));
}

#[test]
fn gc_error_engine_unreachable_display() {
    let err = GcError::EngineUnreachable {
        url: "http://engine:3000".to_string(),
        reason: "connection refused".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("http://engine:3000"));
    assert!(msg.contains("connection refused"));
    assert!(msg.contains("503"));
}

#[test]
fn gc_error_engine_http_error_display() {
    let err = GcError::EngineHttpError {
        url: "http://engine:3000/api".to_string(),
        status: 500,
    };
    let msg = err.to_string();
    assert!(msg.contains("500"));
    assert!(msg.contains("http://engine:3000"));
}

#[test]
fn gc_error_invalid_api_response_display() {
    let err = GcError::InvalidApiResponse {
        reason: "missing field".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("missing field"));
    assert!(msg.contains("parse"));
}

#[test]
fn gc_error_versions_dir_not_found_display() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn rebuild_error_not_initialized_display() {
    let err = RebuildError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("not initialized"));
}

#[test]
fn rebuild_error_projection_not_found_display() {
    let err = RebuildError::ProjectionNotFound("my-proj".to_string());
    let msg = err.to_string();
    assert!(msg.contains("my-proj"));
    assert!(msg.contains("not found"));
}

#[test]
fn rebuild_error_rebuild_failed_display() {
    let err = RebuildError::RebuildFailed("timeout".to_string());
    let msg = err.to_string();
    assert!(msg.contains("timeout"));
    assert!(msg.contains("failed"));
}

#[test]
fn rebuild_error_unsupported_schema_display() {
    let err = RebuildError::UnsupportedSchemaVersion(99);
    let msg = err.to_string();
    assert!(msg.contains("99"));
    assert!(msg.contains("not supported"));
}

#[test]
fn rebuild_error_rebuild_in_progress_display() {
    let err = RebuildError::RebuildInProgress("proj-1".to_string());
    let msg = err.to_string();
    assert!(msg.contains("proj-1"));
    assert!(msg.contains("already in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_display() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "key-abc".to_string(),
        actual: "key-xyz".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("key-abc"));
    assert!(msg.contains("key-xyz"));
    assert!(msg.contains("mismatch"));
}

#[test]
fn rebuild_error_engine_display() {
    let err = RebuildError::Engine("internal failure".to_string());
    let msg = err.to_string();
    assert!(msg.contains("internal failure"));
    assert!(msg.contains("engine"));
}

#[test]
fn cli_error_dispatch_display() {
    let err = CliError::Dispatch("unknown command".to_string());
    let msg = err.to_string();
    assert!(msg.contains("dispatch"));
    assert!(msg.contains("unknown command"));
}

#[test]
fn cli_error_invalid_numeric_display() {
    let err = CliError::InvalidNumeric("bad input".to_string());
    let msg = err.to_string();
    assert!(msg.contains("invalid numeric"));
    assert!(msg.contains("bad input"));
}

#[test]
fn parse_strict_negative_rejected() {
    let err = parse_strict_numeric("-5").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("negative"));
}

#[test]
fn parse_strict_empty_rejected() {
    let err = parse_strict_numeric("").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("empty"));
}

#[test]
fn parse_strict_plus_rejected() {
    let err = parse_strict_numeric("+42").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("plus"));
}

#[test]
fn parse_strict_non_digits_rejected() {
    let err = parse_strict_numeric("abc").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid"));
}

#[test]
fn parse_strict_max_u64_accepted() {
    let result = parse_strict_numeric("18446744073709551615");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), u64::MAX);
}

#[test]
fn parse_strict_overflow_rejected() {
    let err = parse_strict_numeric("18446744073709551616").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("overflow"));
}

// ============================================================
// EXIT CODE MAPPING
// ============================================================

#[test]
fn exit_code_clap_unknown_error_is_2() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::InvalidValue, "bad");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 2);
}

#[test]
fn exit_code_display_help_on_missing_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        "help",
    );
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_init_error_is_1() {
    let err = CliError::Init(InitError::DirNotFound {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_lock_error_is_1() {
    let err = CliError::Lock(LockError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_doctor_error_is_1() {
    let err = CliError::Doctor(DoctorError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_rebuild_error_is_1() {
    let err = CliError::Rebuild(RebuildError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_dispatch_error_is_1() {
    let err = CliError::Dispatch("boom".to_string());
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_invalid_numeric_is_2() {
    let err = CliError::InvalidNumeric("x".to_string());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

// ============================================================
// OUTPUT FORMAT CORRECTNESS
// ============================================================

#[test]
fn format_report_healthy_project_output() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/healthy"),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![CheckResult {
                check: "vo-dir",
                severity: Severity::Info,
                message: ".vo/ exists".into(),
            }],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("/healthy"));
    assert!(stdout.contains("workspace"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_errors_go_to_stderr() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/broken"),
        categories: vec![CategoryReport {
            category: CheckCategory::ConfigValidation,
            checks: vec![CheckResult {
                check: "config-exists",
                severity: Severity::Error,
                message: "config.toml missing".into(),
            }],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("config-exists"));
    assert!(stderr.contains("error(s)"));
}

#[test]
fn format_report_warnings_go_to_stderr() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/warned"),
        categories: vec![CategoryReport {
            category: CheckCategory::StorageIntegrity,
            checks: vec![CheckResult {
                check: "storage-dir",
                severity: Severity::Warn,
                message: "storage directory missing".into(),
            }],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("storage-dir"));
    assert!(stderr.contains("warning(s)"));
}

#[test]
fn format_report_mixed_severity() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/mixed"),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![
                CheckResult {
                    check: "ok1",
                    severity: Severity::Info,
                    message: "all good".into(),
                },
                CheckResult {
                    check: "warn1",
                    severity: Severity::Warn,
                    message: "watch out".into(),
                },
                CheckResult {
                    check: "err1",
                    severity: Severity::Error,
                    message: "broken".into(),
                },
            ],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("ok1"));
    assert!(stderr.contains("warn1"));
    assert!(stderr.contains("err1"));
    assert!(stderr.contains("1 error(s)"));
    assert!(stderr.contains("1 warning(s)"));
}

#[test]
fn format_report_empty_categories() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/empty"),
        categories: vec![CategoryReport {
            category: CheckCategory::SubprocessLiveness,
            checks: vec![],
        }],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("no checks"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_json_structure() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/json-test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![CheckResult {
                    check: "test-check",
                    severity: Severity::Info,
                    message: "all good".into(),
                }],
            },
            CategoryReport {
                category: CheckCategory::LockState,
                checks: vec![CheckResult {
                    check: "lock-check",
                    severity: Severity::Error,
                    message: "hash mismatch".into(),
                }],
            },
        ],
    };
    let json_str = format_report_json(&report);
    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(json["project_dir"].as_str(), Some("/json-test"));
    assert_eq!(json["healthy"].as_bool(), Some(false));
    assert_eq!(json["error_count"].as_u64(), Some(1));
    assert_eq!(json["warn_count"].as_u64(), Some(0));
    let cats = json["categories"].as_array().unwrap();
    assert_eq!(cats.len(), 2);
    assert_eq!(cats[0]["category"].as_str(), Some("workspace"));
    assert_eq!(cats[0]["healthy"].as_bool(), Some(true));
    assert_eq!(cats[1]["category"].as_str(), Some("lock-state"));
    assert_eq!(cats[1]["healthy"].as_bool(), Some(false));
}

#[test]
fn format_report_json_severity_serialization() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/sev-test"),
        categories: vec![CategoryReport {
            category: CheckCategory::ConfigValidation,
            checks: vec![
                CheckResult {
                    check: "i",
                    severity: Severity::Info,
                    message: "info".into(),
                },
                CheckResult {
                    check: "w",
                    severity: Severity::Warn,
                    message: "warn".into(),
                },
                CheckResult {
                    check: "e",
                    severity: Severity::Error,
                    message: "error".into(),
                },
            ],
        }],
    };
    let json_str = format_report_json(&report);
    let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let checks = json["categories"][0]["checks"].as_array().unwrap();
    assert_eq!(checks[0]["severity"].as_str(), Some("info"));
    assert_eq!(checks[1]["severity"].as_str(), Some("warn"));
    assert_eq!(checks[2]["severity"].as_str(), Some("error"));
}

#[test]
fn rebuild_format_progress_started() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Started { from_sequence: 100 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("100"));
}

#[test]
fn rebuild_format_progress_failed() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
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
fn rebuild_format_progress_noop() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
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

#[test]
fn rebuild_format_progress_listed_empty() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec![]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("Registered projections"));
}

#[test]
fn rebuild_format_progress_listed_with_items() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["proj-a".into(), "proj-b".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("proj-a"));
    assert!(output.contains("proj-b"));
}

// ============================================================
// BINARY FORMAT DISPLAY NAMES
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

#[test]
fn check_category_display() {
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
// CHECK CATEGORY + SEPARATOR TRAITS
// ============================================================

#[test]
fn severity_ordering_chain() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Error > Severity::Info);
}

#[test]
fn category_report_warnings_filter() {
    let r = CategoryReport {
        category: CheckCategory::Workspace,
        checks: vec![
            CheckResult {
                check: "a",
                severity: Severity::Info,
                message: "info msg".into(),
            },
            CheckResult {
                check: "b",
                severity: Severity::Warn,
                message: "warn msg".into(),
            },
            CheckResult {
                check: "c",
                severity: Severity::Error,
                message: "err msg".into(),
            },
        ],
    };
    let warnings: Vec<_> = r.warnings().collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].check, "b");
}

#[test]
fn doctor_report_errors_iterator() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![
                    CheckResult {
                        check: "e1",
                        severity: Severity::Error,
                        message: "err".into(),
                    },
                    CheckResult {
                        check: "i1",
                        severity: Severity::Info,
                        message: "info".into(),
                    },
                ],
            },
            CategoryReport {
                category: CheckCategory::LockState,
                checks: vec![CheckResult {
                    check: "e2",
                    severity: Severity::Error,
                    message: "err2".into(),
                }],
            },
        ],
    };
    let errors: Vec<_> = report.errors().collect();
    assert_eq!(errors.len(), 2);
}

#[test]
fn doctor_report_warnings_cross_category() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/test"),
        categories: vec![
            CategoryReport {
                category: CheckCategory::Workspace,
                checks: vec![CheckResult {
                    check: "w1",
                    severity: Severity::Warn,
                    message: "w".into(),
                }],
            },
            CategoryReport {
                category: CheckCategory::StorageIntegrity,
                checks: vec![CheckResult {
                    check: "w2",
                    severity: Severity::Warn,
                    message: "w".into(),
                }],
            },
        ],
    };
    let warnings: Vec<_> = report.warnings().collect();
    assert_eq!(warnings.len(), 2);
}

// ============================================================
// CONFIG FILE LOADING EDGE CASES
// ============================================================

#[test]
fn init_creates_vo_dir_and_workflows_dir() {
    let dir = make_temp_dir();
    let config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&config).unwrap();
    assert!(dir.join(".vo").is_dir());
    assert!(dir.join(".vo/workflows").is_dir());
    assert!(dir.join("config.toml").exists());
}

#[test]
fn init_config_toml_has_correct_content() {
    let dir = make_temp_dir();
    let config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://engine:4000".into(),
        storage_path: PathBuf::from("/data/vo"),
    };
    vo_cli::run_init(&config).unwrap();
    let content = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert!(content.contains("[engine]"));
    assert!(content.contains("url = \"http://engine:4000\""));
    assert!(content.contains("[storage]"));
    assert!(content.contains("/data/vo"));
}

#[test]
fn init_idempotent_with_same_config() {
    let dir = make_temp_dir();
    let config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&config).unwrap();
    let result = vo_cli::run_init(&config);
    assert!(result.is_ok());
}

#[test]
fn init_fails_on_already_initialized_with_different_config() {
    let dir = make_temp_dir();
    let config1 = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&config1).unwrap();
    let config2 = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://different:9999".into(),
        storage_path: PathBuf::from("/other/path"),
    };
    let result = vo_cli::run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn init_fails_on_nonexistent_dir() {
    let config = InitConfig {
        project_dir: PathBuf::from("/no/such/directory/ever"),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::run_init(&config);
    assert!(matches!(result, Err(InitError::DirNotFound { .. })));
}

#[test]
fn init_fails_on_file_as_project_dir() {
    let dir = make_temp_dir();
    let file_path = dir.join("afile");
    std::fs::write(&file_path, b"not a dir").unwrap();
    let config = InitConfig {
        project_dir: file_path.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::run_init(&config);
    assert!(matches!(result, Err(InitError::NotDirectory { .. })));
}

#[test]
fn init_fails_on_symlink() {
    let dir = make_temp_dir();
    let target = dir.join("target_dir");
    std::fs::create_dir_all(&target).unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let config = InitConfig {
        project_dir: link.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::run_init(&config);
    assert!(matches!(result, Err(InitError::SymlinkTarget { .. })));
}

#[test]
fn init_config_default_values() {
    let config = InitConfig::default();
    assert_eq!(config.project_dir, PathBuf::from("."));
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.storage_path, PathBuf::from(".vo/storage"));
}

// ============================================================
// CHECK COMMAND EDGE CASES
// ============================================================

#[test]
fn check_valid_elf_binary() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.elf");
    std::fs::write(&bin_path, [0x7F, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::Elf);
}

#[test]
fn check_valid_macho_64_le() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xCF, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64LittleEndian);
}

#[test]
fn check_valid_macho_64_be() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCF, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64BigEndian);
}

#[test]
fn check_valid_macho_32_le() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xCE, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO32LittleEndian);
}

#[test]
fn check_valid_macho_32_be() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO32BigEndian);
}

#[test]
fn check_file_too_small_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("tiny.bin");
    std::fs::write(&bin_path, [0x7F, 0x45]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn check_invalid_magic_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("bad.bin");
    std::fs::write(&bin_path, [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn check_nonexistent_file_returns_not_found() {
    let result =
        vo_cli::validate_binary_header(PathBuf::from("/tmp/does-not-exist-co5-test").as_path());
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn check_symlink_returns_not_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, [0x7F, 0x45, 0x4C, 0x46]).unwrap();
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = vo_cli::validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn check_directory_returns_not_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let result = vo_cli::validate_binary_header(dir.path());
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

// ============================================================
// LOCK COMMAND EDGE CASES
// ============================================================

#[test]
fn lock_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_fails_without_workflows_dir() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(matches!(result, Err(LockError::NoWorkflowsDir { .. })));
}

#[test]
fn lock_fails_with_empty_workflows() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(matches!(result, Err(LockError::Empty { .. })));
}

#[test]
fn lock_succeeds_with_workflow_binaries() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(dir.join(".vo/workflows/wf-a"), b"binary content a").unwrap();
    std::fs::write(dir.join(".vo/workflows/wf-b"), b"binary content b").unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(result.is_ok());
    let lockmap = result.unwrap();
    assert_eq!(lockmap.len(), 2);
    assert!(lockmap.contains_key("wf-a"));
    assert!(lockmap.contains_key("wf-b"));
    assert!(dir.join("vo.lock").exists());
}

#[test]
fn lock_file_format_is_name_space_hash() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(dir.join(".vo/workflows/test-wf"), b"content").unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    vo_cli::run_lock(&config).unwrap();
    let content = std::fs::read_to_string(dir.join("vo.lock")).unwrap();
    let parts: Vec<&str> = content.trim().splitn(2, ' ').collect();
    assert_eq!(parts[0], "test-wf");
    assert_eq!(parts[1].len(), 64);
}

#[test]
fn lock_ignores_subdirectories() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows/subdir")).unwrap();
    std::fs::write(dir.join(".vo/workflows/wf-1"), b"binary").unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn lock_sorts_entries_alphabetically() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(dir.join(".vo/workflows/z-wf"), b"z").unwrap();
    std::fs::write(dir.join(".vo/workflows/a-wf"), b"a").unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let lockmap = vo_cli::run_lock(&config).unwrap();
    let keys: Vec<_> = lockmap.keys().collect();
    assert_eq!(keys[0], "a-wf");
    assert_eq!(keys[1], "z-wf");
}

// ============================================================
// DOCTOR COMMAND EDGE CASES
// ============================================================

#[test]
fn doctor_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_doctor(&config);
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn doctor_succeeds_on_initialized_project() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    assert_eq!(report.project_dir, dir);
    assert_eq!(report.categories.len(), 5);
}

#[test]
fn doctor_report_has_all_five_categories() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    let cats: Vec<_> = report.categories.iter().map(|c| c.category).collect();
    assert!(cats.contains(&CheckCategory::Workspace));
    assert!(cats.contains(&CheckCategory::LockState));
    assert!(cats.contains(&CheckCategory::SubprocessLiveness));
    assert!(cats.contains(&CheckCategory::StorageIntegrity));
    assert!(cats.contains(&CheckCategory::ConfigValidation));
}

#[test]
fn doctor_healthy_project_is_healthy() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    assert!(report.is_healthy());
}

// ============================================================
// REBUILD COMMAND EDGE CASES
// ============================================================

#[test]
fn rebuild_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_returns_ok() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

#[test]
fn rebuild_without_projection_id_fails() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
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
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("proj-1".into()),
        list_projections: false,
        force: true,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert_eq!(report.projection_id.as_deref(), Some("proj-1"));
    assert!(matches!(report.status, RebuildStatus::Completed));
}

// ============================================================
// E2E INTEGRATION: INIT -> LOCK -> DOCTOR PIPELINE
// ============================================================

#[test]
fn e2e_init_lock_doctor_pipeline() {
    let dir = make_temp_dir();

    let init_config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&init_config).expect("init should succeed");
    assert!(dir.join(".vo").is_dir());
    assert!(dir.join(".vo/workflows").is_dir());
    assert!(dir.join("config.toml").exists());

    std::fs::write(
        dir.join(".vo/workflows/my-workflow"),
        b"#!/bin/bash\necho hello",
    )
    .unwrap();

    let lock_config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let lockmap = vo_cli::run_lock(&lock_config).expect("lock should succeed");
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("my-workflow"));
    assert!(dir.join("vo.lock").exists());

    let doctor_config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&doctor_config).expect("doctor should succeed");
    assert!(report.is_healthy());
}

#[test]
fn e2e_init_lock_verify_doctor_catches_tampering() {
    let dir = make_temp_dir();

    let init_config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&init_config).unwrap();
    std::fs::write(dir.join(".vo/workflows/wf"), b"original").unwrap();

    let lock_config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    vo_cli::run_lock(&lock_config).unwrap();

    std::fs::write(dir.join(".vo/workflows/wf"), b"tampered").unwrap();

    let doctor_config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&doctor_config).unwrap();
    assert!(!report.is_healthy());
    let lock_errors: Vec<_> = report
        .categories
        .iter()
        .filter(|c| c.category == CheckCategory::LockState)
        .flat_map(|c| c.checks.iter())
        .filter(|c| c.check == "lock-integrity" && c.severity == Severity::Error)
        .collect();
    assert!(!lock_errors.is_empty());
}

#[test]
fn e2e_init_then_check_binary() {
    let dir = make_temp_dir();

    let init_config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&init_config).unwrap();

    let bin_path = dir.join(".vo/workflows/valid-elf");
    std::fs::write(&bin_path, [0x7F, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();

    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::Elf);
}

#[test]
fn e2e_init_lock_then_rebuild() {
    let dir = make_temp_dir();

    let init_config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::run_init(&init_config).unwrap();

    let rebuild_config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let report = vo_cli::commands::rebuild::run_rebuild(&rebuild_config).unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

// ============================================================
// GC CONFIG DEFAULTS & SUMMARY
// ============================================================

#[test]
fn gc_config_default_values() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}

#[test]
fn gc_summary_fields() {
    let summary = GcSummary {
        pinned_count: 5,
        scanned_count: 3,
        deleted_count: 2,
        deleted_hashes: vec!["abc123".into()],
        failures: vec![(PathBuf::from("/bad"), "permission denied".into())],
    };
    assert_eq!(summary.pinned_count, 5);
    assert_eq!(summary.scanned_count, 3);
    assert_eq!(summary.deleted_count, 2);
    assert_eq!(summary.deleted_hashes.len(), 1);
    assert_eq!(summary.failures.len(), 1);
}

#[test]
fn gc_error_delete_failed_display() {
    let err = GcError::DeleteFailed {
        path: PathBuf::from("/var/wtf/versions/abc123"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf/versions/abc123"));
    assert!(msg.contains("failed to delete"));
}

// ============================================================
// CHECK ERROR PARTIAL EQ EDGE CASES
// ============================================================

#[test]
fn check_error_eq_same_file_not_found() {
    let a = CheckError::FileNotFound {
        path: PathBuf::from("/same"),
    };
    let b = CheckError::FileNotFound {
        path: PathBuf::from("/same"),
    };
    assert_eq!(a, b);
}

#[test]
fn check_error_neq_different_variants() {
    let a = CheckError::FileNotFound {
        path: PathBuf::from("/x"),
    };
    let b = CheckError::PermissionDenied {
        path: PathBuf::from("/x"),
    };
    assert_ne!(a, b);
}

#[test]
fn check_error_invalid_magic_eq() {
    let a = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [1, 2, 3, 4],
    };
    let b = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [1, 2, 3, 4],
    };
    assert_eq!(a, b);
}

#[test]
fn check_error_invalid_magic_neq_magic() {
    let a = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [1, 2, 3, 4],
    };
    let b = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [5, 6, 7, 8],
    };
    assert_ne!(a, b);
}

#[test]
fn check_error_file_too_small_eq() {
    let a = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    let b = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    assert_eq!(a, b);
}

// ============================================================
// COMMAND CLONE + EQUALITY
// ============================================================

#[test]
fn command_clone_preserves_all_fields() {
    let cmd = Command::Rebuild {
        project_dir: PathBuf::from("/proj"),
        projection_id: Some("p1".into()),
        list_projections: true,
        force: true,
    };
    let cloned = cmd.clone();
    assert_eq!(cmd, cloned);
}

#[test]
fn command_equality_across_variants() {
    let a = Command::Gc {
        engine_url: "http://x:1".into(),
        dry_run: false,
    };
    let b = Command::Gc {
        engine_url: "http://x:1".into(),
        dry_run: false,
    };
    assert_eq!(a, b);

    let c = Command::Gc {
        engine_url: "http://x:1".into(),
        dry_run: true,
    };
    assert_ne!(a, c);
}

#[test]
fn command_init_equality() {
    let a = Command::Init {
        project_dir: PathBuf::from("."),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let b = Command::Init {
        project_dir: PathBuf::from("."),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    assert_eq!(a, b);
}

// ============================================================
// CLI HELP AND VERSION FLAGS
// ============================================================

#[test]
fn help_flag_for_subcommand() {
    let result = interpret_cli_from(vec!["vo", "init", "--help"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn help_flag_for_gc_subcommand() {
    let result = interpret_cli_from(vec!["vo", "gc", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_check_subcommand() {
    let result = interpret_cli_from(vec!["vo", "check", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_lock_subcommand() {
    let result = interpret_cli_from(vec!["vo", "lock", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_doctor_subcommand() {
    let result = interpret_cli_from(vec!["vo", "doctor", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_rebuild_subcommand() {
    let result = interpret_cli_from(vec!["vo", "rebuild", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

// ============================================================
// CHECK KNOWN MAGICS CONSTANTS
// ============================================================

#[test]
fn known_magics_has_five_entries() {
    assert_eq!(vo_cli::KNOWN_MAGICS.len(), 5);
}

#[test]
fn elf_magic_is_correct() {
    assert_eq!(vo_cli::ELF_MAGIC, [0x7F, 0x45, 0x4C, 0x46]);
}

#[test]
fn macho_magic_constants() {
    assert_eq!(vo_cli::MACHO_MAGIC_32_BE, [0xFE, 0xED, 0xFA, 0xCE]);
    assert_eq!(vo_cli::MACHO_MAGIC_32_LE, [0xCE, 0xFA, 0xED, 0xFE]);
    assert_eq!(vo_cli::MACHO_MAGIC_64_BE, [0xFE, 0xED, 0xFA, 0xCF]);
    assert_eq!(vo_cli::MACHO_MAGIC_64_LE, [0xCF, 0xFA, 0xED, 0xFE]);
}

// ============================================================
// CONSTANT EXPORTS
// ============================================================

#[test]
fn exported_constants_values() {
    assert_eq!(VO_DIR_NAME, ".vo");
    assert_eq!(WORKFLOWS_DIR_NAME, "workflows");
    assert_eq!(CONFIG_FILE_NAME, "config.toml");
    assert_eq!(LOCK_FILE_NAME, "vo.lock");
}
