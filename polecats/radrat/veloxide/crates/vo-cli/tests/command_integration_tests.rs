use std::path::PathBuf;
use vo_cli::commands::check::{
    validate_binary_header, BinaryFormat, CheckError, ELF_MAGIC, KNOWN_MAGICS, MACHO_MAGIC_32_BE,
    MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE, MACHO_MAGIC_64_LE,
};
use vo_cli::commands::doctor::{run_doctor, DoctorConfig, DoctorError};
use vo_cli::commands::init::{
    run_init, InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME, WORKFLOWS_DIR_NAME,
};
use vo_cli::commands::lock::{run_lock, LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{
    run_rebuild, RebuildConfig, RebuildError, RebuildReport, RebuildStatus,
};

#[test]
fn elf_magic_constant_values() {
    assert_eq!(ELF_MAGIC, [0x7F, 0x45, 0x4C, 0x46]);
}

#[test]
fn macho_magic_constants_are_unique() {
    let magics = [
        MACHO_MAGIC_32_BE,
        MACHO_MAGIC_32_LE,
        MACHO_MAGIC_64_BE,
        MACHO_MAGIC_64_LE,
    ];
    for i in 0..magics.len() {
        for j in (i + 1)..magics.len() {
            assert_ne!(magics[i], magics[j], "Mach-O magics must be unique");
        }
    }
}

#[test]
fn known_magics_has_five_entries() {
    assert_eq!(KNOWN_MAGICS.len(), 5);
}

#[test]
fn binary_format_display_name_elf() {
    assert!(BinaryFormat::Elf.display_name().contains("ELF"));
}

#[test]
fn binary_format_display_name_macho_32() {
    let name = BinaryFormat::MachO32BigEndian.display_name();
    assert!(name.contains("Mach-O"));
    assert!(name.contains("32"));
}

#[test]
fn binary_format_display_name_macho_64() {
    let name = BinaryFormat::MachO64LittleEndian.display_name();
    assert!(name.contains("Mach-O"));
    assert!(name.contains("64"));
}

#[test]
fn validate_binary_header_elf_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.elf");
    let mut content = vec![0x7F, 0x45, 0x4C, 0x46];
    content.extend_from_slice(b"padding to make it bigger than 4 bytes");
    std::fs::write(&path, &content).unwrap();
    let result = validate_binary_header(&path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::Elf);
}

#[test]
fn validate_binary_header_macho_64_le() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.macho");
    let mut content = vec![0xCF, 0xFA, 0xED, 0xFE];
    content.extend_from_slice(b"extra bytes here");
    std::fs::write(&path, &content).unwrap();
    let result = validate_binary_header(&path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64LittleEndian);
}

#[test]
fn validate_binary_header_too_small_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny.bin");
    std::fs::write(&path, b"\x7F").unwrap();
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn validate_binary_header_invalid_magic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.bin");
    let mut content = vec![0xDE, 0xAD, 0xBE, 0xEF];
    content.extend_from_slice(b"more data");
    std::fs::write(&path, &content).unwrap();
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn validate_binary_header_symlink_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.bin");
    std::fs::write(&target, b"\x7FELFpadding").unwrap();
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn validate_binary_header_not_found() {
    let result = validate_binary_header(PathBuf::from("/tmp/no-such-file-xyz").as_path());
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn init_creates_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = run_init(&config);
    assert!(result.is_ok());
    let vo_dir = result.unwrap();
    assert!(vo_dir.is_dir());
    assert!(dir.path().join(WORKFLOWS_DIR_NAME).exists() || vo_dir.join("workflows").is_dir());
}

#[test]
fn init_creates_config_toml() {
    let dir = tempfile::tempdir().unwrap();
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config).unwrap();
    let config_path = dir.path().join(CONFIG_FILE_NAME);
    assert!(config_path.exists());
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[engine]"));
    assert!(content.contains("http://localhost:3000"));
    assert!(content.contains("[storage]"));
}

#[test]
fn init_rejects_nonexistent_dir() {
    let config = InitConfig {
        project_dir: PathBuf::from("/tmp/vo-cli-init-no-such-dir-xyz"),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = run_init(&config);
    assert!(matches!(result, Err(InitError::DirNotFound { .. })));
}

#[test]
fn init_rejects_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("real");
    std::fs::create_dir_all(&target).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let config = InitConfig {
        project_dir: link,
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = run_init(&config);
    assert!(matches!(result, Err(InitError::SymlinkTarget { .. })));
}

#[test]
fn init_idempotent_same_config() {
    let dir = tempfile::tempdir().unwrap();
    let config = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let first = run_init(&config).unwrap();
    let second = run_init(&config).unwrap();
    assert_eq!(first, second);
}

#[test]
fn init_already_initialized_different_config() {
    let dir = tempfile::tempdir().unwrap();
    let config1 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://localhost:3000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    run_init(&config1).unwrap();
    let config2 = InitConfig {
        project_dir: dir.path().to_path_buf(),
        engine_url: "http://different:4000".to_string(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn lock_requires_initialized_project() {
    let dir = tempfile::tempdir().unwrap();
    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_lock(&config);
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_requires_workflows_with_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo").join("workflows")).unwrap();
    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_lock(&config);
    assert!(matches!(result, Err(LockError::Empty { .. })));
}

#[test]
fn lock_creates_lockfile_with_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let wf_dir = dir.path().join(".vo").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    std::fs::write(wf_dir.join("workflow-a"), b"binary content A").unwrap();
    std::fs::write(wf_dir.join("workflow-b"), b"binary content B").unwrap();
    let config = LockConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let lockmap = run_lock(&config).unwrap();
    assert_eq!(lockmap.len(), 2);
    assert!(lockmap.contains_key("workflow-a"));
    assert!(lockmap.contains_key("workflow-b"));
    let lock_content = std::fs::read_to_string(dir.path().join(LOCK_FILE_NAME)).unwrap();
    assert!(lock_content.contains("workflow-a"));
    assert!(lock_content.contains("workflow-b"));
}

#[test]
fn rebuild_requires_initialized_project() {
    let dir = tempfile::tempdir().unwrap();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_returns_empty_projections() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let report = run_rebuild(&config).unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(ref v) if v.is_empty()));
}

#[test]
fn rebuild_with_projection_id_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("proj-123".to_string()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let report = run_rebuild(&config).unwrap();
    assert!(matches!(report.status, RebuildStatus::Completed));
    assert_eq!(report.projection_id.as_deref(), Some("proj-123"));
}

#[test]
fn rebuild_without_projection_id_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(result.is_err());
}

#[test]
fn doctor_requires_initialized_project() {
    let dir = tempfile::tempdir().unwrap();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let result = run_doctor(&config);
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

#[test]
fn doctor_with_initialized_project_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let vo_dir = dir.path().join(".vo");
    std::fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    std::fs::create_dir_all(vo_dir.join("storage")).unwrap();
    std::fs::write(
        dir.path().join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let config = DoctorConfig {
        project_dir: dir.path().to_path_buf(),
    };
    let report = run_doctor(&config).unwrap();
    assert_eq!(report.project_dir, dir.path().to_path_buf());
    assert_eq!(report.categories.len(), 5);
}
