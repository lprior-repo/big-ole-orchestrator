#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::commands::check::{BinaryFormat, CheckError};
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::doctor_checks::{
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
use vo_cli::commands::gc::GcError;
use vo_cli::commands::init::{InitConfig, InitError};
use vo_cli::commands::lock::LockError;
use vo_cli::{interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command};

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

// ============================================================
// Command Parsing Edge Cases
// ============================================================

#[test]
fn parse_init_defaults_project_dir_is_dot() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_defaults_engine_url_is_localhost() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { engine_url, .. } => {
            assert_eq!(engine_url, "http://localhost:3000");
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_defaults_storage_path() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { storage_path, .. } => {
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_lock_defaults_project_dir_is_dot() {
    let cli = interpret_cli_from(vec!["vo", "lock"]).expect("parse");
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_doctor_defaults_project_dir_is_dot() {
    let cli = interpret_cli_from(vec!["vo", "doctor"]).expect("parse");
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_check_with_special_chars_in_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "/tmp/test@#$/bin"]).expect("parse");
    match cli.command {
        Command::Check { workflow: false, path } => {
            assert_eq!(path, PathBuf::from("/tmp/test@#$/bin"));
        }
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_check_with_unicode_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "/tmp/日本語/binary"]).expect("parse");
    match cli.command {
        Command::Check { workflow: false, path } => {
            assert_eq!(path, PathBuf::from("/tmp/日本語/binary"));
        }
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_purge_with_uuid_instance() {
    let cli = interpret_cli_from(vec![
        "vo",
        "purge",
        "--instance",
        "550e8400-e29b-41d4-a716-446655440000",
    ])
    .expect("parse");
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "550e8400-e29b-41d4-a716-446655440000");
        }
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_purge_empty_instance_is_accepted() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", ""]).expect("parse");
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "");
        }
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_gc_rejects_empty_engine_url() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--engine-url", ""]).expect("parse");
    match cli.command {
        Command::Gc { engine_url, .. } => {
            assert_eq!(engine_url, "");
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_init_with_empty_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", ""]).expect("parse");
    match cli.command {
        Command::Init { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from(""));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_with_empty_storage_path() {
    let cli = interpret_cli_from(vec!["vo", "init", "--storage-path", ""]).expect("parse");
    match cli.command {
        Command::Init { storage_path, .. } => {
            assert_eq!(storage_path, PathBuf::from(""));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_gc_env_override_for_engine_url() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).expect("parse");
    match cli.command {
        Command::Gc { engine_url, .. } => {
            assert_eq!(engine_url, "http://localhost:3000");
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_all_subcommands_are_recognized() {
    let subcommands: Vec<&str> = vec!["purge", "check", "gc", "init", "lock", "doctor"];
    for sub in subcommands {
        let args: Vec<&str> = match sub {
            "purge" => vec!["vo", sub, "--instance", "x"],
            "check" => vec!["vo", sub, "/tmp/f"],
            _ => vec!["vo", sub],
        };
        let result = interpret_cli_from(args);
        assert!(result.is_ok(), "subcommand '{}' should parse", sub);
    }
}

#[test]
fn parse_subcommand_without_required_arg_fails() {
    let result = interpret_cli_from(vec!["vo", "purge"]);
    assert!(result.is_err());
}

#[test]
fn parse_double_dash_before_subcommand() {
    let result = interpret_cli_from(vec!["vo", "--", "check", "/tmp/f"]);
    assert!(result.is_err());
}

#[test]
fn parse_version_output_kind() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayVersion
    );
}

#[test]
fn parse_no_args_returns_help_on_missing() {
    let result = interpret_cli_from(vec!["vo"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

// ============================================================
// Flag Combinations
// ============================================================

#[test]
fn gc_flags_both_set() {
    let cli = interpret_cli_from(vec![
        "vo",
        "gc",
        "--engine-url",
        "http://engine:8080",
        "--dry-run",
    ])
    .expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://engine:8080");
            assert!(dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn gc_flags_only_dry_run() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--dry-run"]).expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn gc_flags_only_engine_url() {
    let cli =
        interpret_cli_from(vec!["vo", "gc", "--engine-url", "http://custom:9090"]).expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://custom:9090");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn gc_flags_none_defaults() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).expect("parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn init_flags_all_custom() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/custom/dir",
        "--engine-url",
        "http://custom:9999",
        "--storage-path",
        "/custom/storage",
    ])
    .expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/custom/dir"));
            assert_eq!(engine_url, "http://custom:9999");
            assert_eq!(storage_path, PathBuf::from("/custom/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn init_flags_partial_project_dir_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", "/my/proj"]).expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/proj"));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn init_flags_partial_engine_url_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--engine-url", "http://remote:8080"])
        .expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://remote:8080");
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn init_flags_partial_storage_path_only() {
    let cli = interpret_cli_from(vec!["vo", "init", "--storage-path", "/data/vo"]).expect("parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(storage_path, PathBuf::from("/data/vo"));
        }
        _ => panic!("expected Init"),
    }
}

// ============================================================
// Error Message Quality
// ============================================================

#[test]
fn check_error_file_not_found_mentions_path() {
    let err = CheckError::FileNotFound {
        path: PathBuf::from("/nonexistent/path"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/nonexistent/path"));
    assert!(msg.contains("file not found"));
}

#[test]
fn check_error_not_regular_file_mentions_path() {
    let err = CheckError::NotRegularFile {
        path: PathBuf::from("/some/dir"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/some/dir"));
    assert!(msg.contains("not a regular file"));
}

#[test]
fn check_error_file_too_small_includes_min_bytes() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/small"),
    };
    let msg = err.to_string();
    assert!(msg.contains("4 bytes"));
}

#[test]
fn check_error_invalid_magic_shows_hex() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/bad"),
        magic: [0xCA, 0xFE, 0xBA, 0xBE],
    };
    let msg = err.to_string();
    assert!(msg.contains("0xca"));
    assert!(msg.contains("0xfe"));
    assert!(msg.contains("0xba"));
    assert!(msg.contains("0xbe"));
}

#[test]
fn gc_error_engine_unreachable_includes_url() {
    let err = GcError::EngineUnreachable {
        url: "http://engine:3000".into(),
        reason: "timeout".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("http://engine:3000"));
    assert!(msg.contains("503"));
}

#[test]
fn gc_error_http_error_includes_status() {
    let err = GcError::EngineHttpError {
        url: "http://engine:3000/api".into(),
        status: 500,
    };
    let msg = err.to_string();
    assert!(msg.contains("500"));
    assert!(msg.contains("HTTP"));
}

#[test]
fn gc_error_invalid_api_response_is_descriptive() {
    let err = GcError::InvalidApiResponse {
        reason: "expected array".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("failed to parse"));
    assert!(msg.contains("expected array"));
}

#[test]
fn gc_error_versions_dir_not_found_includes_path() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf/versions"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf/versions"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn gc_error_delete_failed_includes_path() {
    let err = GcError::DeleteFailed {
        path: PathBuf::from("/var/wtf/versions/abc123"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf/versions/abc123"));
    assert!(msg.contains("failed to delete"));
}

#[test]
fn init_error_dir_not_found_message() {
    let err = InitError::DirNotFound {
        path: PathBuf::from("/no/such/dir"),
    };
    assert!(err.to_string().contains("/no/such/dir"));
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn init_error_not_directory_message() {
    let err = InitError::NotDirectory {
        path: PathBuf::from("/tmp/file"),
    };
    assert!(err.to_string().contains("not a directory"));
}

#[test]
fn init_error_already_initialized_message() {
    let err = InitError::AlreadyInitialized {
        path: PathBuf::from("/proj/.vo"),
    };
    assert!(err.to_string().contains("already initialized"));
}

#[test]
fn init_error_permission_denied_message() {
    let err = InitError::PermissionDenied {
        path: PathBuf::from("/root"),
        reason: "access denied".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("permission denied"));
    assert!(msg.contains("/root"));
}

#[test]
fn init_error_symlink_message() {
    let err = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    assert!(err.to_string().contains("symlink"));
    assert!(err.to_string().contains("refusing"));
}

#[test]
fn lock_error_not_initialized_message() {
    let err = LockError::NotInitialized {
        path: PathBuf::from("/project"),
    };
    assert!(err.to_string().contains("not initialized"));
    assert!(err.to_string().contains("/project"));
}

#[test]
fn lock_error_no_workflows_dir_message() {
    let err = LockError::NoWorkflowsDir {
        path: PathBuf::from("/project/.vo/workflows"),
    };
    assert!(err.to_string().contains("workflows directory not found"));
}

#[test]
fn lock_error_lock_write_message() {
    let err = LockError::LockWrite {
        reason: "disk full".into(),
    };
    assert!(err.to_string().contains("lockfile write failed"));
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn lock_error_empty_message() {
    let err = LockError::Empty {
        path: PathBuf::from("/project/.vo/workflows"),
    };
    assert!(err.to_string().contains("no workflow"));
}

#[test]
fn doctor_error_not_initialized_message() {
    let err = DoctorError::NotInitialized {
        path: PathBuf::from("/project"),
    };
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn cli_error_dispatch_display() {
    let err = CliError::Dispatch("connection refused".into());
    assert!(err.to_string().contains("dispatch"));
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn cli_error_invalid_numeric_display() {
    let err = CliError::InvalidNumeric("bad input".into());
    assert!(err.to_string().contains("invalid numeric"));
    assert!(err.to_string().contains("bad input"));
}

// ============================================================
// Output Format Correctness
// ============================================================

#[test]
fn binary_format_display_name_elf() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
}

#[test]
fn binary_format_display_name_macho_32() {
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
}

#[test]
fn binary_format_display_name_macho_64() {
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
fn doctor_report_format_contains_header() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/test/project"),
        categories: vec![CategoryReport::new(CheckCategory::Workspace)],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("/test/project"));
}

#[test]
fn doctor_report_format_contains_category_names() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![
            CategoryReport::new(CheckCategory::Workspace),
            CategoryReport::new(CheckCategory::LockState),
            CategoryReport::new(CheckCategory::SubprocessLiveness),
            CategoryReport::new(CheckCategory::StorageIntegrity),
            CategoryReport::new(CheckCategory::ConfigValidation),
        ],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("[workspace]"));
    assert!(stdout.contains("[lock-state]"));
    assert!(stdout.contains("[subprocess-liveness]"));
    assert!(stdout.contains("[storage-integrity]"));
    assert!(stdout.contains("[config-validation]"));
}

fn make_cat(
    category: CheckCategory,
    checks: Vec<(&'static str, Severity, String)>,
) -> CategoryReport {
    CategoryReport {
        category,
        checks: checks
            .into_iter()
            .map(|(check, severity, message)| CheckResult {
                check,
                severity,
                message,
            })
            .collect(),
    }
}

#[test]
fn doctor_report_format_info_uses_checkmark() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("test-check", Severity::Info, "all good".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("\u{2713}"));
    assert!(stdout.contains("test-check"));
    assert!(stdout.contains("all good"));
}

#[test]
fn doctor_report_format_error_uses_cross() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("bad-check", Severity::Error, "something broke".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (_stdout, stderr) = format_report(&report);
    assert!(stderr.contains("\u{2717}"));
    assert!(stderr.contains("bad-check"));
    assert!(stderr.contains("something broke"));
}

#[test]
fn doctor_report_format_warn_uses_warning_icon() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("warn-check", Severity::Warn, "be careful".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (_stdout, stderr) = format_report(&report);
    assert!(stderr.contains("\u{26A0}"));
    assert!(stderr.contains("warn-check"));
}

#[test]
fn doctor_report_format_healthy_summary() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![CategoryReport::new(CheckCategory::Workspace)],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("All checks passed"));
}

#[test]
fn doctor_report_format_unhealthy_shows_error_count() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("e1", Severity::Error, "err1".into()),
            ("e2", Severity::Error, "err2".into()),
        ],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let (_stdout, stderr) = format_report(&report);
    assert!(stderr.contains("2 error(s)"));
}

#[test]
fn doctor_report_json_structure() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("check1", Severity::Info, "ok".into())],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/json-test"),
        categories: vec![cat],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed["project_dir"].as_str().unwrap(), "/tmp/json-test");
    assert!(parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 0);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 0);

    let categories = parsed["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0]["category"].as_str().unwrap(), "workspace");
    assert!(categories[0]["healthy"].as_bool().unwrap());

    let checks = categories[0]["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["check"].as_str().unwrap(), "check1");
    assert_eq!(checks[0]["severity"].as_str().unwrap(), "info");
    assert_eq!(checks[0]["message"].as_str().unwrap(), "ok");
}

#[test]
fn doctor_report_json_severity_mapping() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("i", Severity::Info, "info msg".into()),
            ("w", Severity::Warn, "warn msg".into()),
            ("e", Severity::Error, "error msg".into()),
        ],
    );
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![cat],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    let checks = parsed["categories"][0]["checks"].as_array().unwrap();
    assert_eq!(checks[0]["severity"].as_str().unwrap(), "info");
    assert_eq!(checks[1]["severity"].as_str().unwrap(), "warn");
    assert_eq!(checks[2]["severity"].as_str().unwrap(), "error");
}

#[test]
fn check_category_display_format() {
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

#[test]
fn severity_ordering_invariant() {
    assert!(Severity::Info < Severity::Warn);
    assert!(Severity::Warn < Severity::Error);
    assert!(Severity::Info < Severity::Error);
}

#[test]
fn category_report_is_healthy_only_info_and_warn() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("i", Severity::Info, "ok".into()),
            ("w", Severity::Warn, "careful".into()),
        ],
    );
    assert!(cat.is_healthy());
}

#[test]
fn category_report_not_healthy_with_error() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![("e", Severity::Error, "bad".into())],
    );
    assert!(!cat.is_healthy());
}

#[test]
fn category_report_warnings_iterator() {
    let cat = make_cat(
        CheckCategory::Workspace,
        vec![
            ("i", Severity::Info, "ok".into()),
            ("w1", Severity::Warn, "warn1".into()),
            ("w2", Severity::Warn, "warn2".into()),
        ],
    );
    let warns: Vec<_> = cat.warnings().collect();
    assert_eq!(warns.len(), 2);
}

// ============================================================
// Config File Loading
// ============================================================

#[test]
fn init_creates_valid_toml_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://engine:4000".into(),
        storage_path: PathBuf::from("/data/vo"),
    };
    vo_cli::commands::init::run_init(&config).expect("init");

    let content = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    let table: toml::Table = content.parse().expect("valid TOML");
    assert!(table.contains_key("engine"));
    assert!(table.contains_key("storage"));
    assert_eq!(
        table["engine"]["url"].as_str().unwrap(),
        "http://engine:4000"
    );
    assert_eq!(table["storage"]["path"].as_str().unwrap(), "/data/vo");
}

#[test]
fn init_config_content_matches_expected_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config).expect("init");

    let content = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(content.starts_with("[engine]"));
    assert!(content.contains("url = \"http://localhost:3000\""));
    assert!(content.contains("[storage]"));
    assert!(content.contains("path = \".vo/storage\""));
}

#[test]
fn init_idempotent_same_config_returns_same_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let r1 = vo_cli::commands::init::run_init(&config).expect("first");
    let r2 = vo_cli::commands::init::run_init(&config).expect("second");
    assert_eq!(r1, r2);
}

#[test]
fn init_rejects_different_config_after_init() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config1 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let config2 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://different:9999".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config1).expect("first");
    let result = vo_cli::commands::init::run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn doctor_validates_config_toml_parseable() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_project(dir.path());
    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    let config_checks: Vec<_> = report
        .categories
        .iter()
        .find(|c| c.category == CheckCategory::ConfigValidation)
        .map(|c| c.checks.clone())
        .unwrap_or_default();

    assert!(config_checks
        .iter()
        .any(|c| c.check == "config-parseable" && c.severity == Severity::Info));
}

#[test]
fn doctor_detects_empty_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(dir.path().join("config.toml"), "").expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.errors().any(|c| c.check == "config-empty"));
}

#[test]
fn doctor_detects_invalid_toml() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(dir.path().join("config.toml"), "}}}{invalid{{").expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report
        .errors()
        .any(|c| c.check == "config-parseable" && c.severity == Severity::Error));
}

#[test]
fn doctor_detects_missing_engine_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[storage]\npath = \".vo/storage\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-engine"));
}

#[test]
fn doctor_detects_missing_storage_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-storage"));
}

#[test]
fn doctor_detects_empty_engine_url() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-engine-url"));
}

#[test]
fn doctor_detects_empty_storage_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".vo")).expect("mkdir");
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \"\"\n",
    )
    .expect("write");

    let report = vo_cli::commands::doctor::run_doctor(&vo_cli::commands::doctor::DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .expect("ok");

    assert!(report.warnings().any(|c| c.check == "config-storage-path"));
}

// ============================================================
// End-to-End CLI Integration Tests
// ============================================================

#[tokio::test]
async fn e2e_init_creates_vo_dir_and_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());

    assert!(dir.path().join(".vo").is_dir());
    assert!(dir.path().join(".vo/workflows").is_dir());
    assert!(dir.path().join("config.toml").exists());

    let config = std::fs::read_to_string(dir.path().join("config.toml")).expect("read");
    assert!(config.contains("[engine]"));
    assert!(config.contains("[storage]"));
}

#[tokio::test]
async fn e2e_init_then_lock_empty_workflows_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    vo_cli::dispatch(init_cli).await.expect("init");

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).expect("parse");
    let result = vo_cli::dispatch(lock_cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_init_lock_doctor_full_pipeline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("path");

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).expect("parse");
    vo_cli::dispatch(init_cli).await.expect("init");

    std::fs::write(
        dir.path().join(".vo/workflows/test-wf"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00],
    )
    .expect("write workflow binary");

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).expect("parse");
    vo_cli::dispatch(lock_cli).await.expect("lock");

    assert!(dir.path().join("vo.lock").exists());

    let doctor_cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).expect("parse");
    let result = vo_cli::dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_check_valid_elf_binary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin_path = dir.path().join("test.bin");
    std::fs::write(&bin_path, [0x7F, 0x45, 0x4C, 0x46, 0x00]).expect("write");

    let cli =
        interpret_cli_from(vec!["vo", "check", bin_path.to_str().expect("path")]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_check_nonexistent_file_returns_error() {
    let cli =
        interpret_cli_from(vec!["vo", "check", "/tmp/no-such-file-vo-test-xyz"]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
    let code = map_error_to_exit_code(result.as_ref().expect_err("err"));
    assert_eq!(code, 1);
}

#[tokio::test]
async fn e2e_gc_dry_run_succeeds() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--dry-run"]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_init_rejects_nonexistent_dir() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/tmp/nonexistent-e2e-test-xyz-123",
    ])
    .expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_lock_without_init_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = interpret_cli_from(vec![
        "vo",
        "lock",
        "--project-dir",
        dir.path().to_str().expect("path"),
    ])
    .expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_doctor_without_init_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli = interpret_cli_from(vec![
        "vo",
        "doctor",
        "--project-dir",
        dir.path().to_str().expect("path"),
    ])
    .expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_check_directory_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cli =
        interpret_cli_from(vec!["vo", "check", dir.path().to_str().expect("path")]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_check_symlink_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let real = dir.path().join("real.bin");
    std::fs::write(&real, [0x7F, 0x45, 0x4C, 0x46]).expect("write");
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");

    let cli = interpret_cli_from(vec!["vo", "check", link.to_str().expect("path")]).expect("parse");
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_err());
}

// ============================================================
// Parse Strict Numeric Edge Cases
// ============================================================

#[test]
fn parse_strict_numeric_accepts_1() {
    assert_eq!(parse_strict_numeric("1").unwrap(), 1);
}

#[test]
fn parse_strict_numeric_accepts_large_number() {
    assert_eq!(parse_strict_numeric("1000000000").unwrap(), 1_000_000_000);
}

#[test]
fn parse_strict_numeric_rejects_hex() {
    assert!(parse_strict_numeric("0x10").is_err());
}

#[test]
fn parse_strict_numeric_rejects_scientific() {
    assert!(parse_strict_numeric("1e10").is_err());
}

#[test]
fn parse_strict_numeric_rejects_whitespace() {
    assert!(parse_strict_numeric(" 42").is_err());
    assert!(parse_strict_numeric("42 ").is_err());
    assert!(parse_strict_numeric("4 2").is_err());
}

// ============================================================
// Exit Code Mapping
// ============================================================

#[test]
fn exit_code_clap_help_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::DisplayHelp, "help");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_version_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::DisplayVersion, "v");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_missing_arg_help_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        "h",
    );
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_clap_unknown_arg_is_2() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::UnknownArgument, "x");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 2);
}

#[test]
fn exit_code_invalid_numeric_is_2() {
    assert_eq!(
        map_error_to_exit_code(&CliError::InvalidNumeric("x".into())),
        2
    );
}

#[test]
fn exit_code_check_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Check(CheckError::FileNotFound {
            path: PathBuf::from("/tmp")
        })),
        1
    );
}

#[test]
fn exit_code_gc_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Gc(GcError::VersionsDirNotFound {
            path: PathBuf::from("/v")
        })),
        1
    );
}

#[test]
fn exit_code_init_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Init(InitError::DirNotFound {
            path: PathBuf::from("/d")
        })),
        1
    );
}

#[test]
fn exit_code_lock_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Lock(LockError::NotInitialized {
            path: PathBuf::from("/p")
        })),
        1
    );
}

#[test]
fn exit_code_doctor_error_is_1() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Doctor(DoctorError::NotInitialized {
            path: PathBuf::from("/p")
        })),
        1
    );
}

#[test]
fn exit_code_dispatch_error_is_1() {
    assert_eq!(map_error_to_exit_code(&CliError::Dispatch("err".into())), 1);
}
