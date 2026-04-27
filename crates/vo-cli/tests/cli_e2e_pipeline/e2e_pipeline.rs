use std::path::PathBuf;

use vo_cli::commands::check::{validate_binary_header, BinaryFormat, CheckError};
use vo_cli::commands::doctor::{run_doctor, DoctorConfig, DoctorError};
use vo_cli::commands::init::{run_init, InitConfig, InitError, CONFIG_FILE_NAME};
use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};

use super::helpers::{create_elf_binary, create_workflow_binary, setup_project};

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
