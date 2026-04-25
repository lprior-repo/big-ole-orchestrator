use std::path::PathBuf;
use vo_cli::commands::init::{InitConfig, InitError};

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
