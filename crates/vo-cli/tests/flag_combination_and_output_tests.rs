use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use vo_cli::cli::{interpret_cli_from, map_error_to_exit_code, Cli, CliError, Command};
use vo_cli::commands::check::{
    validate_binary_header, BinaryFormat, CheckError, ELF_MAGIC, KNOWN_MAGICS, MACHO_MAGIC_32_BE,
    MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE, MACHO_MAGIC_64_LE,
};
use vo_cli::commands::doctor::{run_doctor, DoctorConfig, DoctorError};
use vo_cli::commands::doctor_checks::{
    check_config_validation, check_lock_state, check_storage_integrity, check_workspace,
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
use vo_cli::commands::gc::{GcConfig, GcError};
use vo_cli::commands::init::{
    run_init, InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME, WORKFLOWS_DIR_NAME,
};
use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};
use vo_cli::parse::parse_strict_numeric;
use vo_cli::utils::{file_hash, sha256_hex};

fn setup_project(dir: &std::path::Path) {
    let vo_dir = dir.join(".vo");
    fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

fn make_binary(dir: &std::path::Path, name: &str, magic: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut data = magic.to_vec();
    data.extend_from_slice(&[0u8; 64]);
    fs::write(&path, &data).unwrap();
    path
}

#[test]
fn gc_engine_url_from_env_var() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into()];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Gc { engine_url, .. } = &cli.command {
        assert_eq!(engine_url, "http://localhost:3000");
    } else {
        panic!("expected Gc");
    }
}

#[test]
fn gc_dry_run_and_engine_url_together() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://engine:4000".into(),
        "--dry-run".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Gc {
        engine_url,
        dry_run,
    } = &cli.command
    {
        assert_eq!(engine_url, "http://engine:4000");
        assert!(*dry_run);
    } else {
        panic!("expected Gc");
    }
}

#[test]
fn init_partial_flags_uses_defaults() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "init".into(),
        "--engine-url".into(),
        "http://custom:5000".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Init {
        project_dir,
        engine_url,
        storage_path,
    } = &cli.command
    {
        assert_eq!(*project_dir, PathBuf::from("."));
        assert_eq!(engine_url, "http://custom:5000");
        assert_eq!(*storage_path, PathBuf::from(".vo/storage"));
    } else {
        panic!("expected Init");
    }
}

#[test]
fn init_only_project_dir_flag() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "init".into(),
        "--project-dir".into(),
        "/tmp/my-project".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Init {
        project_dir,
        engine_url,
        storage_path,
    } = &cli.command
    {
        assert_eq!(*project_dir, PathBuf::from("/tmp/my-project"));
        assert_eq!(engine_url, "http://localhost:3000");
        assert_eq!(*storage_path, PathBuf::from(".vo/storage"));
    } else {
        panic!("expected Init");
    }
}

#[test]
fn rebuild_force_only_no_projection() {
    let args: Vec<OsString> = vec!["vo".into(), "rebuild".into(), "--force".into()];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Rebuild {
        force,
        projection_id,
        list_projections,
        ..
    } = &cli.command
    {
        assert!(*force);
        assert!(projection_id.is_none());
        assert!(!list_projections);
    } else {
        panic!("expected Rebuild");
    }
}

#[test]
fn rebuild_list_and_force_together() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "rebuild".into(),
        "--list".into(),
        "--force".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Rebuild {
        list_projections,
        force,
        ..
    } = &cli.command
    {
        assert!(list_projections);
        assert!(force);
    } else {
        panic!("expected Rebuild");
    }
}

#[test]
fn rebuild_with_custom_project_and_projection() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "rebuild".into(),
        "--project-dir".into(),
        "/data".into(),
        "--projection-id".into(),
        "proj-99".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Rebuild {
        project_dir,
        projection_id,
        ..
    } = &cli.command
    {
        assert_eq!(*project_dir, PathBuf::from("/data"));
        assert_eq!(projection_id.as_deref(), Some("proj-99"));
    } else {
        panic!("expected Rebuild");
    }
}

#[test]
fn check_path_with_relative_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "../bin/workflow".into()];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Check { path } = &cli.command {
        assert_eq!(*path, PathBuf::from("../bin/workflow"));
    } else {
        panic!("expected Check");
    }
}

#[test]
fn check_path_with_dot() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), ".".into()];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Check { path } = &cli.command {
        assert_eq!(*path, PathBuf::from("."));
    } else {
        panic!("expected Check");
    }
}

#[test]
fn purge_instance_with_uuid_format() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "550e8400-e29b-41d4-a716-446655440000".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Purge { instance } = &cli.command {
        assert_eq!(instance, "550e8400-e29b-41d4-a716-446655440000");
    } else {
        panic!("expected Purge");
    }
}

#[test]
fn purge_instance_with_numeric_string() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "12345".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Purge { instance } = &cli.command {
        assert_eq!(instance, "12345");
    } else {
        panic!("expected Purge");
    }
}

#[test]
fn cli_error_from_check_error() {
    let check_err = CheckError::FileNotFound {
        path: PathBuf::from("/missing"),
    };
    let cli_err = CliError::from(check_err);
    assert!(cli_err.to_string().contains("/missing"));
    assert_eq!(map_error_to_exit_code(&cli_err), 1);
}

#[test]
fn cli_error_from_gc_error() {
    let gc_err = GcError::EngineUnreachable {
        url: "http://fail".into(),
        reason: "timeout".into(),
    };
    let cli_err = CliError::from(gc_err);
    assert!(cli_err.to_string().contains("timeout"));
    assert_eq!(map_error_to_exit_code(&cli_err), 1);
}

#[test]
fn cli_error_from_init_error() {
    let init_err = InitError::DirNotFound {
        path: PathBuf::from("/nope"),
    };
    let cli_err = CliError::from(init_err);
    assert!(cli_err.to_string().contains("/nope"));
    assert_eq!(map_error_to_exit_code(&cli_err), 1);
}

#[test]
fn cli_error_from_lock_error() {
    let lock_err = LockError::NotInitialized {
        path: PathBuf::from("/x"),
    };
    let cli_err = CliError::from(lock_err);
    assert!(cli_err.to_string().contains("/x"));
    assert_eq!(map_error_to_exit_code(&cli_err), 1);
}

#[test]
fn cli_error_from_doctor_error() {
    let doc_err = DoctorError::NotInitialized {
        path: PathBuf::from("/y"),
    };
    let cli_err = CliError::from(doc_err);
    assert!(cli_err.to_string().contains("/y"));
    assert_eq!(map_error_to_exit_code(&cli_err), 1);
}

#[test]
fn cli_error_from_rebuild_error() {
    let rb_err = RebuildError::ProjectionNotFound("p-1".into());
    let cli_err = CliError::from(rb_err);
    assert!(cli_err.to_string().contains("p-1"));
    assert_eq!(map_error_to_exit_code(&cli_err), 1);
}

#[test]
fn check_error_file_too_small_message() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    let msg = err.to_string();
    assert!(msg.contains("4 bytes"));
    assert!(msg.contains("/tiny"));
}

#[test]
fn check_error_invalid_magic_format() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/bad"),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = err.to_string();
    assert!(msg.contains("/bad"));
    assert!(msg.contains("0xde"));
    assert!(msg.contains("0xad"));
    assert!(msg.contains("0xbe"));
    assert!(msg.contains("0xef"));
}

#[test]
fn check_error_permission_denied_message() {
    let err = CheckError::PermissionDenied {
        path: PathBuf::from("/root/secret"),
    };
    assert!(err.to_string().contains("/root/secret"));
}

#[test]
fn init_error_io_message() {
    let err = InitError::Io {
        path: PathBuf::from("/io-err"),
        reason: "read failure".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/io-err"));
    assert!(msg.contains("read failure"));
}

#[test]
fn init_error_symlink_message() {
    let err = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/link"));
    assert!(msg.contains("symlink"));
    assert!(msg.contains("refusing"));
}

#[test]
fn lock_error_empty_workflows_message() {
    let err = LockError::NoWorkflowsDir {
        path: PathBuf::from("/no-wf"),
    };
    assert!(err.to_string().contains("/no-wf"));
}

#[test]
fn gc_config_custom_values() {
    let config = GcConfig {
        engine_url: "http://custom:9999".into(),
        versions_dir: PathBuf::from("/data/versions"),
        dry_run: true,
    };
    assert_eq!(config.engine_url, "http://custom:9999");
    assert_eq!(config.versions_dir, PathBuf::from("/data/versions"));
    assert!(config.dry_run);
}

#[test]
fn rebuild_status_failed_format() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Failed {
            reason: "disk full".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(report.format_progress().contains("disk full"));
    assert!(report.format_progress().contains("failed"));
}

#[test]
fn rebuild_status_noop_format() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: None,
        status: RebuildStatus::NoOp {
            reason: "up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(report.format_progress().contains("up to date"));
    assert!(report.format_progress().contains("skipped"));
}

#[test]
fn rebuild_status_started_format() {
    let report = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("42"));
    assert!(output.contains("started"));
}

#[test]
fn rebuild_status_listed_empty() {
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
fn rebuild_status_listed_multiple() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["orders".into(), "inventory".into(), "users".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("orders"));
    assert!(output.contains("inventory"));
    assert!(output.contains("users"));
}

#[test]
fn validate_binary_header_mach_o_32_be() {
    let dir = tempfile::tempdir().unwrap();
    let path = make_binary(dir.path(), "macho32be", &MACHO_MAGIC_32_BE);
    assert_eq!(
        validate_binary_header(&path),
        Ok(BinaryFormat::MachO32BigEndian)
    );
}

#[test]
fn validate_binary_header_mach_o_32_le() {
    let dir = tempfile::tempdir().unwrap();
    let path = make_binary(dir.path(), "macho32le", &MACHO_MAGIC_32_LE);
    assert_eq!(
        validate_binary_header(&path),
        Ok(BinaryFormat::MachO32LittleEndian)
    );
}

#[test]
fn validate_binary_header_mach_o_64_be() {
    let dir = tempfile::tempdir().unwrap();
    let path = make_binary(dir.path(), "macho64be", &MACHO_MAGIC_64_BE);
    assert_eq!(
        validate_binary_header(&path),
        Ok(BinaryFormat::MachO64BigEndian)
    );
}

#[test]
fn validate_binary_header_mach_o_64_le() {
    let dir = tempfile::tempdir().unwrap();
    let path = make_binary(dir.path(), "macho64le", &MACHO_MAGIC_64_LE);
    assert_eq!(
        validate_binary_header(&path),
        Ok(BinaryFormat::MachO64LittleEndian)
    );
}

#[test]
fn validate_binary_header_elf() {
    let dir = tempfile::tempdir().unwrap();
    let path = make_binary(dir.path(), "elf", &ELF_MAGIC);
    assert_eq!(validate_binary_header(&path), Ok(BinaryFormat::Elf));
}

#[test]
fn validate_binary_header_invalid_magic() {
    let dir = tempfile::tempdir().unwrap();
    let path = make_binary(dir.path(), "invalid", &[0x00, 0x00, 0x00, 0x00]);
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn validate_binary_header_too_small() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny");
    fs::write(&path, [0x7Fu8, 0x45]).unwrap();
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn validate_binary_header_directory() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    let result = validate_binary_header(&subdir);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn known_magics_contains_all_five() {
    assert_eq!(KNOWN_MAGICS.len(), 5);
    assert!(KNOWN_MAGICS.contains(&ELF_MAGIC));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_32_BE));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_32_LE));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_64_BE));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_64_LE));
}

#[test]
fn doctor_report_format_stdout_structure() {
    let mut ws = CategoryReport::new(CheckCategory::Workspace);
    ws.push("vo-dir", Severity::Info, ".vo/ directory exists".into());
    let mut lock = CategoryReport::new(CheckCategory::LockState);
    lock.push("lockfile", Severity::Info, "no lockfile".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![ws, lock],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report: /proj"));
    assert!(stdout.contains("[workspace]"));
    assert!(stdout.contains("[lock-state]"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn doctor_report_format_mixed_severity() {
    let mut cat = CategoryReport::new(CheckCategory::ConfigValidation);
    cat.push("config-ok", Severity::Info, "config valid".into());
    cat.push("config-warn", Severity::Warn, "missing field".into());
    cat.push("config-err", Severity::Error, "bad toml".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("config valid"));
    assert!(stderr.contains("missing field"));
    assert!(stderr.contains("bad toml"));
    assert!(stderr.contains("error(s)"));
    assert!(stderr.contains("warning(s)"));
}

#[test]
fn doctor_report_json_roundtrip() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.push("check-a", Severity::Info, "ok".into());
    cat1.push("check-b", Severity::Warn, "warning".into());
    let mut cat2 = CategoryReport::new(CheckCategory::StorageIntegrity);
    cat2.push("check-c", Severity::Error, "error".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/test-project"),
        categories: vec![cat1, cat2],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(!parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 1);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 1);
    assert_eq!(parsed["categories"].as_array().unwrap().len(), 2);

    let ws = &parsed["categories"][0];
    assert_eq!(ws["category"].as_str(), Some("workspace"));
    assert!(ws["healthy"].as_bool().unwrap());

    let si = &parsed["categories"][1];
    assert_eq!(si["category"].as_str(), Some("storage-integrity"));
    assert!(!si["healthy"].as_bool().unwrap());
}

#[test]
fn config_toml_valid_roundtrip_init() {
    let dir = tempfile::tempdir().unwrap();
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://engine:9000".into(),
        storage_path: PathBuf::from(".vo/data"),
    };
    run_init(&config).unwrap();

    let content = fs::read_to_string(dir.path().join(CONFIG_FILE_NAME)).unwrap();
    assert!(content.contains("[engine]"));
    assert!(content.contains("url = \"http://engine:9000\""));
    assert!(content.contains("[storage]"));
    assert!(content.contains("path = \".vo/data\""));

    let table: toml::Table = content.parse().unwrap();
    assert_eq!(table["engine"]["url"].as_str(), Some("http://engine:9000"));
    assert_eq!(table["storage"]["path"].as_str(), Some(".vo/data"));
}

#[test]
fn config_toml_invalid_toml_detected() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(dir.path().join(CONFIG_FILE_NAME), "not valid [toml {{}").unwrap();

    let report = check_config_validation(dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "config-parseable" && c.severity == Severity::Error));
}

#[test]
fn config_toml_missing_both_sections() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[other]\nkey = \"val\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine"));
    assert!(report.warnings().any(|w| w.check == "config-storage"));
}

#[test]
fn config_toml_non_string_url_value() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = 42\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.checks.iter().any(|c| c.check == "config-engine-url"));
}

#[test]
fn config_toml_non_string_path_value() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = 123\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "config-storage-path"));
}

#[test]
fn init_creates_workflows_subdir() {
    let dir = tempfile::tempdir().unwrap();
    run_init(&InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    })
    .unwrap();
    assert!(dir.path().join(".vo/workflows").is_dir());
}

#[test]
fn init_nonexistent_dir_returns_error() {
    let result = run_init(&InitConfig {
        project_dir: PathBuf::from("/nonexistent/path/that/does/not/exist"),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    });
    assert!(matches!(result, Err(InitError::DirNotFound { .. })));
}

#[test]
fn init_file_as_project_dir_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("not-a-dir");
    fs::write(&file_path, b"data").unwrap();
    let result = run_init(&InitConfig {
        project_dir: file_path,
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    });
    assert!(matches!(result, Err(InitError::NotDirectory { .. })));
}

#[test]
fn lock_multiple_workflows_sorted_order() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let wf_dir = dir.path().join(".vo/workflows");
    fs::write(wf_dir.join("zebra"), b"z-data").unwrap();
    fs::write(wf_dir.join("alpha"), b"a-data").unwrap();
    fs::write(wf_dir.join("mid"), b"m-data").unwrap();

    let lockmap = run_lock(&LockConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .unwrap();

    let keys: Vec<_> = lockmap.keys().collect();
    assert_eq!(keys[0], "alpha");
    assert_eq!(keys[1], "mid");
    assert_eq!(keys[2], "zebra");
}

#[test]
fn lock_writes_file_correctly() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(dir.path().join(".vo/workflows/wf1"), b"content").unwrap();

    run_lock(&LockConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .unwrap();

    let lock_content = fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).unwrap();
    assert!(lock_content.starts_with("wf1 "));
    assert!(lock_content.ends_with('\n'));
}

#[test]
fn rebuild_config_with_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let result = run_rebuild(&RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("proj-1".into()),
        list_projections: false,
        force: false,
        schema_version: Some(2),
    });
    assert!(result.is_ok());
}

#[test]
fn e2e_init_lock_check_doctor_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    run_init(&InitConfig {
        project_dir: project.to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    })
    .unwrap();

    let wf_path = make_binary(project, ".vo/workflows/pipeline-wf", &ELF_MAGIC);
    let check_result = validate_binary_header(&wf_path);
    assert!(check_result.is_ok());

    let lockmap = run_lock(&LockConfig {
        project_dir: project.to_path_buf(),
    })
    .unwrap();
    assert_eq!(lockmap.len(), 1);

    let report = run_doctor(&DoctorConfig {
        project_dir: project.to_path_buf(),
    })
    .unwrap();
    assert!(report.is_healthy());

    let rb = run_rebuild(&RebuildConfig {
        project_dir: project.to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    })
    .unwrap();
    assert!(matches!(rb.status, RebuildStatus::Listed(_)));
}

#[test]
fn e2e_tamper_detection_via_doctor() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    setup_project(project);
    fs::write(project.join(".vo/workflows/my-wf"), b"original").unwrap();
    run_lock(&LockConfig {
        project_dir: project.to_path_buf(),
    })
    .unwrap();

    fs::write(project.join(".vo/workflows/my-wf"), b"tampered!").unwrap();

    let report = run_doctor(&DoctorConfig {
        project_dir: project.to_path_buf(),
    })
    .unwrap();
    assert!(!report.is_healthy());
    assert!(report.errors().any(|e| e.check == "lock-integrity"));
}

#[test]
fn cli_debug_format_all_commands() {
    let commands = vec![
        Command::Purge {
            instance: "i".into(),
        },
        Command::Check {
            path: PathBuf::from("/p"),
        },
        Command::Gc {
            engine_url: "http://x".into(),
            dry_run: true,
        },
        Command::Init {
            project_dir: PathBuf::from("."),
            engine_url: "http://x".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
        Command::Lock {
            project_dir: PathBuf::from("."),
        },
        Command::Doctor {
            project_dir: PathBuf::from("."),
        },
        Command::Rebuild {
            project_dir: PathBuf::from("."),
            projection_id: Some("p".into()),
            list_projections: true,
            force: true,
        },
    ];
    for cmd in commands {
        let cli = Cli { command: cmd };
        let debug = format!("{:?}", cli);
        assert!(!debug.is_empty());
    }
}

#[test]
fn check_error_equality_same_path() {
    let e1 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    assert_eq!(e1, e2);
}

#[test]
fn check_error_equality_different_variants() {
    let e1 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::PermissionDenied {
        path: PathBuf::from("/a"),
    };
    assert_ne!(e1, e2);
}

#[test]
fn check_error_equality_invalid_magic() {
    let e1 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [1, 2, 3, 4],
    };
    let e2 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [1, 2, 3, 4],
    };
    let e3 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [5, 6, 7, 8],
    };
    assert_eq!(e1, e2);
    assert_ne!(e1, e3);
}

#[test]
fn parse_strict_numeric_large_valid() {
    assert_eq!(
        parse_strict_numeric("1000000000000").unwrap(),
        1000000000000
    );
}

#[test]
fn parse_strict_numeric_error_messages_content() {
    let empty = parse_strict_numeric("").unwrap_err();
    assert!(empty.to_string().contains("empty"));

    let plus = parse_strict_numeric("+5").unwrap_err();
    assert!(plus.to_string().contains("plus"));

    let neg = parse_strict_numeric("-5").unwrap_err();
    assert!(neg.to_string().contains("negative"));

    let overflow = parse_strict_numeric("99999999999999999999999999").unwrap_err();
    assert!(overflow.to_string().contains("overflow"));

    let alpha = parse_strict_numeric("abc").unwrap_err();
    assert!(alpha.to_string().contains("invalid"));
}

#[test]
fn file_hash_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty");
    fs::write(&path, b"").unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn file_hash_large_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large");
    let data = vec![0xABu8; 1024 * 1024];
    fs::write(&path, &data).unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
}

#[test]
fn sha256_hex_various_inputs() {
    assert_eq!(sha256_hex("hello").len(), 64);
    assert_eq!(sha256_hex("x").chars().filter(|c| *c == 'x').count(), 1);
    assert!(sha256_hex("abc").starts_with("abc"));
    assert!(sha256_hex("abc").ends_with("00000000"));
}
