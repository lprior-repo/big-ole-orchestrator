use std::path::PathBuf;
use vo_cli::commands::check::CheckError;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::gc::GcError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::lock::LockError;
use vo_cli::CliError;

#[test]
fn check_error_file_not_found_mentions_path() {
    let err = CheckError::FileNotFound {
        path: PathBuf::from("/nonexistent/path"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/nonexistent/path"));
    assert!(msg.contains("file not found"));
}

#[test]
fn check_error_not_regular_file_mentions_path() {
    let err = CheckError::NotRegularFile {
        path: PathBuf::from("/some/dir"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/some/dir"));
    assert!(msg.contains("not a regular file"));
}

#[test]
fn check_error_file_too_small_includes_min_bytes() {
    let err = CheckError::FileTooSmall {
        path: PathBuf::from("/small"),
    };
    let msg = err.to_string();
    assert!(msg.contains("4 bytes"));
}

#[test]
fn check_error_invalid_magic_shows_hex() {
    let err = CheckError::InvalidMagic {
        path: PathBuf::from("/bad"),
        magic: [0xCA, 0xFE, 0xBA, 0xBE],
    };
    let msg = err.to_string();
    assert!(msg.contains("0xca"));
    assert!(msg.contains("0xfe"));
    assert!(msg.contains("0xba"));
    assert!(msg.contains("0xbe"));
}

#[test]
fn gc_error_engine_unreachable_includes_url() {
    let err = GcError::EngineUnreachable {
        url: "http://engine:3000".into(),
        reason: "timeout".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("http://engine:3000"));
    assert!(msg.contains("503"));
}

#[test]
fn gc_error_http_error_includes_status() {
    let err = GcError::EngineHttpError {
        url: "http://engine:3000/api".into(),
        status: 500,
    };
    let msg = err.to_string();
    assert!(msg.contains("500"));
    assert!(msg.contains("HTTP"));
}

#[test]
fn gc_error_invalid_api_response_is_descriptive() {
    let err = GcError::InvalidApiResponse {
        reason: "expected array".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("failed to parse"));
    assert!(msg.contains("expected array"));
}

#[test]
fn gc_error_versions_dir_not_found_includes_path() {
    let err = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf/versions"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf/versions"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn gc_error_delete_failed_includes_path() {
    let err = GcError::DeleteFailed {
        path: PathBuf::from("/var/wtf/versions/abc123"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/var/wtf/versions/abc123"));
    assert!(msg.contains("failed to delete"));
}

#[test]
fn init_error_dir_not_found_message() {
    let err = InitError::DirNotFound {
        path: PathBuf::from("/no/such/dir"),
    };
    assert!(err.to_string().contains("/no/such/dir"));
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn init_error_not_directory_message() {
    let err = InitError::NotDirectory {
        path: PathBuf::from("/tmp/file"),
    };
    assert!(err.to_string().contains("not a directory"));
}

#[test]
fn init_error_already_initialized_message() {
    let err = InitError::AlreadyInitialized {
        path: PathBuf::from("/proj/.vo"),
    };
    assert!(err.to_string().contains("already initialized"));
}

#[test]
fn init_error_permission_denied_message() {
    let err = InitError::PermissionDenied {
        path: PathBuf::from("/root"),
        reason: "access denied".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("permission denied"));
    assert!(msg.contains("/root"));
}

#[test]
fn init_error_symlink_message() {
    let err = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    assert!(err.to_string().contains("symlink"));
    assert!(err.to_string().contains("refusing"));
}

#[test]
fn lock_error_not_initialized_message() {
    let err = LockError::NotInitialized {
        path: PathBuf::from("/project"),
    };
    assert!(err.to_string().contains("not initialized"));
    assert!(err.to_string().contains("/project"));
}

#[test]
fn lock_error_no_workflows_dir_message() {
    let err = LockError::NoWorkflowsDir {
        path: PathBuf::from("/project/.vo/workflows"),
    };
    assert!(err.to_string().contains("workflows directory not found"));
}

#[test]
fn lock_error_lock_write_message() {
    let err = LockError::LockWrite {
        reason: "disk full".into(),
    };
    assert!(err.to_string().contains("lockfile write failed"));
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn lock_error_empty_message() {
    let err = LockError::Empty {
        path: PathBuf::from("/project/.vo/workflows"),
    };
    assert!(err.to_string().contains("no workflow"));
}

#[test]
fn doctor_error_not_initialized_message() {
    let err = DoctorError::NotInitialized {
        path: PathBuf::from("/project"),
    };
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn cli_error_dispatch_display() {
    let err = CliError::Dispatch("connection refused".into());
    assert!(err.to_string().contains("dispatch"));
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn cli_error_invalid_numeric_display() {
    let err = CliError::InvalidNumeric("bad input".into());
    assert!(err.to_string().contains("invalid numeric"));
    assert!(err.to_string().contains("bad input"));
}
