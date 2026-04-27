#![allow(clippy::redundant_pattern_matching)]
mod common;

use std::fs;

use common::setup_project;
use vo_cli::commands::doctor_checks::check_config_validation;
use vo_cli::commands::init::CONFIG_FILE_NAME;

#[test]
fn config_missing_engine_section() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine"));
}

#[test]
fn config_missing_storage_section() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-storage"));
}

#[test]
fn config_empty_engine_url() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine-url"));
}

#[test]
fn config_empty_storage_path() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \"\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-storage-path"));
}

#[test]
fn config_missing_url_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nport = 3000\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-engine-url"));
}

#[test]
fn config_missing_path_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    fs::write(
        dir.path().join(CONFIG_FILE_NAME),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\nmode = \"local\"\n",
    )
    .unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-storage-path"));
}

#[test]
fn config_readonly_permissions() {
    let dir = tempfile::tempdir().unwrap();
    setup_project(dir.path());
    let config_path = dir.path().join(INIT_CONFIG_FILE_NAME);
    let mut perms = fs::metadata(&config_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&config_path, perms).unwrap();

    let report = check_config_validation(dir.path());
    assert!(report.warnings().any(|w| w.check == "config-perms"));
}
