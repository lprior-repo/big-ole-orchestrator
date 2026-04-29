#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::{validate_binary_header, BinaryFormat, CheckError};

#[test]
fn check_valid_elf_binary() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.elf");
    std::fs::write(&bin_path, [0x7F, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::Elf);
}

#[test]
fn check_valid_macho_64_le() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xCF, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64LittleEndian);
}

#[test]
fn check_valid_macho_64_be() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCF, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64BigEndian);
}

#[test]
fn check_valid_macho_32_le() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xCE, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO32LittleEndian);
}

#[test]
fn check_valid_macho_32_be() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test.macho");
    std::fs::write(&bin_path, [0xFE, 0xED, 0xFA, 0xCE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO32BigEndian);
}

#[test]
fn check_file_too_small_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("tiny.bin");
    std::fs::write(&bin_path, [0x7F, 0x45]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn check_invalid_magic_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("bad.bin");
    std::fs::write(&bin_path, [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = validate_binary_header(&bin_path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn check_nonexistent_file_returns_not_found() {
    let result = validate_binary_header(PathBuf::from("/tmp/does-not-exist-co5-test").as_path());
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn check_symlink_returns_not_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, [0x7F, 0x45, 0x4C, 0x46]).unwrap();
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn check_directory_returns_not_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let result = validate_binary_header(dir.path());
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}
