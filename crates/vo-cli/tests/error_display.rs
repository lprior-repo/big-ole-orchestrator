use std::path::PathBuf;
use vo_cli::{CheckError, parse_strict_numeric};

#[test]
fn check_error_file_not_found_display() {
    let err = CheckError::FileNotFound {
        path: PathBuf::from("/missing/file"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/missing/file"));
    assert!(msg.contains("not found"));
}

#[test]
fn check_error_not_regular_file_display() {
    let err = CheckError::NotRegularFile {
        path: PathBuf::from("/dev/null"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/dev/null"));
    assert!(msg.contains("not a regular file"));
}

#[test]
fn check_error_file_too_small_display() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tiny"));
    assert!(msg.contains("too small"));
    assert!(msg.contains("4 bytes"));
}

#[test]
fn check_error_invalid_magic_display() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/bad/bin"),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = err.to_string();
    assert!(msg.contains("/bad/bin"));
    assert!(msg.contains("0xde"));
    assert!(msg.contains("0xbe"));
}

#[test]
fn check_error_permission_denied_display() {
    let err = CheckError::PermissionDenied {
        path: PathBuf::from("/secret/bin"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/secret/bin"));
    assert!(msg.contains("permission"));
}

#[test]
fn parse_strict_negative_rejected() {
    let err = parse_strict_numeric("-5").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("negative"));
}

#[test]
fn parse_strict_empty_rejected() {
    let err = parse_strict_numeric("").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("empty"));
}

#[test]
fn parse_strict_plus_rejected() {
    let err = parse_strict_numeric("+42").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("plus"));
}

#[test]
fn parse_strict_non_digits_rejected() {
    let err = parse_strict_numeric("abc").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid"));
}

#[test]
fn parse_strict_max_u64_accepted() {
    let result = parse_strict_numeric("18446744073709551615");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), u64::MAX);
}

#[test]
fn parse_strict_overflow_rejected() {
    let err = parse_strict_numeric("18446744073709551616").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("overflow"));
}
