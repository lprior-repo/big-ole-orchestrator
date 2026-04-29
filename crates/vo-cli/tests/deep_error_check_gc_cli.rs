#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::{map_error_to_exit_code, parse_strict_numeric, CheckError, CliError, GcError};

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
fn gc_error_engine_unreachable_display() {
    let err = GcError::EngineUnreachable {
        url: "http://engine:3000".to_string(),
        reason: "connection refused".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("http://engine:3000"));
    assert!(msg.contains("connection refused"));
    assert!(msg.contains("503"));
}

#[test]
fn gc_error_engine_http_error_display() {
    let err = GcError::EngineHttpError {
        url: "http://engine:3000/api".to_string(),
        status: 500,
    };
    let msg = err.to_string();
    assert!(msg.contains("500"));
    assert!(msg.contains("http://engine:3000"));
}

#[test]
fn gc_error_invalid_api_response_display() {
    let err = GcError::InvalidApiResponse {
        reason: "missing field".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("missing field"));
    assert!(msg.contains("parse"));
}

#[test]
fn gc_error_versions_dir_not_found_display() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn cli_error_dispatch_display() {
    let err = CliError::Dispatch("unknown command".to_string());
    let msg = err.to_string();
    assert!(msg.contains("dispatch"));
    assert!(msg.contains("unknown command"));
}

#[test]
fn cli_error_invalid_numeric_display() {
    let err = CliError::InvalidNumeric("bad input".to_string());
    let msg = err.to_string();
    assert!(msg.contains("invalid numeric"));
    assert!(msg.contains("bad input"));
}

#[test]
fn exit_code_clap_unknown_error_is_2() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::InvalidValue, "bad");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 2);
}

#[test]
fn exit_code_display_help_on_missing_is_0() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
        "help",
    );
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn exit_code_init_error_is_1() {
    let err = CliError::Init(vo_cli::InitError::DirNotFound {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_lock_error_is_1() {
    let err = CliError::Lock(vo_cli::LockError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_doctor_error_is_1() {
    let err = CliError::Doctor(vo_cli::DoctorError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_rebuild_error_is_1() {
    let err = CliError::Rebuild(vo_cli::commands::rebuild::RebuildError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_dispatch_error_is_1() {
    let err = CliError::Dispatch("boom".to_string());
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_invalid_numeric_is_2() {
    let err = CliError::InvalidNumeric("x".to_string());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn check_error_eq_same_file_not_found() {
    let a = CheckError::FileNotFound {
        path: PathBuf::from("/same"),
    };
    let b = CheckError::FileNotFound {
        path: PathBuf::from("/same"),
    };
    assert_eq!(a, b);
}

#[test]
fn check_error_neq_different_variants() {
    let a = CheckError::FileNotFound {
        path: PathBuf::from("/x"),
    };
    let b = CheckError::PermissionDenied {
        path: PathBuf::from("/x"),
    };
    assert_ne!(a, b);
}

#[test]
fn check_error_invalid_magic_eq() {
    let a = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [1, 2, 3, 4],
    };
    let b = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [1, 2, 3, 4],
    };
    assert_eq!(a, b);
}

#[test]
fn check_error_invalid_magic_neq_magic() {
    let a = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [1, 2, 3, 4],
    };
    let b = CheckError::InvalidMagic {
        path: PathBuf::from("/x"),
        magic: [5, 6, 7, 8],
    };
    assert_ne!(a, b);
}

#[test]
fn check_error_file_too_small_eq() {
    let a = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    let b = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    assert_eq!(a, b);
}
