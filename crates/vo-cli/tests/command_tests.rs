use std::path::PathBuf;
mod test_helpers;
use test_helpers::{make_temp_dir, setup_project};
use vo_cli::{
    commands::init::InitConfig, run_check, run_doctor, run_lock, BinaryFormat, CheckCategory, CheckError,
    DoctorConfig, DoctorError, LockConfig, Severity,
};

// ============================================================
// CONFIG FILE LOADING EDGE CASES
// ============================================================

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
    assert!(matches!(
        result,
        Err(vo_cli::InitError::AlreadyInitialized { .. })
    ));
}

#[test]
fn init_fails_on_nonexistent_dir() {
    let config = InitConfig {
        project_dir: PathBuf::from("/no/such/directory/ever"),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::run_init(&config);
    assert!(matches!(result, Err(vo_cli::InitError::DirNotFound { .. })));
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
    assert!(matches!(
        result,
        Err(vo_cli::InitError::NotDirectory { .. })
    ));
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
    assert!(matches!(
        result,
        Err(vo_cli::InitError::SymlinkTarget { .. })
    ));
}

#[test]
fn init_config_default_values() {
    let config = InitConfig::default();
    assert_eq!(config.project_dir, PathBuf::from("."));
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.storage_path, PathBuf::from(".vo/storage"));
}

// ============================================================
// CHECK COMMAND EDGE CASES
// ============================================================

#[test]
fn check_valid_elf_binary() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.elf");
    std::fs::write(&bin_path, [0x7F, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::Elf);
}

#[test]
fn check_valid_macho_64_le() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xCF, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64LittleEndian);
}

#[test]
fn check_valid_macho_64_be() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCF, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64BigEndian);
}

#[test]
fn check_valid_macho_32_le() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xCE, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO32LittleEndian);
}

#[test]
fn check_valid_macho_32_be() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO32BigEndian);
}

#[test]
fn check_file_too_small_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("tiny.bin");
    std::fs::write(&bin_path, [0x7F, 0x45]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn check_invalid_magic_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("bad.bin");
    std::fs::write(&bin_path, [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&bin_path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn check_nonexistent_file_returns_not_found() {
    let result =
        vo_cli::validate_binary_header(PathBuf::from("/tmp/does-not-exist-co5-test").as_path());
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn check_symlink_returns_not_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, [0x7F, 0x45, 0x4C, 0x46]).unwrap();
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = vo_cli::validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn check_directory_returns_not_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let result = vo_cli::validate_binary_header(dir.path());
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

// ============================================================
// LOCK COMMAND EDGE CASES
// ============================================================

#[test]
fn lock_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(matches!(
        result,
        Err(vo_cli::LockError::NotInitialized { .. })
    ));
}

#[test]
fn lock_fails_without_workflows_dir() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(matches!(
        result,
        Err(vo_cli::LockError::NoWorkflowsDir { .. })
    ));
}

#[test]
fn lock_fails_with_empty_workflows() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    let config = vo_cli::LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_lock(&config);
    assert!(matches!(result, Err(vo_cli::LockError::Empty { .. })));
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

// ============================================================
// DOCTOR COMMAND EDGE CASES
// ============================================================

#[test]
fn doctor_fails_without_vo_dir() {
    let dir = make_temp_dir();
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_doctor(&config);
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn doctor_succeeds_on_initialized_project() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    assert_eq!(report.project_dir, dir);
    assert_eq!(report.categories.len(), 5);
}

#[test]
fn doctor_report_has_all_five_categories() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    let cats: Vec<_> = report.categories.iter().map(|c| c.category).collect();
    assert!(cats.contains(&CheckCategory::Workspace));
    assert!(cats.contains(&CheckCategory::LockState));
    assert!(cats.contains(&CheckCategory::SubprocessLiveness));
    assert!(cats.contains(&CheckCategory::StorageIntegrity));
    assert!(cats.contains(&CheckCategory::ConfigValidation));
}

#[test]
fn doctor_healthy_project_is_healthy() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let config = DoctorConfig {
        project_dir: dir.clone(),
    };
    let report = vo_cli::run_doctor(&config).unwrap();
    assert!(report.is_healthy());
}
