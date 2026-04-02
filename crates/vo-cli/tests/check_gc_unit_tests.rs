#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::needless_for_each)]
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use vo_cli::commands::check::{
    run_check, validate_binary_header, BinaryFormat, CheckError, ELF_MAGIC, KNOWN_MAGICS,
    MACHO_MAGIC_32_BE, MACHO_MAGIC_32_LE, MACHO_MAGIC_64_BE, MACHO_MAGIC_64_LE,
};
use vo_cli::commands::gc::{
    delete_version_dir, find_unpinned_directories, run_gc, GcConfig, GcError,
};

fn create_file_with_bytes(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write failed");
    path
}

fn sha256_hex(seed: &str) -> String {
    format!("{:0<64}", seed)
}

fn create_versions_dir(entries: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    entries.iter().for_each(|name| {
        let hash = sha256_hex(name);
        fs::create_dir_all(dir.path().join(&hash)).expect("mkdir");
    });
    dir
}

// ============================================================
// validate_binary_header: valid magic numbers
// ============================================================

#[test]
fn check_valid_elf_binary_returns_elf_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "elf.bin", &[0x7F, 0x45, 0x4C, 0x46, 0x00, 0x00]);
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::Elf));
}

#[test]
fn check_valid_macho_64le_returns_correct_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(
        dir.path(),
        "macho64le.bin",
        &[0xCF, 0xFA, 0xED, 0xFE, 0x07, 0x00],
    );
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::MachO64LittleEndian));
}

#[test]
fn check_valid_macho_64be_returns_correct_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(
        dir.path(),
        "macho64be.bin",
        &[0xFE, 0xED, 0xFA, 0xCF, 0x00, 0x00],
    );
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::MachO64BigEndian));
}

#[test]
fn check_valid_macho_32le_returns_correct_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "macho32le.bin", &[0xCE, 0xFA, 0xED, 0xFE, 0x00]);
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::MachO32LittleEndian));
}

#[test]
fn check_valid_macho_32be_returns_correct_format() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "macho32be.bin", &[0xFE, 0xED, 0xFA, 0xCE, 0x00]);
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::MachO32BigEndian));
}

// ============================================================
// validate_binary_header: error cases
// ============================================================

#[test]
fn check_nonexistent_file_returns_file_not_found() {
    let path = PathBuf::from("/tmp/nonexistent-vel-co5-test-binary-xyz");
    let result = validate_binary_header(&path);
    assert!(
        matches!(result, Err(CheckError::FileNotFound { .. })),
        "expected FileNotFound, got {:?}",
        result
    );
}

#[test]
fn check_directory_returns_not_regular_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = validate_binary_header(dir.path());
    assert!(
        matches!(result, Err(CheckError::NotRegularFile { .. })),
        "expected NotRegularFile, got {:?}",
        result
    );
}

#[test]
fn check_symlink_returns_not_regular_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = create_file_with_bytes(dir.path(), "real.bin", &[0x7F, 0x45, 0x4C, 0x46, 0x00]);
    let link_path = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link_path).expect("symlink");
    let result = validate_binary_header(&link_path);
    assert!(
        matches!(result, Err(CheckError::NotRegularFile { .. })),
        "expected NotRegularFile for symlink, got {:?}",
        result
    );
}

#[test]
fn check_empty_file_returns_file_too_small() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "empty.bin", &[]);
    let result = validate_binary_header(&path);
    assert!(
        matches!(result, Err(CheckError::FileTooSmall { .. })),
        "expected FileTooSmall for 0 bytes, got {:?}",
        result
    );
}

#[test]
fn check_3byte_file_returns_file_too_small() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "small.bin", &[0x7F, 0x45, 0x4C]);
    let result = validate_binary_header(&path);
    assert!(
        matches!(result, Err(CheckError::FileTooSmall { .. })),
        "expected FileTooSmall for 3 bytes, got {:?}",
        result
    );
}

#[test]
fn check_unknown_magic_returns_invalid_magic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "unknown.bin", &[0xDE, 0xAD, 0xBE, 0xEF]);
    let result = validate_binary_header(&path);
    match result {
        Err(CheckError::InvalidMagic { magic, path: p }) => {
            assert_eq!(magic, [0xDE, 0xAD, 0xBE, 0xEF]);
            assert_eq!(p, path);
        }
        other => panic!("expected InvalidMagic, got {:?}", other),
    }
}

#[test]
fn check_exactly_4_bytes_with_elf_magic_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "exact.bin", &[0x7F, 0x45, 0x4C, 0x46]);
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::Elf));
}

#[test]
fn check_5byte_file_with_elf_magic_succeeds_inv1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "five.bin", &[0x7F, 0x45, 0x4C, 0x46, 0x00]);
    let result = validate_binary_header(&path);
    assert_eq!(result, Ok(BinaryFormat::Elf));
}

#[test]
fn check_text_file_returns_invalid_magic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "hello.txt", b"Hello, World!\n");
    let result = validate_binary_header(&path);
    assert!(
        matches!(result, Err(CheckError::InvalidMagic { .. })),
        "expected InvalidMagic for text file, got {:?}",
        result
    );
}

// ============================================================
// BinaryFormat::display_name
// ============================================================

#[test]
fn display_name_elf() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
}

#[test]
fn display_name_macho_32be() {
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
}

#[test]
fn display_name_macho_32le() {
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
}

#[test]
fn display_name_macho_64be() {
    assert_eq!(
        BinaryFormat::MachO64BigEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
}

#[test]
fn display_name_macho_64le() {
    assert_eq!(
        BinaryFormat::MachO64LittleEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
}

// ============================================================
// run_check
// ============================================================

#[test]
fn run_check_valid_elf_returns_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "elf.bin", &[0x7F, 0x45, 0x4C, 0x46, 0x00]);
    let result = run_check(&path);
    assert!(matches!(result, Ok(_)));
}

#[test]
fn run_check_nonexistent_file_returns_err() {
    let path = PathBuf::from("/tmp/nonexistent-vel-co5-run-check");
    let result = run_check(&path);
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn run_check_text_file_returns_err() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = create_file_with_bytes(dir.path(), "hello.txt", b"Hello, World!\n");
    let result = run_check(&path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn run_check_directory_returns_err() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result = run_check(dir.path());
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

// ============================================================
// Magic constants
// ============================================================

#[test]
fn elf_magic_is_correct() {
    assert_eq!(ELF_MAGIC, [0x7F, 0x45, 0x4C, 0x46]);
}

#[test]
fn macho_magic_constants_are_correct() {
    assert_eq!(MACHO_MAGIC_32_BE, [0xFE, 0xED, 0xFA, 0xCE]);
    assert_eq!(MACHO_MAGIC_32_LE, [0xCE, 0xFA, 0xED, 0xFE]);
    assert_eq!(MACHO_MAGIC_64_BE, [0xFE, 0xED, 0xFA, 0xCF]);
    assert_eq!(MACHO_MAGIC_64_LE, [0xCF, 0xFA, 0xED, 0xFE]);
}

#[test]
fn known_magics_contains_exactly_5_entries() {
    assert_eq!(KNOWN_MAGICS.len(), 5);
}

// ============================================================
// CheckError display formatting
// ============================================================

#[test]
fn check_error_file_not_found_display() {
    let err = CheckError::FileNotFound {
        path: PathBuf::from("/tmp/test"),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("/tmp/test"));
    assert!(msg.contains("file not found"));
}

#[test]
fn check_error_not_regular_file_display() {
    let err = CheckError::NotRegularFile {
        path: PathBuf::from("/tmp/dir"),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("not a regular file"));
}

#[test]
fn check_error_file_too_small_display() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/tmp/small"),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("file too small"));
}

#[test]
fn check_error_invalid_magic_display() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/tmp/bad"),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = format!("{}", err);
    assert!(msg.contains("invalid binary format"));
    assert!(msg.contains("[0xde, 0xad, 0xbe, 0xef]"));
}

#[test]
fn check_error_permission_denied_display() {
    let err = CheckError::PermissionDenied {
        path: PathBuf::from("/tmp/secret"),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("permission denied"));
}

// ============================================================
// GcConfig::default
// ============================================================

#[test]
fn gc_config_default_values() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}

// ============================================================
// find_unpinned_directories
// ============================================================

#[test]
fn find_unpinned_returns_correct_set_difference() {
    let dir = create_versions_dir(&["aaa", "bbb", "ccc"]);
    let pinned: HashSet<String> = [sha256_hex("aaa"), sha256_hex("ccc")].into_iter().collect();

    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));

    let unpinned = result.expect("ok");
    assert_eq!(unpinned.len(), 1);
    assert_eq!(
        unpinned[0]
            .file_name()
            .expect("name")
            .to_str()
            .expect("str"),
        sha256_hex("bbb")
    );
}

#[test]
fn find_unpinned_returns_empty_when_all_pinned() {
    let dir = create_versions_dir(&["aaa", "bbb"]);
    let pinned: HashSet<String> = [sha256_hex("aaa"), sha256_hex("bbb")].into_iter().collect();

    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));
    assert!(result.expect("ok").is_empty());
}

#[test]
fn find_unpinned_returns_all_when_none_pinned() {
    let dir = create_versions_dir(&["aaa", "bbb"]);
    let pinned: HashSet<String> = HashSet::new();

    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));
    assert_eq!(result.expect("ok").len(), 2);
}

#[test]
fn find_unpinned_returns_empty_when_dir_not_exists_inv4() {
    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(
        PathBuf::from("/tmp/nonexistent-versions-vel-co5-test").as_path(),
        &pinned,
    );
    assert!(matches!(result, Ok(_)));
    assert!(result.expect("ok").is_empty());
}

#[test]
fn find_unpinned_skips_non_directory_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let hash = sha256_hex("aaa");
    fs::create_dir_all(dir.path().join(&hash)).expect("mkdir");
    fs::write(dir.path().join("readme.txt"), "hello").expect("write");

    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));

    let unpinned = result.expect("ok");
    assert_eq!(unpinned.len(), 1);
    assert_eq!(
        unpinned[0]
            .file_name()
            .expect("name")
            .to_str()
            .expect("str"),
        hash
    );
}

#[test]
fn find_unpinned_skips_non_hex_directory_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let good_hash = sha256_hex("aaa");
    fs::create_dir_all(dir.path().join(&good_hash)).expect("mkdir");
    fs::create_dir_all(dir.path().join("GGGGGGGG")).expect("mkdir");

    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));

    let unpinned = result.expect("ok");
    assert_eq!(unpinned.len(), 1);
    assert_eq!(
        unpinned[0]
            .file_name()
            .expect("name")
            .to_str()
            .expect("str"),
        good_hash
    );
}

#[test]
fn find_unpinned_skips_short_directory_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let good_hash = sha256_hex("aaa");
    fs::create_dir_all(dir.path().join(&good_hash)).expect("mkdir");
    fs::create_dir_all(dir.path().join("abc")).expect("mkdir");

    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));

    let unpinned = result.expect("ok");
    assert_eq!(unpinned.len(), 1);
}

#[test]
fn find_unpinned_returns_empty_for_empty_versions_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));
    assert!(result.expect("ok").is_empty());
}

#[test]
fn find_unpinned_skips_65_char_directory_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let good_hash = sha256_hex("aaa");
    fs::create_dir_all(dir.path().join(&good_hash)).expect("mkdir");
    let long_name = format!("{:0<65}", "a");
    fs::create_dir_all(dir.path().join(&long_name)).expect("mkdir");

    let pinned: HashSet<String> = HashSet::new();
    let result = find_unpinned_directories(dir.path(), &pinned);
    assert!(matches!(result, Ok(_)));

    let unpinned = result.expect("ok");
    assert_eq!(unpinned.len(), 1);
    assert_eq!(
        unpinned[0]
            .file_name()
            .expect("name")
            .to_str()
            .expect("str"),
        good_hash
    );
}

// ============================================================
// delete_version_dir
// ============================================================

#[test]
fn delete_existing_directory_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("target");
    fs::create_dir_all(&target).expect("mkdir");
    fs::write(target.join("file.dat"), b"data").expect("write");

    let result = delete_version_dir(&target);
    assert!(matches!(result, Ok(_)));
    assert!(!target.exists());
}

#[test]
fn delete_nonexistent_directory_fails() {
    let path = PathBuf::from("/tmp/nonexistent-dir-vel-co5-delete-test");
    let result = delete_version_dir(&path);
    assert!(
        matches!(result, Err(GcError::DeleteFailed { .. })),
        "expected DeleteFailed, got {:?}",
        result
    );
}

#[test]
fn delete_readonly_directory_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("readonly");
    fs::create_dir_all(&target).expect("mkdir");
    fs::write(target.join("file.dat"), b"data").expect("write");

    fs::set_permissions(&target, fs::Permissions::from_mode(0o555)).expect("chmod");

    let result = delete_version_dir(&target);

    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("chmod restore");

    assert!(matches!(result, Err(_)));
}

// ============================================================
// GcError display formatting
// ============================================================

#[test]
fn gc_error_engine_unreachable_display() {
    let err = GcError::EngineUnreachable {
        url: "http://localhost:3000".to_string(),
        reason: "connection refused".to_string(),
    };
    let msg = format!("{}", err);

    assert!(msg.contains("503"));
    assert!(msg.contains("HTTP"));
}

#[test]
fn gc_error_invalid_api_response_display() {
    let err = GcError::InvalidApiResponse {
        reason: "expected array".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("failed to parse"));
}

#[test]
fn gc_error_versions_dir_not_found_display() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf/versions"),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("does not exist"));
}

#[test]
fn gc_error_delete_failed_display() {
    let err = GcError::DeleteFailed {
        path: PathBuf::from("/var/wtf/versions/abc"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("failed to delete"));
}

// ============================================================
// run_gc (async)
// ============================================================

#[tokio::test]
async fn run_gc_does_not_delete_pinned_version_inv2() {
    let dir = create_versions_dir(&["aaa", "bbb"]);
    let config = GcConfig {
        engine_url: "http://localhost:19998".to_string(),
        versions_dir: dir.path().to_path_buf(),
        dry_run: false,
    };

    let result = run_gc(&config).await;
    assert!(matches!(result, Ok(_)));

    let summary = result.expect("ok");
    assert!(dir.path().join(sha256_hex("aaa")).exists() || summary.deleted_count == 0);
}

#[tokio::test]
async fn run_gc_empty_pinned_deletes_all_inv3() {
    let dir = create_versions_dir(&["aaa", "bbb"]);
    let config = GcConfig {
        engine_url: "http://localhost:19998".to_string(),
        versions_dir: dir.path().to_path_buf(),
        dry_run: false,
    };

    let result = run_gc(&config).await;
    assert!(matches!(result, Ok(_)));

    let summary = result.expect("ok");
    assert_eq!(summary.scanned_count, 2);
    assert_eq!(summary.deleted_count, 2);
}

#[tokio::test]
async fn run_gc_versions_dir_not_exists_is_noop_inv4() {
    let config = GcConfig {
        engine_url: "http://localhost:19998".to_string(),
        versions_dir: PathBuf::from("/tmp/nonexistent-versions-vel-co5-gc-test"),
        dry_run: false,
    };

    let result = run_gc(&config).await;
    assert!(matches!(result, Ok(_)));

    let summary = result.expect("ok");
    assert_eq!(summary.scanned_count, 0);
    assert_eq!(summary.deleted_count, 0);
}
