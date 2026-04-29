//! Consolidated E2E pipeline and business logic tests.
//!
//! Replaces duplicated e2e/pipeline/integration tests from 7+ test files.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use vo_cli::commands::doctor_checks::{
    check_config_validation, check_lock_state, check_storage_integrity, check_workspace,
    format_report, format_report_json, CategoryReport, CheckCategory, CheckResult, DoctorReport,
    Severity,
};
use vo_cli::{
    validate_binary_header, BinaryFormat, CheckError,
    ELF_MAGIC, KNOWN_MAGICS, MACHO_MAGIC_32_BE, MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE,
    MACHO_MAGIC_64_LE,
    run_doctor, DoctorConfig, DoctorError,
    run_init, InitConfig, InitError,
    run_lock, LockConfig, LockError, LOCK_FILE_NAME,
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};

fn make_temp_dir() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().to_path_buf();
    std::mem::forget(dir);
    p
}

fn setup_project(dir: &Path) {
    let vo_dir = dir.join(".vo");
    fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    fs::create_dir_all(vo_dir.join("storage")).unwrap();
    fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

fn create_workflow_binary(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let workflows = dir.join(".vo").join("workflows");
    fs::create_dir_all(&workflows).unwrap();
    let path = workflows.join(name);
    fs::write(&path, content).unwrap();
    path
}

fn elf_bytes() -> Vec<u8> {
    vec![0x7f, 0x45, 0x4c, 0x46, 0x00, 0x00, 0x00, 0x00]
}

// ---------------------------------------------------------------------------
// E2E Pipeline tests
// ---------------------------------------------------------------------------

#[test]
fn e2e_full_pipeline_happy_path() {
    let dir = make_temp_dir();
    let cfg = InitConfig { project_dir: dir.clone(), ..Default::default() };
    run_init(&cfg).unwrap();
    create_workflow_binary(&dir, "mybin", &elf_bytes());
    run_lock(&LockConfig { project_dir: dir.clone() }).unwrap();
    let report = run_doctor(&DoctorConfig { project_dir: dir.clone() }).unwrap();
    assert!(report.is_healthy());
    let rebuild_cfg = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let rebuild_report = run_rebuild(&rebuild_cfg).unwrap();
    assert!(matches!(rebuild_report.status, RebuildStatus::Listed(_)));
}

#[test]
fn e2e_init_lock_tamper_doctor_catches_mismatch() {
    let dir = make_temp_dir();
    run_init(&InitConfig { project_dir: dir.clone(), ..Default::default() }).unwrap();
    create_workflow_binary(&dir, "mybin", &vec![0x7f, 0x45, 0x4c, 0x46, 0x00]);
    run_lock(&LockConfig { project_dir: dir.clone() }).unwrap();
    create_workflow_binary(&dir, "mybin", &vec![0x7f, 0x45, 0x4c, 0x46, 0xFF]);
    let report = run_doctor(&DoctorConfig { project_dir: dir.clone() }).unwrap();
    assert!(!report.is_healthy());
}

#[test]
fn e2e_init_idempotent_same_config() {
    let dir = make_temp_dir();
    let cfg = InitConfig { project_dir: dir.clone(), ..Default::default() };
    let r1 = run_init(&cfg).unwrap();
    let r2 = run_init(&cfg).unwrap();
    assert_eq!(r1, r2);
}

#[test]
fn e2e_init_rejects_different_config() {
    let dir = make_temp_dir();
    run_init(&InitConfig { project_dir: dir.clone(), ..Default::default() }).unwrap();
    let other = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://other:9999".into(),
        storage_path: "/other".into(),
    };
    assert!(matches!(run_init(&other), Err(InitError::AlreadyInitialized { .. })));
}

// ---------------------------------------------------------------------------
// Business logic: error paths
// ---------------------------------------------------------------------------

#[test]
fn doctor_without_init_returns_error() {
    let dir = make_temp_dir();
    assert!(matches!(
        run_doctor(&DoctorConfig { project_dir: dir.clone() }),
        Err(DoctorError::NotInitialized { .. })
    ));
}

#[test]
fn rebuild_without_init_returns_error() {
    let dir = make_temp_dir();
    assert!(matches!(
        run_rebuild(&RebuildConfig {
            project_dir: dir.clone(),
            projection_id: None,
            list_projections: false,
            force: false,
            schema_version: None,
        }),
        Err(RebuildError::NotInitialized { .. })
    ));
}

#[test]
fn rebuild_requires_projection_id_or_list() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let cfg = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    assert!(run_rebuild(&cfg).is_err());
}

#[test]
fn rebuild_list_returns_empty_projections() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let cfg = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let report = run_rebuild(&cfg).unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

#[test]
fn rebuild_with_projection_id_succeeds() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let cfg = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("proj-1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let report = run_rebuild(&cfg).unwrap();
    assert!(matches!(report.status, RebuildStatus::Completed));
}

#[test]
fn lock_without_init_returns_error() {
    let dir = make_temp_dir();
    assert!(matches!(
        run_lock(&LockConfig { project_dir: dir.clone() }),
        Err(LockError::NotInitialized { .. })
    ));
}

#[test]
fn lock_with_empty_workflows_returns_error() {
    let dir = make_temp_dir();
    setup_project(&dir);
    assert!(matches!(
        run_lock(&LockConfig { project_dir: dir.clone() }),
        Err(LockError::Empty { .. })
    ));
}

#[test]
fn lock_multiple_workflows_sorted() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let elf: Vec<u8> = vec![0x7f, 0x45, 0x4c, 0x46, 0x00];
    create_workflow_binary(&dir, "z_wf", &elf);
    create_workflow_binary(&dir, "a_wf", &elf);
    create_workflow_binary(&dir, "m_wf", &elf);
    let result = run_lock(&LockConfig { project_dir: dir.clone() }).unwrap();
    let keys: Vec<_> = result.keys().collect();
    assert_eq!(keys.len(), 3);
    assert_eq!(keys[0], "a_wf");
    assert_eq!(keys[1], "m_wf");
    assert_eq!(keys[2], "z_wf");
}

// ---------------------------------------------------------------------------
// Init edge cases
// ---------------------------------------------------------------------------

#[test]
fn init_rejects_nonexistent_dir() {
    let cfg = InitConfig { project_dir: PathBuf::from("/nonexistent/path"), ..Default::default() };
    assert!(matches!(run_init(&cfg), Err(InitError::DirNotFound { .. })));
}

#[test]
fn init_rejects_symlink() {
    let dir = make_temp_dir();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    assert!(matches!(
        run_init(&InitConfig { project_dir: link, ..Default::default() }),
        Err(InitError::SymlinkTarget { .. })
    ));
}

#[test]
fn init_creates_vo_dir_and_workflows() {
    let dir = make_temp_dir();
    run_init(&InitConfig { project_dir: dir.clone(), ..Default::default() }).unwrap();
    assert!(dir.join(".vo").is_dir());
    assert!(dir.join(".vo").join("workflows").is_dir());
}

#[test]
fn init_writes_correct_config_toml() {
    let dir = make_temp_dir();
    run_init(&InitConfig { project_dir: dir.clone(), ..Default::default() }).unwrap();
    let content = fs::read_to_string(dir.join("config.toml")).unwrap();
    assert!(content.contains("[engine]"));
    assert!(content.contains("http://localhost:3000"));
    assert!(content.contains("[storage]"));
}

// ---------------------------------------------------------------------------
// Binary validation (rstest parameterized)
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::elf(vec![0x7f, 0x45, 0x4c, 0x46, 0x00, 0x00, 0x00, 0x00], BinaryFormat::Elf)]
#[case::macho_64le(vec![0xCF, 0xFA, 0xED, 0xFE, 0x00], BinaryFormat::MachO64LittleEndian)]
#[case::macho_64be(vec![0xFE, 0xED, 0xFA, 0xCF, 0x00], BinaryFormat::MachO64BigEndian)]
#[case::macho_32le(vec![0xCE, 0xFA, 0xED, 0xFE, 0x00], BinaryFormat::MachO32LittleEndian)]
#[case::macho_32be(vec![0xFE, 0xED, 0xFA, 0xCE, 0x00], BinaryFormat::MachO32BigEndian)]
fn validate_valid_binary_headers(#[case] magic: Vec<u8>, #[case] expected: BinaryFormat) {
    let dir = make_temp_dir();
    let path = dir.join("test.bin");
    fs::write(&path, magic).unwrap();
    assert_eq!(validate_binary_header(&path).unwrap(), expected);
}

#[test]
fn validate_too_small_file() {
    let dir = make_temp_dir();
    let path = dir.join("tiny");
    fs::write(&path, [0x00_u8, 0x01]).unwrap();
    assert!(matches!(validate_binary_header(&path), Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn validate_invalid_magic() {
    let dir = make_temp_dir();
    let path = dir.join("bad");
    fs::write(&path, [0xDE_u8, 0xAD, 0xBE, 0xEF, 0x00]).unwrap();
    assert!(matches!(validate_binary_header(&path), Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn validate_nonexistent_file() {
    assert!(matches!(
        validate_binary_header(Path::new("/nonexistent")),
        Err(CheckError::FileNotFound { .. })
    ));
}

#[test]
fn validate_symlink_rejected() {
    let dir = make_temp_dir();
    let real = dir.join("real");
    fs::write(&real, &[0x7f_u8, 0x45, 0x4c, 0x46, 0x00]).unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(matches!(validate_binary_header(&link), Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn validate_directory_rejected() {
    let dir = make_temp_dir();
    assert!(matches!(validate_binary_header(&dir), Err(CheckError::NotRegularFile { .. })));
}

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

#[test]
fn config_missing_engine_section() {
    let dir = make_temp_dir();
    setup_project(&dir);
    fs::write(dir.join("config.toml"), "[storage]\npath = \".vo/storage\"\n").unwrap();
    let report = check_config_validation(&dir);
    assert!(report.warnings().any(|c| c.check == "config-engine"));
}

#[test]
fn config_missing_storage_section() {
    let dir = make_temp_dir();
    setup_project(&dir);
    fs::write(dir.join("config.toml"), "[engine]\nurl = \"http://localhost:3000\"\n").unwrap();
    let report = check_config_validation(&dir);
    assert!(report.warnings().any(|c| c.check == "config-storage"));
}

#[test]
fn config_empty_engine_url() {
    let dir = make_temp_dir();
    setup_project(&dir);
    fs::write(dir.join("config.toml"), "[engine]\nurl = \"\"\n\n[storage]\npath = \".vo/storage\"\n").unwrap();
    let report = check_config_validation(&dir);
    assert!(report.warnings().any(|c| c.check == "config-engine-url"));
}

#[test]
fn config_readonly_permissions() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let cfg_path = dir.join("config.toml");
    let mut perms = fs::metadata(&cfg_path).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&cfg_path, perms).unwrap();
    let report = check_config_validation(&dir);
    assert!(report.warnings().any(|c| c.check == "config-perms"));
}

// ---------------------------------------------------------------------------
// Doctor checks
// ---------------------------------------------------------------------------

#[test]
fn doctor_workspace_readonly_vo_dir() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let vo = dir.join(".vo");
    let mut perms = fs::metadata(&vo).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&vo, perms.clone()).unwrap();
    let report = check_workspace(&dir, &vo);
    assert!(report.checks.iter().any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Error));
    let mut perms = fs::metadata(&vo).unwrap().permissions();
    perms.set_mode(0o755);
    let _ = fs::set_permissions(&vo, perms);
}

#[test]
fn doctor_lockstate_empty_lockfile() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let vo = dir.join(".vo");
    // Lockfile lives at project root, not inside .vo
    fs::write(dir.join(LOCK_FILE_NAME), "").unwrap();
    let report = check_lock_state(&dir, &vo);
    assert!(report.checks.iter().any(|c| c.check == "lockfile" && c.message.contains("empty")));
}

#[test]
fn doctor_storage_wal_patterns() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let storage = dir.join(".vo").join("storage");
    fs::write(storage.join("data.journal"), b"").unwrap();
    fs::write(storage.join("data-wal"), b"").unwrap();
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(report.warnings().any(|c| c.check == "storage-wal"));
}

// ---------------------------------------------------------------------------
// Report format tests
// ---------------------------------------------------------------------------

#[test]
fn format_report_empty_categories() {
    let report = DoctorReport { project_dir: PathBuf::from("."), categories: vec![] };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stdout.contains("All checks passed"));
    assert!(stderr.is_empty());
}

#[test]
fn format_report_with_errors_and_warnings() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![
                CheckResult { check: "ok".into(), severity: Severity::Info, message: "all good".into() },
                CheckResult { check: "warn".into(), severity: Severity::Warn, message: "watch out".into() },
                CheckResult { check: "err".into(), severity: Severity::Error, message: "broken".into() },
            ],
        }],
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
    let report = DoctorReport {
        project_dir: PathBuf::from("/proj"),
        categories: vec![CategoryReport {
            category: CheckCategory::Workspace,
            checks: vec![CheckResult { check: "test".into(), severity: Severity::Info, message: "ok".into() }],
        }],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["project_dir"], "/proj");
    assert!(parsed["healthy"].is_boolean());
}

// ---------------------------------------------------------------------------
// RebuildReport format_progress
// ---------------------------------------------------------------------------

fn make_report(status: RebuildStatus) -> RebuildReport {
    RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status,
        events_applied: 0,
        duration_ms: 0,
    }
}

#[test]
fn rebuild_format_progress_all_statuses() {
    let listed = make_report(RebuildStatus::Listed(vec!["p1".into(), "p2".into()]));
    assert!(listed.format_progress().contains("p1"));

    let started = make_report(RebuildStatus::Started { from_sequence: 42 });
    assert!(started.format_progress().contains("42"));

    let in_progress = make_report(RebuildStatus::InProgress { progress_percent: 75, at_sequence: 3000 });
    assert!(in_progress.format_progress().contains("75%"));

    let failed = make_report(RebuildStatus::Failed { reason: "disk full".into() });
    assert!(failed.format_progress().contains("disk full"));

    let noop = make_report(RebuildStatus::NoOp { reason: "up to date".into() });
    assert!(noop.format_progress().contains("up to date"));
}

// ---------------------------------------------------------------------------
// KNOWN_MAGICS constant
// ---------------------------------------------------------------------------

#[test]
fn known_magics_contains_all_five() {
    assert_eq!(KNOWN_MAGICS.len(), 5);
    assert!(KNOWN_MAGICS.contains(&ELF_MAGIC));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_32_BE));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_32_LE));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_64_BE));
    assert!(KNOWN_MAGICS.contains(&MACHO_MAGIC_64_LE));
}
