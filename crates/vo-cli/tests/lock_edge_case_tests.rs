use std::path::PathBuf;
use vo_cli::commands::lock::{LockConfig, LockError, LOCK_FILE_NAME};

#[test]
fn lock_file_format_is_name_space_hash() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vo_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&vo_dir).expect("mkdir");
    std::fs::write(
        vo_dir.join("wf1"),
        [0x7Fu8, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00],
    )
    .expect("write");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = vo_cli::commands::lock::run_lock(&config).expect("lock");
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("wf1"));

    let lock_content = std::fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).expect("read");
    let parts: Vec<&str> = lock_content.trim().splitn(2, ' ').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "wf1");
    assert_eq!(parts[1].len(), 64);
}

#[test]
fn lock_with_multiple_workflows_sorts_by_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vo_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&vo_dir).expect("mkdir");
    for name in ["c_wf", "a_wf", "b_wf"] {
        std::fs::write(vo_dir.join(name), b"\x7fELF\x00\x00\x00\x00").expect("write");
    }

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = vo_cli::commands::lock::run_lock(&config).expect("lock");
    assert_eq!(lockmap.len(), 3);

    let keys: Vec<&String> = lockmap.keys().collect();
    assert_eq!(keys[0], "a_wf");
    assert_eq!(keys[1], "b_wf");
    assert_eq!(keys[2], "c_wf");
}

#[test]
fn lock_rejects_non_directory_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("a_file");
    std::fs::write(&file_path, b"not a dir").expect("write");

    let config = LockConfig {
        project_dir: file_path,
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_ignores_subdirectories_in_workflows() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vo_dir = dir.path().join(".vo/workflows");
    std::fs::create_dir_all(&vo_dir).expect("mkdir");
    std::fs::write(vo_dir.join("real_wf"), b"\x7fELF\x00").expect("write");
    std::fs::create_dir_all(vo_dir.join("subdir")).expect("mkdir");

    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = vo_cli::commands::lock::run_lock(&config).expect("lock");
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("real_wf"));
}
