use std::path::PathBuf;
use vo_cli::commands::init::{run_init, InitConfig, VO_DIR_NAME};

#[test]
fn init_config_default_has_expected_engine_url() {
    let config = InitConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
}

#[test]
fn init_creates_vo_dir_in_project_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };

    let result = run_init(&config);

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    let vo_dir = result.expect("ok");
    assert!(vo_dir.is_dir());
    assert_eq!(vo_dir, dir.path().join(VO_DIR_NAME));
}

#[test]
fn init_creates_workflows_subdirectory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config).expect("init");
    let workflows = dir.path().join(".vo").join("workflows");
    assert!(workflows.is_dir(), "workflows dir should exist");
}

#[test]
fn init_creates_config_toml_with_engine_section() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config).expect("init");
    let config_path = dir.path().join("config.toml");
    assert!(config_path.exists(), "config.toml should exist");
    let content = std::fs::read_to_string(&config_path).expect("read");
    assert!(
        content.contains("[engine]"),
        "should contain [engine] section"
    );
    assert!(content.contains("http://localhost:3000"));
}

#[test]
fn init_rejects_nonexistent_directory() {
    let config = InitConfig {
        project_dir: PathBuf::from("/tmp/nonexistent-vo-init-test-xyz"),
        ..InitConfig::default()
    };
    let result = run_init(&config);
    assert!(result.is_err(), "should fail for nonexistent dir");
}

use vo_cli::commands::init::InitError;

#[test]
fn init_rejects_file_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("not_a_dir");
    std::fs::write(&file_path, b"data").expect("write");
    let config = InitConfig {
        project_dir: file_path,
        ..InitConfig::default()
    };
    let result = run_init(&config);
    assert!(matches!(result, Err(InitError::NotDirectory { .. })));
}

#[test]
fn init_rejects_symlink_target() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("target");
    std::fs::create_dir_all(&target).expect("mkdir");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");
    let config = InitConfig {
        project_dir: link,
        ..InitConfig::default()
    };
    let result = run_init(&config);
    assert!(matches!(result, Err(InitError::SymlinkTarget { .. })));
}

#[test]
fn init_idempotent_with_same_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let r1 = run_init(&config);
    let r2 = run_init(&config);
    assert!(r1.is_ok());
    assert!(r2.is_ok(), "second init should be idempotent");
}

#[test]
fn init_rejects_conflicting_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config1 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let config2 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:9999".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config1).expect("first init");
    let result = run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn init_error_dir_not_found_display() {
    let err = InitError::DirNotFound {
        path: PathBuf::from("/tmp/test"),
    };
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn init_error_not_directory_display() {
    let err = InitError::NotDirectory {
        path: PathBuf::from("/tmp/test"),
    };
    assert!(err.to_string().contains("not a directory"));
}

#[test]
fn init_error_symlink_display() {
    let err = InitError::SymlinkTarget {
        path: PathBuf::from("/tmp/link"),
    };
    assert!(err.to_string().contains("symlink"));
}

// ============================================================
// Lock command tests
// ============================================================

use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};

fn setup_init_project(dir: &std::path::Path) {
    let init_config = InitConfig {
        project_dir: dir.to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&init_config).expect("init");
}

#[test]
fn lock_creates_lockfile_in_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());

    // Place a fake workflow binary
    let workflows = dir.path().join(".vo").join("workflows");
    std::fs::write(
        workflows.join("my-workflow"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00],
    )
    .expect("write binary");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_lock(&config);

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    assert!(
        dir.path().join(LOCK_FILE_NAME).exists(),
        "lockfile should exist"
    );
}

#[test]
fn lock_file_contains_name_and_hash() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let wf = dir.path().join(".vo").join("workflows").join("test-wf");
    std::fs::write(&wf, b"hello").expect("write");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let map = run_lock(&config).expect("lock");

    assert_eq!(map.len(), 1);
    let (name, hash) = map.iter().next().expect("entry");
    assert_eq!(name, "test-wf");
    assert_eq!(hash.len(), 64, "SHA-256 hex should be 64 chars");

    let lockfile = std::fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).expect("read");
    assert!(lockfile.starts_with("test-wf "));
}

#[test]
fn lock_rejects_uninit_project() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_lock(&config);
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_rejects_empty_workflows_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    // No binaries placed

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_lock(&config);
    assert!(matches!(result, Err(LockError::Empty { .. })));
}

#[test]
fn lock_skips_subdirectories_in_workflows() {
    let dir = tempfile::tempdir().expect("tempdir");
    setup_init_project(dir.path());
    let wf_dir = dir.path().join(".vo").join("workflows");
    std::fs::write(wf_dir.join("real-bin"), [0x7F, 0x45, 0x4C, 0x46, 0x00]).expect("bin");
    std::fs::create_dir_all(wf_dir.join("subdir")).expect("mkdir");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let map = run_lock(&config).expect("lock");
    assert_eq!(map.len(), 1);
    assert!(map.contains_key("real-bin"));
}

#[test]
fn lock_error_not_initialized_display() {
    let err = LockError::NotInitialized {
        path: PathBuf::from("/tmp/test"),
    };
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn lock_error_empty_display() {
    let err = LockError::Empty {
        path: PathBuf::from("/tmp/test"),
    };
    assert!(err.to_string().contains("no workflow"));
}
