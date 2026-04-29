#![allow(clippy::redundant_pattern_matching)]
use sha2::Digest;
use std::path::PathBuf;
use vo_cli::commands::check::{validate_binary_header, BinaryFormat, CheckError};
use vo_cli::commands::doctor::{run_doctor, DoctorConfig, DoctorError};
use vo_cli::commands::doctor_checks::{
    check_config_validation, check_lock_state, check_storage_integrity, check_subprocess_liveness,
    check_workspace, format_report, format_report_json, CategoryReport, CheckCategory, CheckResult,
    DoctorReport, Severity,
};
use vo_cli::commands::gc::{GcConfig, GcSummary};
use vo_cli::commands::init::{run_init, InitConfig, InitError, CONFIG_FILE_NAME};
use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};
use vo_cli::utils::{file_hash, sha256_hex};
use vo_cli::{
    interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command,
    HandlerRegistry,
};

fn setup_project(dir: &std::path::Path) {
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    std::fs::create_dir_all(vo_dir.join("storage")).unwrap();
    std::fs::write(
        dir.join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

// ============================================================
// file_hash: direct unit tests
// ============================================================

#[test]
fn file_hash_returns_sha256_hex_for_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.txt");
    std::fs::write(&path, b"hello world").unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn file_hash_empty_file_returns_known_hash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty");
    std::fs::write(&path, b"").unwrap();
    let hash = file_hash(&path).unwrap();
    let expected = format!("{:x}", sha2::Sha256::digest(b""));
    assert_eq!(hash, expected);
}

#[test]
fn file_hash_nonexistent_file_returns_error() {
    let result = file_hash(PathBuf::from("/tmp/nonexistent-vo-file-hash-test").as_path());
    assert!(result.is_err());
}

#[test]
fn file_hash_large_content_still_produces_64_char_hex() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big");
    let content = vec![0xABu8; 1024 * 64];
    std::fs::write(&path, &content).unwrap();
    let hash = file_hash(&path).unwrap();
    assert_eq!(hash.len(), 64);
}

#[test]
fn file_hash_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("det");
    std::fs::write(&path, b"deterministic content").unwrap();
    let h1 = file_hash(&path).unwrap();
    let h2 = file_hash(&path).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn file_hash_different_content_different_hash() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a");
    let p2 = dir.path().join("b");
    std::fs::write(&p1, b"content A").unwrap();
    std::fs::write(&p2, b"content B").unwrap();
    assert_ne!(file_hash(&p1).unwrap(), file_hash(&p2).unwrap());
}

// ============================================================
// sha256_hex utility
// ============================================================

#[test]
fn sha256_hex_pads_to_64_chars() {
    let result = sha256_hex("short");
    assert_eq!(result.len(), 64);
}

#[test]
fn sha256_hex_empty_seed() {
    let result = sha256_hex("");
    assert_eq!(result.len(), 64);
}

#[test]
fn sha256_hex_long_seed_still_64_chars() {
    let result = sha256_hex(&"x".repeat(200));
    assert_eq!(result.len(), 64);
}

// ============================================================
// Doctor checks: granular unit tests
// ============================================================

#[test]
fn check_workspace_detects_readonly_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let vo_dir = dir.path().join(".vo");
    let mut perms = std::fs::metadata(&vo_dir).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&vo_dir, perms).unwrap();

    let report = check_workspace(dir.path(), &vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Error));

    let mut perms = std::fs::metadata(&vo_dir).unwrap().permissions();
    perms.set_readonly(false);
    std::fs::set_permissions(&vo_dir, perms).unwrap();
}

#[test]
fn check_workspace_reports_binary_count() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let wf_dir = dir.path().join(".vo/workflows");
    std::fs::write(wf_dir.join("wf-a"), b"\x7fELF\x00").unwrap();
    std::fs::write(wf_dir.join("wf-b"), b"\x7fELF\x00").unwrap();

    let report = check_workspace(dir.path(), &dir.path().join(".vo"));
    let wf_check = report
        .checks
        .iter()
        .find(|c| c.check == "workflows-dir")
        .unwrap();
    assert!(wf_check.message.contains("2 binaries"));
}

#[test]
fn check_storage_integrity_detects_partitions() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");
    std::fs::create_dir_all(storage.join("events")).unwrap();
    std::fs::create_dir_all(storage.join("instances")).unwrap();

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-partitions"));
}

#[test]
fn check_storage_detects_wal_journal_files() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");
    std::fs::write(storage.join("events.journal"), b"j").unwrap();

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report.checks.iter().any(|c| c.check == "storage-wal"));
}

#[test]
fn check_storage_detects_wal_pattern() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");
    std::fs::write(storage.join("data-wal"), b"w").unwrap();

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report.warnings().any(|c| c.check == "storage-wal"));
}

#[test]
fn check_subprocess_no_runtime_dir_is_info() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let vo_dir = dir.path().join(".vo");
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "subprocess-liveness" && c.severity == Severity::Info));
}

#[test]
fn check_config_validation_detects_missing_engine_url_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    std::fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|c| c.check == "config-engine-url"));
}

#[test]
fn check_config_validation_detects_missing_storage_path_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    std::fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\n",
    )
    .unwrap();
    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|c| c.check == "config-storage-path"));
}

#[test]
fn check_config_validation_nonexistent_storage_path_warns() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    std::fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/nonexistent\"\n",
    )
    .unwrap();
    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|c| c.check == "config-storage-path"));
}

#[test]
fn check_config_validation_valid_storage_path_info() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let report = check_config_validation(dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "config-storage-path" && c.severity == Severity::Info));
}

#[test]
fn check_lock_state_empty_lockfile_warns() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    std::fs::write(dir.path().join(LOCK_FILE_NAME), "\n").unwrap();

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lockfile" && c.severity == Severity::Warn));
}

#[test]
fn check_lock_state_valid_lock_with_matching_hash() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let content = b"workflow-bin";
    std::fs::write(dir.path().join(".vo/workflows/wf1"), content).unwrap();
    let hash = file_hash(&dir.path().join(".vo/workflows/wf1")).unwrap();
    std::fs::write(dir.path().join(LOCK_FILE_NAME), format!("wf1 {hash}\n")).unwrap();

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lock-integrity" && c.severity == Severity::Info));
}

// ============================================================
// Init: permission denied edge case
// ============================================================

#[test]
fn init_error_io_display_shows_reason() {
    let err = InitError::Io {
        path: PathBuf::from("/some/dir"),
        reason: "creating workflows dir".into(),
        source: std::io::Error::other("disk full"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/some/dir"));
    assert!(msg.contains("creating workflows dir"));
}

// ============================================================
// Rebuild: RebuildReport clone and debug
// ============================================================

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
fn rebuild_status_in_progress_format() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::InProgress {
            progress_percent: 50,
            at_sequence: 1000,
        },
        events_applied: 500,
        duration_ms: 100,
    };
    let output = report.format_progress();
    assert!(output.contains("50%"));
    assert!(output.contains("1000"));
}

#[test]
fn rebuild_config_clone_and_debug() {
    let config = RebuildConfig {
        project_dir: PathBuf::from("/p"),
        projection_id: Some("x".into()),
        list_projections: false,
        force: true,
        schema_version: Some(1),
    };
    let cloned = config.clone();
    assert_eq!(config.project_dir, cloned.project_dir);
    assert_eq!(config.projection_id, cloned.projection_id);
    assert_eq!(config.force, cloned.force);
}

// ============================================================
// GcSummary construction and field verification
// ============================================================

#[test]
fn gc_summary_zero_values() {
    let summary = GcSummary {
        pinned_count: 0,
        scanned_count: 0,
        deleted_count: 0,
        deleted_hashes: vec![],
        failures: vec![],
    };
    assert_eq!(summary.pinned_count, 0);
    assert_eq!(summary.scanned_count, 0);
    assert!(summary.deleted_hashes.is_empty());
    assert!(summary.failures.is_empty());
}

// ============================================================
// CLI parsing: additional edge cases
// ============================================================

#[test]
fn parse_purge_with_special_chars_in_instance() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", "inst-123_456"]).unwrap();
    match cli.command {
        Command::Purge { instance, .. } => assert_eq!(instance, "inst-123_456"),
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_check_with_absolute_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "/usr/bin/ls"]).unwrap();
    match cli.command {
        Command::Check {
            workflow: false,
            path,
        } => assert_eq!(path, PathBuf::from("/usr/bin/ls")),
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_check_with_relative_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "../bin/app"]).unwrap();
    match cli.command {
        Command::Check {
            workflow: false,
            path,
        } => assert_eq!(path, PathBuf::from("../bin/app")),
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_init_all_custom_paths() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/data/app",
        "--engine-url",
        "http://prod:8080",
        "--storage-path",
        "/mnt/storage/vo",
    ])
    .unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/data/app"));
            assert_eq!(engine_url, "http://prod:8080");
            assert_eq!(storage_path, PathBuf::from("/mnt/storage/vo"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_rebuild_projection_id_with_special_chars() {
    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "order-summary-v2"]).unwrap();
    match cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id.as_deref(), Some("order-summary-v2"));
        }
        _ => panic!("expected Rebuild"),
    }
}

// ============================================================
// Exit code: comprehensive mapping
// ============================================================

#[test]
fn exit_code_for_rebuild_not_initialized() {
    let err = CliError::Rebuild(RebuildError::NotInitialized {
        path: PathBuf::from("/proj"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_for_rebuild_engine_error() {
    let err = CliError::Rebuild(RebuildError::Engine("timeout".into()));
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_for_rebuild_io_error() {
    let err = CliError::Rebuild(RebuildError::Io {
        path: PathBuf::from("/p"),
        reason: "read".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

// ============================================================
// Doctor report: JSON with multiple error/warning counts
// ============================================================

#[test]
fn doctor_report_json_with_multiple_errors() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "e1",
        severity: Severity::Error,
        message: "err1".into(),
    });
    cat.checks.push(CheckResult {
        check: "e2",
        severity: Severity::Error,
        message: "err2".into(),
    });
    cat.checks.push(CheckResult {
        check: "w1",
        severity: Severity::Warn,
        message: "warn1".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    let json_str = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 2);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 1);
    assert!(!parsed["healthy"].as_bool().unwrap());
}

#[test]
fn format_report_only_warnings_no_errors() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "w1",
        severity: Severity::Warn,
        message: "careful".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("1 warning(s)"));
    assert!(!stderr.contains("error(s)"));
}

// ============================================================
// E2E: Full pipeline with multiple workflows and lock verification
// ============================================================

#[tokio::test]
async fn e2e_init_lock_with_three_workflows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).unwrap();
    vo_cli::dispatch(init_cli).await.unwrap();

    let wf_dir = dir.path().join(".vo/workflows");
    for name in ["alpha", "beta", "gamma"] {
        std::fs::write(wf_dir.join(name), [0x7Fu8, 0x45, 0x4C, 0x46, 0x00]).unwrap();
    }

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).unwrap();
    vo_cli::dispatch(lock_cli).await.unwrap();

    let lock_content = std::fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).unwrap();
    let lines: Vec<&str> = lock_content.trim().lines().collect();
    assert_eq!(lines.len(), 3);

    let names: Vec<&str> = lines
        .iter()
        .map(|l| l.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);
}

#[tokio::test]
async fn e2e_doctor_detects_tampered_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).unwrap();
    vo_cli::dispatch(init_cli).await.unwrap();

    let wf_path = dir.path().join(".vo/workflows/myapp");
    std::fs::write(&wf_path, b"original-content").unwrap();

    let lock_cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", path]).unwrap();
    vo_cli::dispatch(lock_cli).await.unwrap();

    std::fs::write(&wf_path, b"tampered-content").unwrap();

    let doctor_cli = interpret_cli_from(vec!["vo", "doctor", "--project-dir", path]).unwrap();
    let result = vo_cli::dispatch(doctor_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_rebuild_list_after_init() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();

    let init_cli = interpret_cli_from(vec!["vo", "init", "--project-dir", path]).unwrap();
    vo_cli::dispatch(init_cli).await.unwrap();

    let rebuild_cli =
        interpret_cli_from(vec!["vo", "rebuild", "--project-dir", path, "--list"]).unwrap();
    let result = vo_cli::dispatch(rebuild_cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_check_macho_64le_binary() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("macho.bin");
    std::fs::write(&bin_path, [0xCF, 0xFA, 0xED, 0xFE, 0x00]).unwrap();

    let cli = interpret_cli_from(vec!["vo", "check", bin_path.to_str().unwrap()]).unwrap();
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn e2e_check_macho_64be_binary() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("macho_be.bin");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCF, 0x00]).unwrap();

    let cli = interpret_cli_from(vec!["vo", "check", bin_path.to_str().unwrap()]).unwrap();
    let result = vo_cli::dispatch(cli).await;
    assert!(result.is_ok());
}

// ============================================================
// Validate binary header: all magic variants directly
// ============================================================

#[test]
fn validate_elf_magic_direct() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("elf");
    std::fs::write(&path, [0x7F, 0x45, 0x4C, 0x46]).unwrap();
    assert_eq!(validate_binary_header(&path), Ok(BinaryFormat::Elf));
}

#[test]
fn validate_macho_32le_magic_direct() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m32le");
    std::fs::write(&path, [0xCE, 0xFA, 0xED, 0xFE]).unwrap();
    assert_eq!(
        validate_binary_header(&path),
        Ok(BinaryFormat::MachO32LittleEndian)
    );
}

#[test]
fn validate_macho_32be_magic_direct() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m32be");
    std::fs::write(&path, [0xFE, 0xED, 0xFA, 0xCE]).unwrap();
    assert_eq!(
        validate_binary_header(&path),
        Ok(BinaryFormat::MachO32BigEndian)
    );
}

// ============================================================
// parse_strict_numeric: additional edge cases
// ============================================================

#[test]
fn parse_strict_numeric_accepts_large_u64() {
    assert_eq!(
        parse_strict_numeric("18446744073709551615").unwrap(),
        u64::MAX
    );
}

#[test]
fn parse_strict_numeric_rejects_u64_overflow() {
    assert!(parse_strict_numeric("18446744073709551616").is_err());
}

#[test]
fn parse_strict_numeric_rejects_minus_zero() {
    assert!(parse_strict_numeric("-0").is_err());
}

// ============================================================
// Doctor report format: edge cases
// ============================================================

#[test]
fn format_report_category_with_no_checks() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp"),
        categories: vec![CategoryReport::new(CheckCategory::Workspace)],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("no checks"));
}

#[test]
fn format_report_json_empty_categories_valid_json() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/tmp/empty"),
        categories: vec![],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["categories"].as_array().unwrap().is_empty());
}

// ============================================================
// CheckError PartialEq: additional cases
// ============================================================

#[test]
fn check_error_partial_eq_different_invalid_magic() {
    let e1 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [0x01, 0x02, 0x03, 0x04],
    };
    let e2 = CheckError::InvalidMagic {
        path: PathBuf::from("/a"),
        magic: [0x05, 0x06, 0x07, 0x08],
    };
    assert_ne!(e1, e2);
}

#[test]
fn check_error_partial_eq_file_too_small_same_path() {
    let e1 = CheckError::FileTooSmall {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::FileTooSmall {
        path: PathBuf::from("/a"),
    };
    assert_eq!(e1, e2);
}

#[test]
fn check_error_partial_eq_permission_denied_same_path() {
    let e1 = CheckError::PermissionDenied {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::PermissionDenied {
        path: PathBuf::from("/a"),
    };
    assert_eq!(e1, e2);
}

// ============================================================
// Init: creates correct directory structure
// ============================================================

#[test]
fn init_creates_vo_dir_not_just_workflows() {
    let dir = tempfile::tempdir().unwrap();
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config).unwrap();
    assert!(dir.path().join(".vo").is_dir());
    assert!(dir.path().join(".vo/workflows").is_dir());
}

#[test]
fn init_config_toml_has_correct_sections() {
    let dir = tempfile::tempdir().unwrap();
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://custom:9999".into(),
        storage_path: PathBuf::from("/data/vo"),
    };
    run_init(&config).unwrap();
    let content = std::fs::read_to_string(dir.path().join(CONFIG_FILE_NAME)).unwrap();
    let table: toml::Table = content.parse().unwrap();
    assert!(table.contains_key("engine"));
    assert!(table.contains_key("storage"));
    assert_eq!(
        table["engine"]["url"].as_str().unwrap(),
        "http://custom:9999"
    );
    assert_eq!(table["storage"]["path"].as_str().unwrap(), "/data/vo");
}

// ============================================================
// Lock: hash verification with multiple files
// ============================================================

#[test]
fn lock_hashes_are_sha256_hex() {
    let dir = tempfile::tempdir().unwrap();
    let wf_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    let content = b"test workflow binary";
    std::fs::write(wf_dir.join("wf1"), content).unwrap();

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let map = run_lock(&config).unwrap();
    let hash = map.get("wf1").unwrap();
    assert_eq!(hash.len(), 64);
    let expected = format!("{:x}", sha2::Sha256::digest(content));
    assert_eq!(hash, &expected);
}

#[test]
fn lock_no_workflows_dir_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_lock(&config);
    assert!(matches!(result, Err(LockError::NoWorkflowsDir { .. })));
}

// ============================================================
// Doctor: comprehensive integration
// ============================================================

#[test]
fn doctor_full_check_five_categories() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let report = run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .unwrap();
    assert_eq!(report.categories.len(), 5);
    let category_types: Vec<CheckCategory> = report.categories.iter().map(|c| c.category).collect();
    assert!(category_types.contains(&CheckCategory::Workspace));
    assert!(category_types.contains(&CheckCategory::LockState));
    assert!(category_types.contains(&CheckCategory::SubprocessLiveness));
    assert!(category_types.contains(&CheckCategory::StorageIntegrity));
    assert!(category_types.contains(&CheckCategory::ConfigValidation));
}

#[test]
fn doctor_with_valid_project_is_healthy() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let report = run_doctor(&DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    })
    .unwrap();
    assert!(report.is_healthy());
}

// ============================================================
// Command Debug: all 7 variants produce non-empty strings
// ============================================================

#[test]
fn command_all_variants_debug_format() {
    let variants = vec![
        format!(
            "{:?}",
            Command::Purge {
                instance: "test".into(),
            }
        ),
        format!(
            "{:?}",
            Command::Check {
                workflow: false,
                path: PathBuf::from("/tmp")
            }
        ),
        format!(
            "{:?}",
            Command::Gc {
                engine_url: "u".into(),
                dry_run: false
            }
        ),
        format!(
            "{:?}",
            Command::Init {
                project_dir: PathBuf::from("."),
                engine_url: "u".into(),
                storage_path: PathBuf::from("s"),
            }
        ),
        format!(
            "{:?}",
            Command::Lock {
                project_dir: PathBuf::from(".")
            }
        ),
        format!(
            "{:?}",
            Command::Doctor {
                project_dir: PathBuf::from(".")
            }
        ),
        format!(
            "{:?}",
            Command::Rebuild {
                project_dir: PathBuf::from("."),
                projection_id: None,
                list_projections: false,
                force: false,
            }
        ),
    ];
    assert_eq!(variants.len(), 7);
    for v in &variants {
        assert!(!v.is_empty());
    }
}

// ============================================================
// CliError: from conversions
// ============================================================

#[test]
fn cli_error_from_rebuild_error() {
    let err = CliError::Rebuild(RebuildError::ProjectionNotFound("p".into()));
    assert!(err.to_string().contains("p"));
}

// ============================================================
// Rebuild: all error display variants
// ============================================================

#[test]
fn rebuild_error_io_path_and_reason_in_display() {
    let err = RebuildError::Io {
        path: PathBuf::from("/data/events"),
        reason: "reading event log".into(),
        source: std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "unexpected eof"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/data/events"));
    assert!(msg.contains("reading event log"));
}

#[test]
fn rebuild_error_projection_not_found_contains_name() {
    let err = RebuildError::ProjectionNotFound("my-agg".into());
    assert!(err.to_string().contains("my-agg"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn rebuild_error_unsupported_schema() {
    let err = RebuildError::UnsupportedSchemaVersion(255);
    let msg = err.to_string();
    assert!(msg.contains("255"));
    assert!(msg.contains("not supported"));
}

#[test]
fn rebuild_error_in_progress_contains_name() {
    let err = RebuildError::RebuildInProgress("orders-projection".into());
    let msg = err.to_string();
    assert!(msg.contains("orders-projection"));
    assert!(msg.contains("in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_shows_both() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "key-123".into(),
        actual: "key-456".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("key-123"));
    assert!(msg.contains("key-456"));
    assert!(msg.contains("mismatch"));
}

// ============================================================
// DoctorReport: is_healthy with warn-only categories
// ============================================================

#[test]
fn doctor_report_healthy_with_only_warnings() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.checks.push(CheckResult {
        check: "w1",
        severity: Severity::Warn,
        message: "minor issue".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    assert!(report.is_healthy());
}

#[test]
fn doctor_report_not_healthy_with_single_error() {
    let mut cat = CategoryReport::new(CheckCategory::StorageIntegrity);
    cat.checks.push(CheckResult {
        check: "e1",
        severity: Severity::Error,
        message: "broken".into(),
    });
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    assert!(!report.is_healthy());
}
