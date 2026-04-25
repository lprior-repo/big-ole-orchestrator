#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use common::make_temp_dir;
use vo_cli::{InitConfig, validate_binary_header, BinaryFormat, CheckCategory, Severity};
use vo_cli::commands::rebuild::{RebuildConfig, RebuildStatus};

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

    let doctor_config = vo_cli::DoctorConfig {
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

    let doctor_config = vo_cli::DoctorConfig {
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

    let result = validate_binary_header(&bin_path);
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
