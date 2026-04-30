#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::cli::CliError;
use vo_cli::commands::check::CheckError;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::gc::GcError;
use vo_cli::commands::history::HistoryError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::lock::LockError;
use vo_cli::commands::rebuild::{RebuildError, RebuildReport, RebuildStatus};
use vo_cli::CliError;

#[test]
fn history_error_display_all_variants() {
    let e1 = HistoryError::HistoryFileNotFound {
        path: PathBuf::from("/hist.json"),
    };
    assert!(e1.to_string().contains("/hist.json"));

    let e2 = HistoryError::ReadFailed {
        reason: "disk error".into(),
    };
    assert!(e2.to_string().contains("disk error"));

    let e3 = HistoryError::WriteFailed {
        reason: "permission".into(),
    };
    assert!(e3.to_string().contains("permission"));

    let e4 = HistoryError::InvalidFormat {
        reason: "bad json".into(),
    };
    assert!(e4.to_string().contains("bad json"));
}

#[test]
fn rebuild_error_display_all_variants() {
    let e1 = RebuildError::NotInitialized {
        path: PathBuf::from("/nope"),
    };
    assert!(e1.to_string().contains("/nope"));

    let e2 = RebuildError::ProjectionNotFound("proj-x".into());
    assert!(e2.to_string().contains("proj-x"));

    let e3 = RebuildError::RebuildFailed("OOM".into());
    assert!(e3.to_string().contains("OOM"));

    let e4 = RebuildError::UnsupportedSchemaVersion(99);
    assert!(e4.to_string().contains("99"));

    let e5 = RebuildError::RebuildInProgress("proj-y".into());
    assert!(e5.to_string().contains("proj-y"));

    let e6 = RebuildError::IdempotencyMismatch {
        expected: "abc".into(),
        actual: "def".into(),
    };
    assert!(e6.to_string().contains("abc"));
    assert!(e6.to_string().contains("def"));

    let e7 = RebuildError::Io {
        path: PathBuf::from("/io"),
        reason: "read".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e7.to_string().contains("/io"));

    let e8 = RebuildError::Engine("engine fail".into());
    assert!(e8.to_string().contains("engine fail"));
}

#[test]
fn init_error_display_all_variants() {
    let e1 = InitError::DirNotFound {
        path: PathBuf::from("/no-dir"),
    };
    assert!(e1.to_string().contains("/no-dir"));

    let e2 = InitError::NotDirectory {
        path: PathBuf::from("/file"),
    };
    assert!(e2.to_string().contains("/file"));

    let e3 = InitError::AlreadyInitialized {
        path: PathBuf::from("/exists"),
    };
    assert!(e3.to_string().contains("/exists"));

    let e4 = InitError::PermissionDenied {
        path: PathBuf::from("/denied"),
        reason: "nope".into(),
    };
    assert!(e4.to_string().contains("/denied"));

    let e5 = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    assert!(e5.to_string().contains("/link"));
    assert!(e5.to_string().contains("symlink"));
}

#[test]
fn lock_error_display_all_variants() {
    let e1 = LockError::NotInitialized {
        path: PathBuf::from("/no"),
    };
    assert!(e1.to_string().contains("/no"));

    let e2 = LockError::NoWorkflowsDir {
        path: PathBuf::from("/no-wf"),
    };
    assert!(e2.to_string().contains("/no-wf"));

    let e3 = LockError::Io {
        path: PathBuf::from("/io"),
        reason: "fail".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e3.to_string().contains("/io"));

    let e4 = LockError::LockWrite {
        reason: "disk full".into(),
    };
    assert!(e4.to_string().contains("disk full"));

    let e5 = LockError::Empty {
        path: PathBuf::from("/empty"),
    };
    assert!(e5.to_string().contains("/empty"));
}

#[test]
fn doctor_error_display() {
    let e1 = DoctorError::NotInitialized {
        path: PathBuf::from("/no-init"),
    };
    assert!(e1.to_string().contains("/no-init"));

    let e2 = DoctorError::Io {
        path: PathBuf::from("/io"),
        reason: "read err".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e2.to_string().contains("/io"));
}

#[test]
fn gc_error_display_all_variants() {
    let e1 = GcError::EngineUnreachable {
        url: "http://fail".into(),
        reason: "timeout".into(),
    };
    assert!(e1.to_string().contains("http://fail"));
    assert!(e1.to_string().contains("timeout"));

    let e2 = GcError::EngineHttpError {
        url: "http://err".into(),
        status: 500,
    };
    assert!(e2.to_string().contains("500"));

    let e3 = GcError::InvalidApiResponse {
        reason: "bad json".into(),
    };
    assert!(e3.to_string().contains("bad json"));

    let e4 = GcError::VersionsDirNotFound {
        path: PathBuf::from("/no-versions"),
    };
    assert!(e4.to_string().contains("/no-versions"));

    let e5 = GcError::DeleteFailed {
        path: PathBuf::from("/del"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope"),
    };
    assert!(e5.to_string().contains("/del"));
}

#[test]
fn cli_error_display_dispatch() {
    let e = CliError::Dispatch("something went wrong".to_string());
    assert!(e.to_string().contains("something went wrong"));
}

#[test]
fn cli_error_display_invalid_numeric() {
    let e = CliError::InvalidNumeric("bad number".to_string());
    assert!(e.to_string().contains("bad number"));
}

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

#[test]
fn rebuild_status_listed_format_shows_projections() {
    let report = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["orders".to_string(), "inventory".to_string()]),
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("orders"));
    assert!(output.contains("inventory"));
    assert!(output.contains("Registered projections"));
}

#[test]
fn rebuild_status_listed_empty_format() {
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
fn rebuild_status_started_format() {
    let report = RebuildReport {
        projection_id: Some("p1".to_string()),
        rebuild_id: Some("r1".to_string()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("started"));
    assert!(output.contains("42"));
}

#[test]
fn rebuild_status_failed_format() {
    let report = RebuildReport {
        projection_id: Some("p1".to_string()),
        rebuild_id: Some("r1".to_string()),
        status: RebuildStatus::Failed {
            reason: "disk full".to_string(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("failed"));
    assert!(output.contains("disk full"));
}

#[test]
fn rebuild_status_noop_format() {
    let report = RebuildReport {
        projection_id: Some("p1".to_string()),
        rebuild_id: Some("r1".to_string()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".to_string(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    let output = report.format_progress();
    assert!(output.contains("skipped"));
    assert!(output.contains("already up to date"));
}

#[test]
fn rebuild_status_equality_same() {
    let a = RebuildStatus::Completed;
    let b = RebuildStatus::Completed;
    assert_eq!(a, b);
}

#[test]
fn rebuild_status_inequality() {
    let a = RebuildStatus::Completed;
    let b = RebuildStatus::Failed {
        reason: "x".to_string(),
    };
    assert_ne!(a, b);
}

#[test]
fn rebuild_report_equality() {
    let a = RebuildReport {
        projection_id: Some("p".to_string()),
        rebuild_id: None,
        status: RebuildStatus::Completed,
        events_applied: 100,
        duration_ms: 50,
    };
    let b = RebuildReport {
        projection_id: Some("p".to_string()),
        rebuild_id: None,
        status: RebuildStatus::Completed,
        events_applied: 100,
        duration_ms: 50,
    };
    assert_eq!(a, b);
}

#[test]
fn init_error_dir_not_found_message() {
    let err = InitError::DirNotFound {
        path: PathBuf::from("/missing"),
    };
    assert!(err.to_string().contains("/missing"));
}

#[test]
fn init_error_not_directory_message() {
    let err = InitError::NotDirectory {
        path: PathBuf::from("/a/file"),
    };
    assert!(err.to_string().contains("not a directory"));
    assert!(err.to_string().contains("/a/file"));
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
        reason: "access denied".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("permission denied"));
    assert!(msg.contains("/root"));
}

#[test]
fn init_error_io_message() {
    let err = InitError::Io {
        path: PathBuf::from("/tmp"),
        reason: "write failed".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp"));
    assert!(msg.contains("write failed"));
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
fn lock_error_io_message() {
    let err = LockError::Io {
        path: PathBuf::from("/project/.vo"),
        reason: "read error".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "eof"),
    };
    assert!(err.to_string().contains("/project/.vo"));
}

#[test]
fn lock_error_lock_write_message() {
    let err = LockError::LockWrite {
        reason: "disk full".to_string(),
    };
    assert!(err.to_string().contains("lockfile write failed"));
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn lock_error_empty_message() {
    let err = LockError::Empty {
        path: PathBuf::from("/project/.vo/workflows"),
    };
    assert!(err.to_string().contains("no workflow binaries"));
}

#[test]
fn doctor_error_not_initialized_message() {
    let err = DoctorError::NotInitialized {
        path: PathBuf::from("/proj"),
    };
    assert!(err.to_string().contains("not initialized"));
    assert!(err.to_string().contains("/proj"));
}

#[test]
fn doctor_error_io_message() {
    let err = DoctorError::Io {
        path: PathBuf::from("/proj/.vo"),
        reason: "metadata read".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/proj/.vo"));
    assert!(msg.contains("metadata read"));
}

#[test]
fn rebuild_error_projection_not_found_message() {
    let err = RebuildError::ProjectionNotFound("orders".to_string());
    assert!(err.to_string().contains("orders"));
}

#[test]
fn rebuild_error_rebuild_failed_message() {
    let err = RebuildError::RebuildFailed("corrupt log".to_string());
    assert!(err.to_string().contains("corrupt log"));
}

#[test]
fn rebuild_error_unsupported_schema_message() {
    let err = RebuildError::UnsupportedSchemaVersion(99);
    assert!(err.to_string().contains("99"));
}

#[test]
fn rebuild_error_in_progress_message() {
    let err = RebuildError::RebuildInProgress("proj-1".to_string());
    assert!(err.to_string().contains("proj-1"));
    assert!(err.to_string().contains("in progress"));
}

#[test]
fn rebuild_error_idempotency_mismatch_message() {
    let err = RebuildError::IdempotencyMismatch {
        expected: "abc".to_string(),
        actual: "def".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("abc"));
    assert!(msg.contains("def"));
}

#[test]
fn rebuild_error_engine_message() {
    let err = RebuildError::Engine("timeout".to_string());
    assert!(err.to_string().contains("timeout"));
}

#[test]
fn check_error_io_display() {
    let err = CheckError::Io {
        path: PathBuf::from("/tmp/file"),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    assert!(err.to_string().contains("/tmp/file"));
}

#[test]
fn cli_error_from_check_error() {
    let err = CliError::from(CheckError::FileNotFound {
        path: PathBuf::from("/missing"),
    });
    assert!(err.to_string().contains("/missing"));
}

#[test]
fn cli_error_from_init_error() {
    let err = CliError::from(InitError::DirNotFound {
        path: PathBuf::from("/gone"),
    });
    assert!(err.to_string().contains("/gone"));
}

#[test]
fn cli_error_from_lock_error() {
    let err = CliError::from(LockError::NotInitialized {
        path: PathBuf::from("/nope"),
    });
    assert!(err.to_string().contains("/nope"));
}

#[test]
fn cli_error_from_doctor_error() {
    let err = CliError::from(DoctorError::NotInitialized {
        path: PathBuf::from("/nope"),
    });
    assert!(err.to_string().contains("/nope"));
}

#[test]
fn cli_error_from_rebuild_error() {
    let err = CliError::from(RebuildError::NotInitialized {
        path: PathBuf::from("/nope"),
    });
    assert!(err.to_string().contains("/nope"));
}

#[test]
fn cli_error_from_gc_error() {
    let err = CliError::from(GcError::VersionsDirNotFound {
        path: PathBuf::from("/versions"),
    });
    assert!(err.to_string().contains("/versions"));
}

#[test]
fn cli_error_dispatch_message() {
    let err = CliError::Dispatch("something broke".to_string());
    assert!(err.to_string().contains("something broke"));
}

#[test]
fn cli_error_invalid_numeric_message() {
    let err = CliError::InvalidNumeric("not a number".to_string());
    assert!(err.to_string().contains("not a number"));
}
