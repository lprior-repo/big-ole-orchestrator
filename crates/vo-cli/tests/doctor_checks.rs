#![allow(clippy::redundant_pattern_matching)]
mod common;

use std::fs;

use common::setup_project;
use vo_cli::commands::doctor_checks::{
    check_lock_state, check_storage_integrity, check_workspace, CheckCategory, Severity,
};
use vo_cli::commands::init::LOCK_FILE_NAME;

#[test]
fn doctor_workspace_detects_readonly_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    let vo_dir = dir.path().join(".vo");
    fs::create_dir_all(&vo_dir).unwrap();

    let mut perms = fs::metadata(&vo_dir).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&vo_dir, perms).unwrap();

    let report = check_workspace(dir.path(), &vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Error));
}

#[test]
fn doctor_workspace_info_for_writable_vo_dir() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let report = check_workspace(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Info));
}

#[test]
fn doctor_lockstate_empty_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(dir.path().join(LOCK_FILE_NAME), "").unwrap();

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lockfile" && c.message.contains("empty")));
}

#[test]
fn doctor_lockstate_unreadable_lockfile() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let lock_path = dir.path().join(LOCK_FILE_NAME);
    fs::write(&lock_path, "test hash123\n").unwrap();
    let mut perms = fs::metadata(&lock_path).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&lock_path, perms).unwrap();

    let report = check_lock_state(dir.path(), &dir.path().join(".vo"));
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "lockfile" && c.severity == Severity::Error));

    let mut perms = fs::metadata(&lock_path).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&lock_path, perms).unwrap();
}

#[test]
fn doctor_storage_wal_patterns() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let storage = dir.path().join(".vo/storage");

    for pattern in &[
        "events.wal",
        "data.journal",
        "main-wal",
        "secondary-journal",
    ] {
        fs::write(storage.join(pattern), b"wal").unwrap();
    }

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    let wal_warnings: Vec<_> = report
        .warnings()
        .filter(|w| w.check == "storage-wal")
        .collect();
    assert!(wal_warnings.len() >= 3);
}

#[test]
fn doctor_storage_probe_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-rw" && c.severity == Severity::Info));

    assert!(!dir.path().join(".vo/storage/.doctor-probe").exists());
}

#[test]
fn doctor_storage_empty() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".vo/storage")).unwrap();

    let report = check_storage_integrity(&dir.path().join(".vo"), dir.path());
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-contents" && c.message.contains("empty")));
}
