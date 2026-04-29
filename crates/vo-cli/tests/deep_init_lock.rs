#![allow(clippy::redundant_pattern_matching)]
use common::make_temp_dir;
use std::path::PathBuf;
use vo_cli::{InitConfig, InitError, LockError};

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
