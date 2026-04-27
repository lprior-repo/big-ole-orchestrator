mod test_helpers;
use test_helpers::{make_temp_dir, setup_project};
use vo_cli::commands::doctor_checks::{check_storage_integrity, Severity};

// ============================================================
// GAP: check_storage_integrity config path reference isolation
// ============================================================

#[test]
fn storage_integrity_config_references_nonexistent_path() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/nonexistent\"\n",
    )
    .unwrap();
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(
        report.warnings().any(|c| c.check == "storage-path-ref"),
        "should warn about non-existent storage path in config"
    );
}

#[test]
fn storage_integrity_config_references_valid_path() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "storage-path-ref" && c.severity == Severity::Info),
        "should report valid storage path reference"
    );
}

#[test]
fn storage_integrity_empty_storage_dir() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/storage")).unwrap();
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "storage-contents" && c.message.contains("empty")));
}

#[test]
fn storage_integrity_with_partitions() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/storage/events")).unwrap();
    std::fs::create_dir_all(dir.join(".vo/storage/instances")).unwrap();
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "storage-partitions"),
        "should detect known partitions"
    );
}

#[test]
fn storage_integrity_with_journal_file() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/storage")).unwrap();
    std::fs::write(dir.join(".vo/storage/events.journal"), b"j").unwrap();
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(
        report.warnings().any(|c| c.check == "storage-wal"),
        "should detect journal files"
    );
}

#[test]
fn storage_integrity_with_wal_suffix_file() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/storage")).unwrap();
    std::fs::write(dir.join(".vo/storage/data-wal"), b"w").unwrap();
    let report = check_storage_integrity(&dir.join(".vo"), &dir);
    assert!(
        report.warnings().any(|c| c.check == "storage-wal"),
        "should detect -wal suffixed files"
    );
}
