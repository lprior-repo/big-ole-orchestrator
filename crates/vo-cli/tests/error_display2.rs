use std::path::PathBuf;
use vo_cli::{CliError, DoctorError, GcError, InitError, LockError, RebuildError};

#[test]
fn init_error_dir_not_found_display() {
    let err = InitError::DirNotFound {
        path: PathBuf::from("/no/such/dir"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/no/such/dir"));
    assert!(msg.contains("does not exist"));
}

#[test]
fn init_error_not_directory_display() {
    let err = InitError::NotDirectory {
        path: PathBuf::from("/tmp/file"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/file"));
    assert!(msg.contains("not a directory"));
}

#[test]
fn init_error_already_initialized_display() {
    let err = InitError::AlreadyInitialized {
        path: PathBuf::from("/proj/.vo"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj/.vo"));
    assert!(msg.contains("already initialized"));
}

#[test]
fn init_error_permission_denied_display() {
    let err = InitError::PermissionDenied {
        path: PathBuf::from("/root"),
        reason: "read-only filesystem".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/root"));
    assert!(msg.contains("permission denied"));
    assert!(msg.contains("read-only"));
}

#[test]
fn init_error_symlink_target_display() {
    let err = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/link"));
    assert!(msg.contains("symlink"));
    assert!(msg.contains("refusing"));
}

#[test]
fn lock_error_not_initialized_display() {
    let err = LockError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("not initialized"));
}

#[test]
fn lock_error_no_workflows_dir_display() {
    let err = LockError::NoWorkflowsDir {
        path: PathBuf::from("/proj/.vo/workflows"),
    };
    let msg = err.to_string();
    assert!(msg.contains("workflows"));
    assert!(msg.contains("not found"));
}

#[test]
fn lock_error_empty_display() {
    let err = LockError::Empty {
        path: PathBuf::from("/proj/.vo/workflows"),
    };
    let msg = err.to_string();
    assert!(msg.contains("no workflow"));
}

#[test]
fn lock_error_lock_write_display() {
    let err = LockError::LockWrite {
        reason: "disk full".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("disk full"));
    assert!(msg.contains("lockfile"));
}

#[test]
fn doctor_error_not_initialized_display() {
    let err = DoctorError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("not initialized"));
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
fn rebuild_error_not_initialized_display() {
    let err = RebuildError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj"));
    assert!(msg.contains("not initialized"));
}

#[test]
fn rebuild_error_projection_not_found_display() {
    let err = RebuildError::ProjectionNotFound("my-proj".to_string());
    let msg = err.to_string();
    assert!(msg.contains("my-proj"));
    assert!(msg.contains("not found"));
}

#[test]
fn rebuild_error_rebuild_failed_display() {
    let err = RebuildError::RebuildFailed("timeout".to_string());
    let msg = err.to_string();
    assert!(msg.contains("timeout"));
    assert!(msg.contains("failed"));
}

#[test]
fn rebuild_error_unsupported_schema_display() {
    let err = RebuildError::UnsupportedSchemaVersion(99);
    let msg = err.to_string();
    assert!(msg.contains("99"));
    assert!(msg.contains("not supported"));
}

#[test]
fn rebuild_error_rebuild_in_progress_display() {
    let err = RebuildError::RebuildInProgress("proj-1".to_string());
    let msg = err.to_string();
    assert!(msg.contains("proj-1"));
    assert!(msg.contains("already in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_display() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "key-abc".to_string(),
        actual: "key-xyz".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("key-abc"));
    assert!(msg.contains("key-xyz"));
    assert!(msg.contains("mismatch"));
}

#[test]
fn rebuild_error_engine_display() {
    let err = RebuildError::Engine("internal failure".to_string());
    let msg = err.to_string();
    assert!(msg.contains("internal failure"));
    assert!(msg.contains("engine"));
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
