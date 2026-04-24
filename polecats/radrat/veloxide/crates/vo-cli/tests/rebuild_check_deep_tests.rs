use std::path::PathBuf;
use vo_cli::commands::check::*;
use vo_cli::commands::rebuild::*;

#[test]
fn rebuild_status_equality_same() {
    let s1 = RebuildStatus::Completed;
    let s2 = RebuildStatus::Completed;
    assert_eq!(s1, s2);
}

#[test]
fn rebuild_status_inequality_different() {
    let s1 = RebuildStatus::Completed;
    let s2 = RebuildStatus::Failed {
        reason: "oops".into(),
    };
    assert_ne!(s1, s2);
}

#[test]
fn rebuild_report_equality_same() {
    let r1 = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Completed,
        events_applied: 100,
        duration_ms: 50,
    };
    let r2 = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Completed,
        events_applied: 100,
        duration_ms: 50,
    };
    assert_eq!(r1, r2);
}

#[test]
fn rebuild_report_inequality_different_events() {
    let r1 = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Completed,
        events_applied: 100,
        duration_ms: 50,
    };
    let r2 = RebuildReport {
        projection_id: Some("p1".into()),
        rebuild_id: Some("r1".into()),
        status: RebuildStatus::Completed,
        events_applied: 200,
        duration_ms: 50,
    };
    assert_ne!(r1, r2);
}

#[test]
fn rebuild_config_equality() {
    let c1 = RebuildConfig {
        project_dir: PathBuf::from("/tmp"),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let c2 = RebuildConfig {
        project_dir: PathBuf::from("/tmp"),
        projection_id: Some("p1".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    assert_eq!(c1, c2);
}

#[test]
fn rebuild_config_clone() {
    let c = RebuildConfig {
        project_dir: PathBuf::from("/tmp"),
        projection_id: Some("p1".into()),
        list_projections: true,
        force: true,
        schema_version: Some(2),
    };
    let cloned = c.clone();
    assert_eq!(c, cloned);
}

#[test]
fn rebuild_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let rebuild_err: RebuildError = io_err.into();
    match rebuild_err {
        RebuildError::Io { path, reason, .. } => {
            assert!(reason.contains("file missing"));
            assert!(path.as_os_str().is_empty());
        }
        _ => panic!("expected Io variant"),
    }
}

#[test]
fn rebuild_format_progress_started() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("42"));
}

#[test]
fn rebuild_format_progress_in_progress() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::InProgress {
            progress_percent: 75,
            at_sequence: 7500,
        },
        events_applied: 7500,
        duration_ms: 100,
    };
    let output = report.format_progress();
    assert!(output.contains("75%"));
    assert!(output.contains("7500"));
}

#[test]
fn rebuild_format_progress_failed() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::Failed {
            reason: "disk full".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("failed"));
    assert!(output.contains("disk full"));
}

#[test]
fn rebuild_format_progress_noop() {
    let report = RebuildReport {
        projection_id: Some("p".into()),
        rebuild_id: Some("r".into()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("skipped"));
    assert!(output.contains("already up to date"));
}

#[test]
fn rebuild_format_progress_listed_empty() {
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
fn rebuild_format_progress_listed_with_items() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["proj-a".into(), "proj-b".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("proj-a"));
    assert!(output.contains("proj-b"));
}

#[test]
fn rebuild_run_not_initialized() {
    let config = RebuildConfig {
        project_dir: PathBuf::from("/nonexistent/path"),
        projection_id: Some("p".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_run_list_mode() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

#[test]
fn rebuild_run_with_projection_id() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.path().to_path_buf(),
        projection_id: Some("my-projection".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert_eq!(report.projection_id.as_deref(), Some("my-projection"));
    assert_eq!(report.status, RebuildStatus::Completed);
}

#[test]
fn rebuild_run_without_projection_id_and_not_list() {
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
fn check_error_file_not_found_mentions_path() {
    let err = CheckError::FileNotFound {
        path: PathBuf::from("/missing/file"),
    };
    assert!(err.to_string().contains("/missing/file"));
}

#[test]
fn check_error_not_regular_file_mentions_path() {
    let err = CheckError::NotRegularFile {
        path: PathBuf::from("/a/dir"),
    };
    assert!(err.to_string().contains("/a/dir"));
}

#[test]
fn check_error_file_too_small_mentions_path() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    assert!(err.to_string().contains("/tiny"));
    assert!(err.to_string().contains("4 bytes"));
}

#[test]
fn check_error_invalid_magic_shows_bytes() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/bad"),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = err.to_string();
    assert!(msg.contains("0xdeadbeef") || msg.contains("0xde") || msg.contains("/bad"));
}

#[test]
fn check_error_permission_denied_mentions_path() {
    let err = CheckError::PermissionDenied {
        path: PathBuf::from("/root/secret"),
    };
    assert!(err.to_string().contains("/root/secret"));
}

#[test]
fn validate_binary_header_elf() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.elf");
    let mut data = vec![0x7F, 0x45, 0x4C, 0x46];
    data.extend_from_slice(b"padding");
    std::fs::write(&path, &data).unwrap();
    let fmt = validate_binary_header(&path).unwrap();
    assert_eq!(fmt, BinaryFormat::Elf);
}

#[test]
fn validate_binary_header_macho_64_le() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.macho");
    let mut data = vec![0xCF, 0xFA, 0xED, 0xFE];
    data.extend_from_slice(b"padding");
    std::fs::write(&path, &data).unwrap();
    let fmt = validate_binary_header(&path).unwrap();
    assert_eq!(fmt, BinaryFormat::MachO64LittleEndian);
}

#[test]
fn validate_binary_header_macho_64_be() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.macho");
    let mut data = vec![0xFE, 0xED, 0xFA, 0xCF];
    data.extend_from_slice(b"padding");
    std::fs::write(&path, &data).unwrap();
    let fmt = validate_binary_header(&path).unwrap();
    assert_eq!(fmt, BinaryFormat::MachO64BigEndian);
}

#[test]
fn validate_binary_header_macho_32_le() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.macho");
    let mut data = vec![0xCE, 0xFA, 0xED, 0xFE];
    data.extend_from_slice(b"padding");
    std::fs::write(&path, &data).unwrap();
    let fmt = validate_binary_header(&path).unwrap();
    assert_eq!(fmt, BinaryFormat::MachO32LittleEndian);
}

#[test]
fn validate_binary_header_macho_32_be() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.macho");
    let mut data = vec![0xFE, 0xED, 0xFA, 0xCE];
    data.extend_from_slice(b"padding");
    std::fs::write(&path, &data).unwrap();
    let fmt = validate_binary_header(&path).unwrap();
    assert_eq!(fmt, BinaryFormat::MachO32BigEndian);
}

#[test]
fn validate_binary_header_directory() {
    let dir = tempfile::tempdir().unwrap();
    let result = validate_binary_header(dir.path());
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn validate_binary_header_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, b"\x7FELFpadding").unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn validate_binary_header_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty");
    std::fs::write(&path, b"").unwrap();
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn validate_binary_header_3_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny");
    std::fs::write(&path, b"\x7F\x45\x4C").unwrap();
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn validate_binary_header_nonexistent() {
    let result = validate_binary_header(PathBuf::from("/no/such/file").as_path());
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn validate_binary_header_invalid_magic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid");
    std::fs::write(&path, b"ABCDextra").unwrap();
    let result = validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn run_check_valid_elf() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.elf");
    let mut data = vec![0x7F, 0x45, 0x4C, 0x46];
    data.extend_from_slice(b"padding");
    std::fs::write(&path, &data).unwrap();
    let result = run_check(&path);
    assert!(result.is_ok());
}

#[test]
fn run_check_nonexistent() {
    let result = run_check(PathBuf::from("/nonexistent").as_path());
    assert!(result.is_err());
}

#[test]
fn binary_format_display_name_all() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64BigEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64LittleEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
}

#[test]
fn check_constants_correct() {
    assert_eq!(ELF_MAGIC, [0x7F, 0x45, 0x4C, 0x46]);
    assert_eq!(MACHO_MAGIC_32_BE, [0xFE, 0xED, 0xFA, 0xCE]);
    assert_eq!(MACHO_MAGIC_32_LE, [0xCE, 0xFA, 0xED, 0xFE]);
    assert_eq!(MACHO_MAGIC_64_BE, [0xFE, 0xED, 0xFA, 0xCF]);
    assert_eq!(MACHO_MAGIC_64_LE, [0xCF, 0xFA, 0xED, 0xFE]);
    assert_eq!(KNOWN_MAGICS.len(), 5);
}

#[test]
fn check_error_partial_eq_io_never_equal() {
    let e1 = CheckError::Io {
        path: PathBuf::from("/a"),
        source: std::io::Error::new(std::io::ErrorKind::Other, "x"),
    };
    let e2 = CheckError::Io {
        path: PathBuf::from("/a"),
        source: std::io::Error::new(std::io::ErrorKind::Other, "x"),
    };
    assert_ne!(e1, e2);
}
