use std::path::PathBuf;
use vo_cli::commands::rebuild::{RebuildError, RebuildReport, RebuildStatus};

#[test]
fn rebuild_error_not_initialized_display() {
    let err = RebuildError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    assert!(err.to_string().contains("/proj"));
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn rebuild_error_projection_not_found_display() {
    let err = RebuildError::ProjectionNotFound("my-proj".into());
    assert!(err.to_string().contains("my-proj"));
}

#[test]
fn rebuild_error_rebuild_failed_display() {
    let err = RebuildError::RebuildFailed("timeout".into());
    assert!(err.to_string().contains("timeout"));
}

#[test]
fn rebuild_error_unsupported_schema_version_display() {
    let err = RebuildError::UnsupportedSchemaVersion(99);
    assert!(err.to_string().contains("99"));
}

#[test]
fn rebuild_error_rebuild_in_progress_display() {
    let err = RebuildError::RebuildInProgress("p1".into());
    assert!(err.to_string().contains("p1"));
    assert!(err.to_string().contains("in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_display() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "abc".into(),
        actual: "def".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("abc"));
    assert!(msg.contains("def"));
    assert!(msg.contains("mismatch"));
}

#[test]
fn rebuild_error_engine_display() {
    let err = RebuildError::Engine("connection refused".into());
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn rebuild_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
    let err: RebuildError = io_err.into();
    assert!(err.to_string().contains("pipe broke"));
}

#[test]
fn exit_code_rebuild_error_is_1() {
    use vo_cli::{map_error_to_exit_code, CliError};
    let err = CliError::Rebuild(RebuildError::NotInitialized {
        path: PathBuf::from("/p"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}
