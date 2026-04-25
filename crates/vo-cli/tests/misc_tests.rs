#![allow(clippy::redundant_pattern_matching)]
use std::fs;
use std::path::{Path, PathBuf};

use vo_cli::commands::doctor_checks::{CheckCategory, Severity};
use vo_cli::commands::gc::GcConfig;
use vo_cli::commands::init::InitConfig;
use vo_cli::utils::{file_hash, sha256_hex};
use vo_cli::{parse_strict_numeric, CliError, Command};

#[test]
fn sha256_hex_pads_to_64_chars() {
    let result = sha256_hex("test");
    assert_eq!(result.len(), 64);
}

#[test]
fn sha256_hex_empty_input() {
    let result = sha256_hex("");
    assert_eq!(result.len(), 64);
    assert!(result.chars().all(|c| c == '0'));
}

#[test]
fn file_hash_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hash_test");
    fs::write(&path, b"hello world").unwrap();

    let h1 = file_hash(&path).unwrap();
    let h2 = file_hash(&path).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn file_hash_different_content() {
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a");
    let p2 = dir.path().join("b");
    fs::write(&p1, b"content a").unwrap();
    fs::write(&p2, b"content b").unwrap();

    let h1 = file_hash(&p1).unwrap();
    let h2 = file_hash(&p2).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn file_hash_nonexistent_file() {
    let result = file_hash(Path::new("/tmp/nonexistent-file-hash-test"));
    assert!(result.is_err());
}

#[test]
fn command_equality() {
    let c1 = Command::Check {
        workflow: false,
        path: PathBuf::from("/tmp"),
    };
    let c2 = Command::Check {
        workflow: false,
        path: PathBuf::from("/tmp"),
    };
    let c3 = Command::Check {
        workflow: false,
        path: PathBuf::from("/other"),
    };
    assert_eq!(c1, c2);
    assert_ne!(c1, c3);
}

#[test]
fn command_clone() {
    let c = Command::Purge {
        instance: "test".to_string(),
    };
    let cloned = c.clone();
    assert_eq!(c, cloned);
}

#[test]
fn severity_ord_total() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Error > Severity::Info);
    assert!(Severity::Info <= Severity::Info);
    assert!(Severity::Warn >= Severity::Warn);
}

#[test]
fn check_category_display_all() {
    assert_eq!(CheckCategory::Workspace.to_string(), "workspace");
    assert_eq!(CheckCategory::LockState.to_string(), "lock-state");
    assert_eq!(
        CheckCategory::SubprocessLiveness.to_string(),
        "subprocess-liveness"
    );
    assert_eq!(
        CheckCategory::StorageIntegrity.to_string(),
        "storage-integrity"
    );
    assert_eq!(
        CheckCategory::ConfigValidation.to_string(),
        "config-validation"
    );
}

#[test]
fn parse_strict_numeric_valid_numbers() {
    assert_eq!(parse_strict_numeric("0").unwrap(), 0);
    assert_eq!(parse_strict_numeric("1").unwrap(), 1);
    assert_eq!(parse_strict_numeric("999999").unwrap(), 999999);
    assert_eq!(
        parse_strict_numeric("18446744073709551615").unwrap(),
        u64::MAX
    );
}

#[test]
fn parse_strict_numeric_negative() {
    let result = parse_strict_numeric("-1");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("negative")));
}

#[test]
fn parse_strict_numeric_leading_plus() {
    let result = parse_strict_numeric("+42");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("plus")));
}

#[test]
fn parse_strict_numeric_empty() {
    let result = parse_strict_numeric("");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("empty")));
}

#[test]
fn parse_strict_numeric_letters() {
    let result = parse_strict_numeric("abc");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("invalid")));
}

#[test]
fn parse_strict_numeric_overflow() {
    let result = parse_strict_numeric("18446744073709551616");
    assert!(matches!(result, Err(CliError::InvalidNumeric(msg)) if msg.contains("overflow")));
}

#[test]
fn init_config_default() {
    let config = InitConfig::default();
    assert_eq!(config.project_dir, PathBuf::from("."));
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.storage_path, PathBuf::from(".vo/storage"));
}

#[test]
fn gc_config_default() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}
