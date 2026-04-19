use std::path::PathBuf;
use vo_cli::cli::CliError;
use vo_cli::commands::check::CheckError;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::gc::GcError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::lock::LockError;
use vo_cli::commands::rebuild::RebuildError;
use vo_cli::commands::rebuild::{RebuildReport, RebuildStatus};

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
fn cli_error_dispatch_display() {
    let err = CliError::Dispatch("something broke".to_string());
    assert!(err.to_string().contains("something broke"));
}

#[test]
fn cli_error_invalid_numeric_display() {
    let err = CliError::InvalidNumeric("not a number".to_string());
    assert!(err.to_string().contains("not a number"));
}
