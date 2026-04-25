#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;

use vo_cli::commands::check::CheckError;
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::gc::GcError;
use vo_cli::commands::history::HistoryError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::lock::LockError;
use vo_cli::commands::rebuild::RebuildError;
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
