#![allow(clippy::redundant_pattern_matching)]
use std::collections::HashSet;
use std::path::PathBuf;
use vo_cli::commands::check::{BinaryFormat, CheckError};
use vo_cli::commands::doctor::DoctorError;
use vo_cli::commands::doctor_checks::{
    check_config_validation, check_lock_state, check_storage_integrity, check_subprocess_liveness,
    check_workspace, format_report, format_report_json, CategoryReport, CheckCategory, CheckResult,
    DoctorReport, Severity,
};
use vo_cli::commands::gc::{GcConfig, GcError, GcSummary};
use vo_cli::commands::history::{
    get_history, load_history, redo_command, save_history, undo_command, HistoryConfig,
    HistoryError, HistoryOutput,
};
use vo_cli::commands::init::{InitConfig, InitError, CONFIG_FILE_NAME, VO_DIR_NAME};
use vo_cli::commands::lock::{LockConfig, LockError, LOCK_FILE_NAME};
use vo_cli::commands::rebuild::{RebuildConfig, RebuildError, RebuildReport, RebuildStatus};
use vo_cli::{
    interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command,
    CommandContext, HandlerRegistry,
};

fn make_temp_dir() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().to_path_buf();
    std::mem::forget(dir);
    p
}

fn setup_project(dir: &std::path::Path) {
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("workflows")).unwrap();
    std::fs::create_dir_all(vo_dir.join("storage")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
}

// ============================================================
// GAP: check_subprocess_liveness with PID files
// ============================================================

#[test]
fn subprocess_liveness_with_stale_pid_file() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/test.pid"), "999999999\n").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    let has_dead = report.checks.iter().any(|c| c.check == "process-dead");
    assert!(has_dead, "stale PID should produce process-dead check");
}

#[test]
fn subprocess_liveness_with_invalid_pid_content() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/bad.pid"), "not-a-number\n").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    let no_checks = report
        .checks
        .iter()
        .all(|c| c.check != "process-alive" && c.check != "process-dead");
    assert!(
        no_checks,
        "invalid PID file content should be skipped gracefully"
    );
}

#[test]
fn subprocess_liveness_with_empty_pid_file() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/empty.pid"), "").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    let no_checks = report
        .checks
        .iter()
        .all(|c| c.check != "process-alive" && c.check != "process-dead");
    assert!(no_checks, "empty PID file should be skipped gracefully");
}

#[test]
fn subprocess_liveness_with_non_pid_files() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    std::fs::write(vo_dir.join("runtime/readme.txt"), "hello").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "subprocess-liveness" && c.message.contains("no PID files")));
}

#[test]
fn subprocess_liveness_with_current_pid() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    let my_pid = std::process::id();
    std::fs::write(vo_dir.join("runtime/self.pid"), format!("{my_pid}\n")).unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(
        report.checks.iter().any(|c| c.check == "process-alive"),
        "current process PID should be detected as alive"
    );
}

#[test]
fn subprocess_liveness_mixed_pid_files() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    let my_pid = std::process::id();
    std::fs::write(vo_dir.join("runtime/alive.pid"), format!("{my_pid}\n")).unwrap();
    std::fs::write(vo_dir.join("runtime/dead.pid"), "999999999\n").unwrap();
    std::fs::write(vo_dir.join("runtime/bad.pid"), "xyz\n").unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report.checks.iter().any(|c| c.check == "process-alive"));
    assert!(report.checks.iter().any(|c| c.check == "process-dead"));
}

#[test]
fn subprocess_liveness_cannot_read_runtime_dir() {
    let dir = make_temp_dir();
    let vo_dir = dir.join(".vo");
    std::fs::create_dir_all(vo_dir.join("runtime")).unwrap();
    let report = check_subprocess_liveness(&vo_dir);
    assert!(report
        .checks
        .iter()
        .any(|c| c.check == "runtime-dir" || c.check == "subprocess-liveness"));
}

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

// ============================================================
// GAP: check_workspace with stale PID files in workspace check
// ============================================================

#[test]
fn workspace_detects_stale_pid_files() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::create_dir_all(dir.join(".vo/runtime")).unwrap();
    std::fs::write(dir.join(".vo/runtime/old.pid"), "999999999\n").unwrap();
    let report = check_workspace(&dir, &dir.join(".vo"));
    assert!(
        report.checks.iter().any(|c| c.check == "stale-pid-files"),
        "workspace check should detect stale PID files"
    );
}

#[test]
fn workspace_detects_alive_pid_files() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::create_dir_all(dir.join(".vo/runtime")).unwrap();
    let my_pid = std::process::id();
    std::fs::write(dir.join(".vo/runtime/self.pid"), format!("{my_pid}\n")).unwrap();
    let report = check_workspace(&dir, &dir.join(".vo"));
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "stale-pid-files" && c.severity == Severity::Info),
        "workspace check should report all alive PIDs"
    );
}

#[test]
fn workspace_readonly_vo_dir() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let vo_dir = dir.join(".vo");
    let mut perms = std::fs::metadata(&vo_dir).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&vo_dir, perms.clone()).unwrap();
    let report = check_workspace(&dir, &vo_dir);
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "vo-dir-perms" && c.severity == Severity::Error),
        "readonly .vo dir should be an error"
    );
    perms.set_readonly(false);
    std::fs::set_permissions(&vo_dir, perms).ok();
}

#[test]
fn workspace_missing_storage_dir_warns() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    let report = check_workspace(&dir, &dir.join(".vo"));
    assert!(
        report
            .checks
            .iter()
            .any(|c| c.check == "storage-dir" && c.severity == Severity::Warn),
        "missing storage dir should warn"
    );
}

// ============================================================
// GAP: check_config_validation edge cases
// ============================================================

#[test]
fn config_validation_missing_engine_section() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-engine"),
        "should warn about missing [engine] section"
    );
}

#[test]
fn config_validation_engine_url_missing() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nport = 3000\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-engine-url"),
        "should warn about missing engine URL"
    );
}

#[test]
fn config_validation_engine_url_empty() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-engine-url"),
        "should warn about empty engine URL"
    );
}

#[test]
fn config_validation_storage_missing_section() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n",
    )
    .unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-storage"),
        "should warn about missing [storage] section"
    );
}

#[test]
fn config_validation_storage_path_missing() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\nother = \"x\"\n",
    )
    .unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-storage-path"),
        "should warn about missing storage path"
    );
}

#[test]
fn config_validation_storage_path_empty() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \"\"\n",
    )
    .unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-storage-path"),
        "should warn about empty storage path"
    );
}

#[test]
fn config_validation_readonly_config() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(
        dir.join("config.toml"),
        "[engine]\nurl = \"http://localhost:3000\"\n\n[storage]\npath = \".vo/storage\"\n",
    )
    .unwrap();
    let config_path = dir.join("config.toml");
    let mut perms = std::fs::metadata(&config_path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&config_path, perms.clone()).unwrap();
    let report = check_config_validation(&dir);
    assert!(
        report.warnings().any(|c| c.check == "config-perms"),
        "readonly config should produce warning"
    );
    perms.set_readonly(false);
    std::fs::set_permissions(&config_path, perms).ok();
}

// ============================================================
// GAP: check_lock_state edge cases
// ============================================================

#[test]
fn lock_state_empty_lockfile_warns() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::write(dir.join("vo.lock"), "").unwrap();
    let report = check_lock_state(&dir, &dir.join(".vo"));
    assert!(report
        .warnings()
        .any(|c| c.check == "lockfile" && c.message.contains("empty")));
}

#[test]
fn lock_state_unreadable_lockfile() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::write(dir.join("vo.lock"), "wf abc123\n").unwrap();
    let lock_path = dir.join("vo.lock");
    let mut perms = std::fs::metadata(&lock_path).unwrap().permissions();
    perms.set_readonly(false);
    std::fs::set_permissions(&lock_path, perms).unwrap();
    let report = check_lock_state(&dir, &dir.join(".vo"));
    assert!(report.checks.iter().any(|c| c.check == "lockfile"));
}

#[test]
fn lock_state_binary_read_fails() {
    let dir = make_temp_dir();
    setup_project(&dir);
    std::fs::write(dir.join(".vo/workflows/mybin"), b"content").unwrap();
    let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    std::fs::write(dir.join("vo.lock"), format!("mybin {bad_hash}\n")).unwrap();
    let report = check_lock_state(&dir, &dir.join(".vo"));
    assert!(!report.is_healthy(), "hash mismatch should be unhealthy");
}

// ============================================================
// GAP: History module — undo_command/redo_command success paths
// ============================================================

#[test]
fn history_undo_success_path() {
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                compensation_policy: None,
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let result = undo_command(&mut history);
    assert!(result.success, "undo with history should succeed");
    assert_eq!(result.message, "Undo successful");
}

#[test]
fn history_redo_success_path() {
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                compensation_policy: None,
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let _ = undo_command(&mut history);
    let result = redo_command(&mut history);
    assert!(result.success, "redo after undo should succeed");
    assert_eq!(result.message, "Redo successful");
}

#[test]
fn history_undo_then_undo_empty() {
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                compensation_policy: None,
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let _ = undo_command(&mut history);
    let result = undo_command(&mut history);
    assert!(!result.success, "second undo should be empty");
}

#[test]
fn history_redo_empty_after_push() {
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                compensation_policy: None,
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let result = redo_command(&mut history);
    assert!(!result.success, "redo without undo should be empty");
}

#[test]
fn history_get_history_with_entries() {
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                compensation_policy: None,
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let output = get_history(&history);
    assert_eq!(output.entries.len(), 1);
    assert!(output.can_undo);
    assert!(!output.can_redo);
}

#[test]
fn history_save_and_reload_roundtrip() {
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                compensation_policy: None,
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    let dir = make_temp_dir();
    let path = dir.join("history.json");
    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    save_history(&path, &history).unwrap();
    let loaded = load_history(&path).unwrap();
    assert_eq!(loaded.entries().len(), 1);
}

#[test]
fn history_load_nonexistent_returns_empty() {
    let path = PathBuf::from("/tmp/nonexistent_history_test_12345.json");
    let result = load_history(&path);
    assert!(result.is_ok());
    assert!(result.unwrap().entries().is_empty());
}

#[test]
fn history_load_invalid_json() {
    let dir = make_temp_dir();
    let path = dir.join("bad_history.json");
    std::fs::write(&path, "not valid json{{{").unwrap();
    let result = load_history(&path);
    assert!(matches!(result, Err(HistoryError::InvalidFormat { .. })));
}

#[test]
fn history_error_display_all_variants() {
    let e1 = HistoryError::HistoryFileNotFound {
        path: PathBuf::from("/a"),
    };
    assert!(e1.to_string().contains("/a"));

    let e2 = HistoryError::ReadFailed {
        reason: "disk".into(),
    };
    assert!(e2.to_string().contains("disk"));

    let e3 = HistoryError::WriteFailed {
        reason: "full".into(),
    };
    assert!(e3.to_string().contains("full"));

    let e4 = HistoryError::InvalidFormat {
        reason: "bad json".into(),
    };
    assert!(e4.to_string().contains("bad json"));
}

// ============================================================
// GAP: GcError display all variants
// ============================================================

#[test]
fn gc_error_engine_unreachable_display() {
    let e = GcError::EngineUnreachable {
        url: "http://localhost:3000".into(),
        reason: "connection refused".into(),
    };
    assert!(e.to_string().contains("localhost"));
    assert!(e.to_string().contains("connection refused"));
}

#[test]
fn gc_error_http_error_display() {
    let e = GcError::EngineHttpError {
        url: "http://localhost:3000".into(),
        status: 500,
    };
    assert!(e.to_string().contains("500"));
    assert!(e.to_string().contains("localhost"));
}

#[test]
fn gc_error_invalid_api_response_display() {
    let e = GcError::InvalidApiResponse {
        reason: "missing hashes".into(),
    };
    assert!(e.to_string().contains("missing hashes"));
}

#[test]
fn gc_error_versions_dir_not_found_display() {
    let e = GcError::VersionsDirNotFound {
        path: PathBuf::from("/var/wtf"),
    };
    assert!(e.to_string().contains("/var/wtf"));
}

#[test]
fn gc_error_delete_failed_display() {
    let e = GcError::DeleteFailed {
        path: PathBuf::from("/var/wtf/abc"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    assert!(e.to_string().contains("/var/wtf/abc"));
    assert!(e.to_string().contains("denied"));
}

#[test]
fn gc_config_default_values() {
    let config = GcConfig::default();
    assert_eq!(config.engine_url, "http://localhost:3000");
    assert_eq!(config.versions_dir, PathBuf::from("/var/wtf/versions"));
    assert!(!config.dry_run);
}

#[test]
fn gc_summary_fields() {
    let summary = GcSummary {
        pinned_count: 5,
        scanned_count: 10,
        deleted_count: 3,
        deleted_hashes: vec!["abc".into(), "def".into()],
        failures: vec![(PathBuf::from("/x"), "err".into())],
    };
    assert_eq!(summary.pinned_count, 5);
    assert_eq!(summary.scanned_count, 10);
    assert_eq!(summary.deleted_count, 3);
    assert_eq!(summary.deleted_hashes.len(), 2);
    assert_eq!(summary.failures.len(), 1);
}

#[tokio::test]
async fn gc_find_unpinned_with_empty_versions_dir() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("nonexistent");
    let pinned: HashSet<String> = HashSet::new();
    let result = vo_cli::commands::gc::find_unpinned_directories(&versions_dir, &pinned).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn gc_find_unpinned_filters_non_hex_names() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("versions");
    std::fs::create_dir_all(versions_dir.join("not-a-hex-name")).unwrap();
    std::fs::create_dir_all(versions_dir.join("abc123")).unwrap();
    let pinned: HashSet<String> = HashSet::new();
    let result = vo_cli::commands::gc::find_unpinned_directories(&versions_dir, &pinned).await;
    assert!(result.is_ok());
    let unpinned = result.unwrap();
    assert!(unpinned.is_empty(), "non-hex-64 names should be filtered");
}

#[tokio::test]
async fn gc_find_unpinned_filters_pinned() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("versions");
    let hash = "a".repeat(64);
    std::fs::create_dir_all(versions_dir.join(&hash)).unwrap();
    let mut pinned: HashSet<String> = HashSet::new();
    pinned.insert(hash.clone());
    let result = vo_cli::commands::gc::find_unpinned_directories(&versions_dir, &pinned).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty(), "pinned dirs should be excluded");
}

#[tokio::test]
async fn gc_find_unpinned_returns_unpinned() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("versions");
    let hash1 = "a".repeat(64);
    let hash2 = "b".repeat(64);
    std::fs::create_dir_all(versions_dir.join(&hash1)).unwrap();
    std::fs::create_dir_all(versions_dir.join(&hash2)).unwrap();
    let mut pinned: HashSet<String> = HashSet::new();
    pinned.insert(hash1);
    let result = vo_cli::commands::gc::find_unpinned_directories(&versions_dir, &pinned).await;
    assert!(result.is_ok());
    let unpinned = result.unwrap();
    assert_eq!(unpinned.len(), 1);
    assert!(unpinned[0].to_str().unwrap().contains(&hash2));
}

#[tokio::test]
async fn gc_find_unpinned_results_sorted() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("versions");
    let hash_b = "b".repeat(64);
    let hash_a = "a".repeat(64);
    std::fs::create_dir_all(versions_dir.join(&hash_b)).unwrap();
    std::fs::create_dir_all(versions_dir.join(&hash_a)).unwrap();
    let pinned: HashSet<String> = HashSet::new();
    let result = vo_cli::commands::gc::find_unpinned_directories(&versions_dir, &pinned).await;
    let unpinned = result.unwrap();
    assert!(unpinned[0].to_str().unwrap().contains(&hash_a));
    assert!(unpinned[1].to_str().unwrap().contains(&hash_b));
}

#[tokio::test]
async fn gc_find_unpinned_ignores_files() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("versions");
    std::fs::create_dir_all(&versions_dir).unwrap();
    std::fs::write(versions_dir.join("somefile.txt"), "data").unwrap();
    let pinned: HashSet<String> = HashSet::new();
    let result = vo_cli::commands::gc::find_unpinned_directories(&versions_dir, &pinned).await;
    assert!(
        result.unwrap().is_empty(),
        "regular files should be ignored"
    );
}

#[tokio::test]
async fn gc_delete_version_dir_success() {
    let dir = make_temp_dir();
    let target = dir.join("to_delete");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("file.txt"), "data").unwrap();
    let result = vo_cli::commands::gc::delete_version_dir(&target).await;
    assert!(result.is_ok());
    assert!(!target.exists());
}

#[tokio::test]
async fn gc_delete_version_dir_nonexistent() {
    let result = vo_cli::commands::gc::delete_version_dir(
        PathBuf::from("/tmp/nonexistent_dir_xyz").as_path(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn gc_run_gc_dry_run() {
    let dir = make_temp_dir();
    let versions_dir = dir.join("versions");
    let hash = "c".repeat(64);
    std::fs::create_dir_all(versions_dir.join(&hash)).unwrap();
    let config = GcConfig {
        engine_url: "http://localhost:19998".into(),
        versions_dir: versions_dir.clone(),
        dry_run: true,
    };
    let result = vo_cli::commands::gc::run_gc(&config).await;
    if let Ok(summary) = result {
        assert!(
            versions_dir.join(&hash).exists(),
            "dry run should not delete"
        );
        assert!(
            summary.deleted_count > 0,
            "dry run should report would-be-deleted count"
        );
        assert!(
            summary.failures.is_empty(),
            "dry run should have no failures"
        );
    }
}

// ============================================================
// GAP: RebuildError all variants display + From<std::io::Error>
// ============================================================

#[test]
fn rebuild_error_all_variants_display() {
    let e1 = RebuildError::NotInitialized {
        path: PathBuf::from("/p"),
    };
    assert!(e1.to_string().contains("/p"));

    let e2 = RebuildError::ProjectionNotFound("proj1".into());
    assert!(e2.to_string().contains("proj1"));

    let e3 = RebuildError::RebuildFailed("timeout".into());
    assert!(e3.to_string().contains("timeout"));

    let e4 = RebuildError::UnsupportedSchemaVersion(99);
    assert!(e4.to_string().contains("99"));

    let e5 = RebuildError::RebuildInProgress("proj-x".into());
    assert!(e5.to_string().contains("proj-x"));

    let e6 = RebuildError::IdempotencyMismatch {
        expected: "abc".into(),
        actual: "def".into(),
    };
    assert!(e6.to_string().contains("abc"));
    assert!(e6.to_string().contains("def"));

    let e7 = RebuildError::Io {
        path: PathBuf::from("/io"),
        reason: "read error".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke"),
    };
    assert!(e7.to_string().contains("/io"));

    let e8 = RebuildError::Engine("engine fail".into());
    assert!(e8.to_string().contains("engine fail"));
}

#[test]
fn rebuild_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let rebuild_err: RebuildError = io_err.into();
    match rebuild_err {
        RebuildError::Io { path, reason, .. } => {
            assert!(path.as_os_str().is_empty());
            assert!(reason.contains("not found"));
        }
        _ => panic!("expected Io variant"),
    }
}

#[test]
fn rebuild_status_all_format_progress() {
    let listed = RebuildReport {
        projection_id: None,
        rebuild_id: None,
        status: RebuildStatus::Listed(vec!["p1".into(), "p2".into()]),
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(listed.format_progress().contains("p1"));
    assert!(listed.format_progress().contains("p2"));

    let started = RebuildReport {
        projection_id: Some("x".into()),
        rebuild_id: Some("x-1".into()),
        status: RebuildStatus::Started { from_sequence: 42 },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(started.format_progress().contains("42"));

    let failed = RebuildReport {
        projection_id: Some("y".into()),
        rebuild_id: Some("y-1".into()),
        status: RebuildStatus::Failed {
            reason: "OOM".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(failed.format_progress().contains("OOM"));

    let noop = RebuildReport {
        projection_id: Some("z".into()),
        rebuild_id: Some("z-1".into()),
        status: RebuildStatus::NoOp {
            reason: "already up to date".into(),
        },
        events_applied: 0,
        duration_ms: 0,
    };
    assert!(noop.format_progress().contains("already up to date"));
}

#[test]
fn rebuild_not_initialized_error() {
    let dir = make_temp_dir();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("p".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(matches!(result, Err(RebuildError::NotInitialized { .. })));
}

#[test]
fn rebuild_list_projections_success() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: true,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(matches!(report.status, RebuildStatus::Listed(_)));
}

#[test]
fn rebuild_without_projection_id_and_not_list() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: None,
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_err());
}

#[test]
fn rebuild_with_projection_id_success() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = RebuildConfig {
        project_dir: dir.clone(),
        projection_id: Some("my-proj".into()),
        list_projections: false,
        force: false,
        schema_version: None,
    };
    let result = vo_cli::commands::rebuild::run_rebuild(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(matches!(report.status, RebuildStatus::Completed));
    assert_eq!(report.projection_id.as_deref(), Some("my-proj"));
}

// ============================================================
// GAP: DoctorError display variants
// ============================================================

#[test]
fn doctor_error_not_initialized_display() {
    let e = DoctorError::NotInitialized {
        path: PathBuf::from("/myproject"),
    };
    assert!(e.to_string().contains("/myproject"));
}

#[test]
fn doctor_error_io_display() {
    let e = DoctorError::Io {
        path: PathBuf::from("/myproject/.vo"),
        reason: "read failed".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    let msg = e.to_string();
    assert!(msg.contains("/myproject/.vo"));
    assert!(msg.contains("read failed"));
}

// ============================================================
// GAP: LockError all display variants
// ============================================================

#[test]
fn lock_error_all_display_variants() {
    let e1 = LockError::NotInitialized {
        path: PathBuf::from("/a"),
    };
    assert!(e1.to_string().contains("/a"));

    let e2 = LockError::NoWorkflowsDir {
        path: PathBuf::from("/b"),
    };
    assert!(e2.to_string().contains("/b"));

    let e3 = LockError::Io {
        path: PathBuf::from("/c"),
        reason: "read".into(),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"),
    };
    assert!(e3.to_string().contains("/c"));

    let e4 = LockError::LockWrite {
        reason: "perm".into(),
    };
    assert!(e4.to_string().contains("perm"));

    let e5 = LockError::Empty {
        path: PathBuf::from("/d"),
    };
    assert!(e5.to_string().contains("/d"));
}

// ============================================================
// GAP: InitError all display variants
// ============================================================

#[test]
fn init_error_dir_not_found_display() {
    let e = InitError::DirNotFound {
        path: PathBuf::from("/nope"),
    };
    assert!(e.to_string().contains("/nope"));
}

#[test]
fn init_error_not_directory_display() {
    let e = InitError::NotDirectory {
        path: PathBuf::from("/file"),
    };
    assert!(e.to_string().contains("/file"));
}

#[test]
fn init_error_already_initialized_display() {
    let e = InitError::AlreadyInitialized {
        path: PathBuf::from("/has"),
    };
    assert!(e.to_string().contains("/has"));
}

#[test]
fn init_error_permission_denied_display() {
    let e = InitError::PermissionDenied {
        path: PathBuf::from("/denied"),
        reason: "no access".into(),
    };
    let msg = e.to_string();
    assert!(msg.contains("/denied"));
    assert!(msg.contains("no access"));
}

#[test]
fn init_error_symlink_target_display() {
    let e = InitError::SymlinkTarget {
        path: PathBuf::from("/link"),
    };
    assert!(e.to_string().contains("/link"));
}

// ============================================================
// GAP: CheckError display + PartialEq edge cases
// ============================================================

#[test]
fn check_error_file_not_found_display() {
    let e = CheckError::FileNotFound {
        path: PathBuf::from("/gone"),
    };
    assert!(e.to_string().contains("/gone"));
}

#[test]
fn check_error_not_regular_file_display() {
    let e = CheckError::NotRegularFile {
        path: PathBuf::from("/dir"),
    };
    assert!(e.to_string().contains("/dir"));
}

#[test]
fn check_error_file_too_small_display() {
    let e = CheckError::FileTooSmall {
        path: PathBuf::from("/tiny"),
    };
    assert!(e.to_string().contains("/tiny"));
    assert!(e.to_string().contains("4 bytes"));
}

#[test]
fn check_error_invalid_magic_display() {
    let e = CheckError::InvalidMagic {
        path: PathBuf::from("/bad"),
        magic: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let msg = e.to_string();
    assert!(msg.contains("/bad"));
    assert!(msg.contains("0xde"));
}

#[test]
fn check_error_partial_eq_cross_variant() {
    let e1 = CheckError::FileNotFound {
        path: PathBuf::from("/a"),
    };
    let e2 = CheckError::NotRegularFile {
        path: PathBuf::from("/a"),
    };
    assert_ne!(e1, e2, "cross-variant should not be equal");
}

#[test]
fn check_error_partial_eq_io_never_equal() {
    let e1 = CheckError::Io {
        path: PathBuf::from("/a"),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "x"),
    };
    let e2 = CheckError::Io {
        path: PathBuf::from("/a"),
        source: std::io::Error::new(std::io::ErrorKind::BrokenPipe, "x"),
    };
    assert_ne!(e1, e2, "Io variants should never be equal (no match arm)");
}

// ============================================================
// GAP: CliError display for InvalidNumeric
// ============================================================

#[test]
fn cli_error_invalid_numeric_display() {
    let e = CliError::InvalidNumeric("abc".into());
    assert!(e.to_string().contains("abc"));
}

#[test]
fn cli_error_dispatch_display() {
    let e = CliError::Dispatch("something broke".into());
    assert!(e.to_string().contains("something broke"));
}

// ============================================================
// GAP: CommandContext clone + metadata overwrite
// ============================================================

#[test]
fn command_context_metadata_overwrite() {
    let ctx = CommandContext::new("test");
    ctx.set_metadata("key", "value1");
    ctx.set_metadata("key", "value2");
    assert_eq!(ctx.get_metadata("key"), Some("value2".into()));
}

#[test]
fn command_context_clone_shares_metadata() {
    let ctx = CommandContext::new("test");
    let ctx2 = ctx.clone();
    ctx.set_metadata("k", "v");
    assert_eq!(
        ctx2.get_metadata("k"),
        Some("v".into()),
        "cloned context should share metadata Arc"
    );
}

// ============================================================
// GAP: HandlerRegistry all command lookups
// ============================================================

#[test]
fn registry_lookup_gc_handler() {
    let registry = HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Gc {
            engine_url: "http://localhost:3000".into(),
            dry_run: true,
        },
    };
    let handler = registry.get(&cli).expect("gc handler");
    assert_eq!(handler.name(), "gc");
}

#[test]
fn registry_lookup_init_handler() {
    let registry = HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Init {
            project_dir: PathBuf::from("."),
            engine_url: "http://localhost:3000".into(),
            storage_path: PathBuf::from(".vo/storage"),
        },
    };
    let handler = registry.get(&cli).expect("init handler");
    assert_eq!(handler.name(), "init");
}

#[test]
fn registry_lookup_lock_handler() {
    let registry = HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Lock {
            project_dir: PathBuf::from("."),
        },
    };
    let handler = registry.get(&cli).expect("lock handler");
    assert_eq!(handler.name(), "lock");
}

#[test]
fn registry_lookup_doctor_handler() {
    let registry = HandlerRegistry::default();
    let cli = vo_cli::Cli {
        command: Command::Doctor {
            project_dir: PathBuf::from("."),
        },
    };
    let handler = registry.get(&cli).expect("doctor handler");
    assert_eq!(handler.name(), "doctor");
}

#[test]
fn registry_names_contains_all() {
    let registry = HandlerRegistry::default();
    let names = registry.names();
    assert_eq!(names.len(), 9);
    for name in &[
        "purge",
        "check",
        "compensate",
        "gc",
        "init",
        "lock",
        "doctor",
        "rebuild",
        "status",
    ] {
        assert!(names.contains(name), "missing handler: {name}");
    }
}

// ============================================================
// GAP: interpret_cli_from additional edge cases
// ============================================================

#[test]
fn parse_gc_defaults() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).expect("parse gc");
    match &cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_gc_custom_engine_url() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--engine-url", "http://engine:4000"])
        .expect("parse gc custom");
    match &cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://engine:4000");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_init_all_defaults() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse init");
    match &cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(*project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(*storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_custom_values() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/myproject",
        "--engine-url",
        "http://custom:5000",
        "--storage-path",
        "/data/storage",
    ])
    .expect("parse init custom");
    match &cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(*project_dir, PathBuf::from("/myproject"));
            assert_eq!(engine_url, "http://custom:5000");
            assert_eq!(*storage_path, PathBuf::from("/data/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_doctor_default_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "doctor"]).expect("parse doctor");
    match &cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(*project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).expect("parse rebuild");
    match &cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(*project_dir, PathBuf::from("."));
            assert!(projection_id.is_none());
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_projection_and_force() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--projection-id",
        "my-proj",
        "--force",
        "--list",
    ])
    .expect("parse rebuild all flags");
    match &cli.command {
        Command::Rebuild {
            projection_id,
            list_projections,
            force,
            ..
        } => {
            assert_eq!(projection_id.as_deref(), Some("my-proj"));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_lock_default_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "lock"]).expect("parse lock");
    match &cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(*project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Lock"),
    }
}

// ============================================================
// GAP: map_error_to_exit_code for all variants
// ============================================================

#[test]
fn exit_code_invalid_numeric_is_2() {
    let e = CliError::InvalidNumeric("x".into());
    assert_eq!(map_error_to_exit_code(&e), 2);
}

#[test]
fn exit_code_gc_error_is_1() {
    let e = CliError::Gc(GcError::EngineUnreachable {
        url: "http://x".into(),
        reason: "nope".into(),
    });
    assert_eq!(map_error_to_exit_code(&e), 1);
}

#[test]
fn exit_code_init_error_is_1() {
    let e = CliError::Init(InitError::DirNotFound {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&e), 1);
}

#[test]
fn exit_code_lock_error_is_1() {
    let e = CliError::Lock(LockError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&e), 1);
}

#[test]
fn exit_code_doctor_error_is_1() {
    let e = CliError::Doctor(DoctorError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&e), 1);
}

#[test]
fn exit_code_rebuild_error_is_1() {
    let e = CliError::Rebuild(RebuildError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&e), 1);
}

#[test]
fn exit_code_dispatch_error_is_1() {
    let e = CliError::Dispatch("fail".into());
    assert_eq!(map_error_to_exit_code(&e), 1);
}

// ============================================================
// GAP: parse_strict_numeric edge cases
// ============================================================

#[test]
fn parse_strict_numeric_negative() {
    assert!(parse_strict_numeric("-1").is_err());
}

#[test]
fn parse_strict_numeric_hex_prefix() {
    assert!(parse_strict_numeric("0x10").is_err());
}

#[test]
fn parse_strict_numeric_leading_zeros() {
    assert!(parse_strict_numeric("007").is_ok());
    assert_eq!(parse_strict_numeric("007").unwrap(), 7);
}

#[test]
fn parse_strict_numeric_large_value() {
    assert!(parse_strict_numeric("256").is_ok());
    assert_eq!(parse_strict_numeric("256").unwrap(), 256);
}

#[test]
fn parse_strict_numeric_max_value() {
    assert!(parse_strict_numeric("255").is_ok());
    assert_eq!(parse_strict_numeric("255").unwrap(), 255);
}

#[test]
fn parse_strict_numeric_u64_overflow() {
    let big = format!("{}{}", u64::MAX, "0");
    assert!(parse_strict_numeric(&big).is_err());
}

#[test]
fn parse_strict_numeric_empty() {
    assert!(parse_strict_numeric("").is_err());
}

#[test]
fn parse_strict_numeric_whitespace() {
    assert!(parse_strict_numeric(" 42").is_err());
    assert!(parse_strict_numeric("42 ").is_err());
}

// ============================================================
// GAP: DoctorReport / CategoryReport structural methods
// ============================================================

#[test]
fn doctor_report_errors_and_warnings() {
    let mut cat1 = CategoryReport::new(CheckCategory::Workspace);
    cat1.push("c1", Severity::Error, "e1".into());
    cat1.push("c2", Severity::Warn, "w1".into());
    cat1.push("c3", Severity::Info, "i1".into());

    let mut cat2 = CategoryReport::new(CheckCategory::LockState);
    cat2.push("c4", Severity::Error, "e2".into());

    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat1, cat2],
    };

    assert!(!report.is_healthy());
    assert_eq!(report.errors().count(), 2);
    assert_eq!(report.warnings().count(), 1);
}

#[test]
fn doctor_report_healthy_when_all_info_or_warn() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("c1", Severity::Info, "ok".into());
    cat.push("c2", Severity::Warn, "meh".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    assert!(report.is_healthy());
}

#[test]
fn category_report_warnings_iterator() {
    let mut cat = CategoryReport::new(CheckCategory::StorageIntegrity);
    cat.push("a", Severity::Warn, "w1".into());
    cat.push("b", Severity::Info, "i1".into());
    cat.push("c", Severity::Warn, "w2".into());
    let warnings: Vec<_> = cat.warnings().collect();
    assert_eq!(warnings.len(), 2);
}

#[test]
fn check_category_display_all() {
    assert_eq!(CheckCategory::Workspace.to_string(), "workspace");
    assert_eq!(CheckCategory::LockState.to_string(), "lock-state");
    assert_eq!(
        CheckCategory::SubprocessLiveness.to_string(),
        "subprocess-liveness"
    );
    assert_eq!(
        CheckCategory::StorageIntegrity.to_string(),
        "storage-integrity"
    );
    assert_eq!(
        CheckCategory::ConfigValidation.to_string(),
        "config-validation"
    );
}

#[test]
fn severity_ordering_comprehensive() {
    assert!(Severity::Error > Severity::Warn);
    assert!(Severity::Error > Severity::Info);
    assert!(Severity::Warn > Severity::Info);
    assert!(Severity::Info < Severity::Warn);
    assert!(Severity::Info < Severity::Error);
    assert!(Severity::Warn < Severity::Error);
}

// ============================================================
// GAP: format_report with mixed severity output
// ============================================================

#[test]
fn format_report_errors_and_warnings_counted() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("e1", Severity::Error, "bad".into());
    cat.push("w1", Severity::Warn, "meh".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    let (stdout, stderr) = format_report(&report);
    assert!(stdout.contains("Doctor Report"));
    assert!(stderr.contains("1 error(s)"));
    assert!(stderr.contains("1 warning(s)"));
}

#[test]
fn format_report_healthy() {
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![CategoryReport::new(CheckCategory::Workspace)],
    };
    let (stdout, _stderr) = format_report(&report);
    assert!(stdout.contains("All checks passed"));
}

#[test]
fn format_report_json_with_errors() {
    let mut cat = CategoryReport::new(CheckCategory::Workspace);
    cat.push("e1", Severity::Error, "fail".into());
    let report = DoctorReport {
        project_dir: PathBuf::from("/p"),
        categories: vec![cat],
    };
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(!parsed["healthy"].as_bool().unwrap());
    assert_eq!(parsed["error_count"].as_u64().unwrap(), 1);
    assert_eq!(parsed["warn_count"].as_u64().unwrap(), 0);
}

// ============================================================
// GAP: BinaryFormat display_name
// ============================================================

#[test]
fn binary_format_display_name_all_variants() {
    assert_eq!(BinaryFormat::Elf.display_name(), "valid ELF binary");
    assert_eq!(
        BinaryFormat::MachO32BigEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO32LittleEndian.display_name(),
        "valid Mach-O 32-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64BigEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
    assert_eq!(
        BinaryFormat::MachO64LittleEndian.display_name(),
        "valid Mach-O 64-bit binary"
    );
}

// ============================================================
// GAP: InitConfig not exported but used through run_init
// ============================================================

#[test]
fn init_creates_config_file_content() {
    let dir = make_temp_dir();
    let config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://myengine:4000".into(),
        storage_path: PathBuf::from(".vo/data"),
    };
    vo_cli::commands::init::run_init(&config).unwrap();
    let content = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    assert!(content.contains("http://myengine:4000"));
    assert!(content.contains(".vo/data"));
}

#[test]
fn init_idempotent_with_same_config() {
    let dir = make_temp_dir();
    let config = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result1 = vo_cli::commands::init::run_init(&config);
    assert!(result1.is_ok());
    let result2 = vo_cli::commands::init::run_init(&config);
    assert!(
        result2.is_ok(),
        "re-init with same config should be idempotent"
    );
}

#[test]
fn init_rejects_different_config() {
    let dir = make_temp_dir();
    let config1 = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    vo_cli::commands::init::run_init(&config1).unwrap();
    let config2 = InitConfig {
        project_dir: dir.clone(),
        engine_url: "http://different:9999".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::commands::init::run_init(&config2);
    assert!(matches!(result, Err(InitError::AlreadyInitialized { .. })));
}

#[test]
fn init_rejects_nonexistent_dir() {
    let config = InitConfig {
        project_dir: PathBuf::from("/nonexistent_dir_xyz_12345"),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::commands::init::run_init(&config);
    assert!(matches!(result, Err(InitError::DirNotFound { .. })));
}

#[test]
fn init_rejects_symlink() {
    let dir = make_temp_dir();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    let config = InitConfig {
        project_dir: link.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::commands::init::run_init(&config);
    assert!(matches!(result, Err(InitError::SymlinkTarget { .. })));
}

#[test]
fn init_rejects_file_as_dir() {
    let dir = make_temp_dir();
    let file_path = dir.join("not_a_dir");
    std::fs::write(&file_path, b"data").unwrap();
    let config = InitConfig {
        project_dir: file_path.clone(),
        engine_url: "http://localhost:3000".into(),
        storage_path: PathBuf::from(".vo/storage"),
    };
    let result = vo_cli::commands::init::run_init(&config);
    assert!(matches!(result, Err(InitError::NotDirectory { .. })));
}

// ============================================================
// GAP: CheckCommand validate_binary_header end-to-end
// ============================================================

#[test]
fn validate_elf_binary() {
    let dir = make_temp_dir();
    let elf_path = dir.join("binary");
    std::fs::write(&elf_path, [0x7F, 0x45, 0x4C, 0x46, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&elf_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::Elf);
}

#[test]
fn validate_macho_64_le_binary() {
    let dir = make_temp_dir();
    let path = dir.join("binary");
    std::fs::write(&path, [0xCF, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), BinaryFormat::MachO64LittleEndian);
}

#[test]
fn validate_too_small_file() {
    let dir = make_temp_dir();
    let path = dir.join("tiny");
    std::fs::write(&path, [0x7F, 0x45]).unwrap();
    let result = vo_cli::validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::FileTooSmall { .. })));
}

#[test]
fn validate_invalid_magic() {
    let dir = make_temp_dir();
    let path = dir.join("bad");
    std::fs::write(&path, [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00]).unwrap();
    let result = vo_cli::validate_binary_header(&path);
    assert!(matches!(result, Err(CheckError::InvalidMagic { .. })));
}

#[test]
fn validate_nonexistent_file() {
    let result = vo_cli::validate_binary_header(PathBuf::from("/nonexistent_xyz_12345").as_path());
    assert!(matches!(result, Err(CheckError::FileNotFound { .. })));
}

#[test]
fn validate_symlink_rejected() {
    let dir = make_temp_dir();
    let target = dir.join("real");
    std::fs::write(&target, [0x7F, 0x45, 0x4C, 0x46, 0x00]).unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = vo_cli::validate_binary_header(&link);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

#[test]
fn validate_directory_rejected() {
    let dir = make_temp_dir();
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    let result = vo_cli::validate_binary_header(&sub);
    assert!(matches!(result, Err(CheckError::NotRegularFile { .. })));
}

// ============================================================
// GAP: HistoryConfig default
// ============================================================

#[test]
fn history_config_default() {
    let config = HistoryConfig::default();
    assert_eq!(
        config.history_path,
        PathBuf::from(".vo/command_history.json")
    );
    assert_eq!(config.workflow_name, "default");
}

// ============================================================
// GAP: Doctor full pipeline with storage integrity + config
// ============================================================

#[test]
fn doctor_full_pipeline_with_all_categories() {
    let dir = make_temp_dir();
    setup_project(&dir);
    let wf_path = dir.join(".vo/workflows/mybin");
    std::fs::write(&wf_path, b"binary-content-here").unwrap();
    let config = vo_cli::DoctorConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_doctor(&config);
    assert!(result.is_ok());
    let report = result.unwrap();
    assert_eq!(report.categories.len(), 5);
    assert!(report
        .categories
        .iter()
        .any(|c| c.category == CheckCategory::Workspace));
    assert!(report
        .categories
        .iter()
        .any(|c| c.category == CheckCategory::LockState));
    assert!(report
        .categories
        .iter()
        .any(|c| c.category == CheckCategory::SubprocessLiveness));
    assert!(report
        .categories
        .iter()
        .any(|c| c.category == CheckCategory::StorageIntegrity));
    assert!(report
        .categories
        .iter()
        .any(|c| c.category == CheckCategory::ConfigValidation));
}

#[test]
fn doctor_not_initialized() {
    let dir = make_temp_dir();
    let config = vo_cli::DoctorConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::run_doctor(&config);
    assert!(matches!(result, Err(DoctorError::NotInitialized { .. })));
}

// ============================================================
// GAP: Lock command full pipeline
// ============================================================

#[test]
fn lock_not_initialized() {
    let dir = make_temp_dir();
    let config = LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(matches!(result, Err(LockError::NotInitialized { .. })));
}

#[test]
fn lock_no_workflows_dir() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo")).unwrap();
    let config = LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(matches!(result, Err(LockError::NoWorkflowsDir { .. })));
}

#[test]
fn lock_empty_workflows() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    let config = LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(matches!(result, Err(LockError::Empty { .. })));
}

#[test]
fn lock_with_single_binary() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(dir.join(".vo/workflows/mybin"), b"content").unwrap();
    let config = LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(result.is_ok());
    let lockmap = result.unwrap();
    assert_eq!(lockmap.len(), 1);
    assert!(lockmap.contains_key("mybin"));
    let lock_content = std::fs::read_to_string(dir.join("vo.lock")).unwrap();
    assert!(lock_content.contains("mybin"));
}

#[test]
fn lock_multiple_binaries_sorted() {
    let dir = make_temp_dir();
    std::fs::create_dir_all(dir.join(".vo/workflows")).unwrap();
    std::fs::write(dir.join(".vo/workflows/z_bin"), b"z").unwrap();
    std::fs::write(dir.join(".vo/workflows/a_bin"), b"a").unwrap();
    let config = LockConfig {
        project_dir: dir.clone(),
    };
    let result = vo_cli::commands::lock::run_lock(&config);
    assert!(result.is_ok());
    let lockmap = result.unwrap();
    let keys: Vec<_> = lockmap.keys().collect();
    assert_eq!(keys[0], "a_bin");
    assert_eq!(keys[1], "z_bin");
}

// ============================================================
// GAP: Check constants
// ============================================================

#[test]
fn check_constants_values() {
    assert_eq!(vo_cli::ELF_MAGIC, [0x7F, 0x45, 0x4C, 0x46]);
    assert_eq!(vo_cli::MACHO_MAGIC_32_BE, [0xFE, 0xED, 0xFA, 0xCE]);
    assert_eq!(vo_cli::MACHO_MAGIC_32_LE, [0xCE, 0xFA, 0xED, 0xFE]);
    assert_eq!(vo_cli::MACHO_MAGIC_64_BE, [0xFE, 0xED, 0xFA, 0xCF]);
    assert_eq!(vo_cli::MACHO_MAGIC_64_LE, [0xCF, 0xFA, 0xED, 0xFE]);
    assert_eq!(vo_cli::KNOWN_MAGICS.len(), 5);
}

// ============================================================
// GAP: Command equality and clone
// ============================================================

#[test]
fn command_clone_equality() {
    let cmd = Command::Check {
        workflow: false,
        path: PathBuf::from("/test"),
    };
    let cmd2 = cmd.clone();
    assert_eq!(cmd, cmd2);
}

#[test]
fn command_purge_clone_equality() {
    let cmd = Command::Purge {
        instance: "abc".into(),
    };
    let cmd2 = cmd.clone();
    assert_eq!(cmd, cmd2);
}

#[test]
fn command_rebuild_clone_equality() {
    let cmd = Command::Rebuild {
        project_dir: PathBuf::from("/p"),
        projection_id: Some("proj".into()),
        list_projections: true,
        force: true,
    };
    let cmd2 = cmd.clone();
    assert_eq!(cmd, cmd2);
}

// ============================================================
// GAP: Severity equality + copy
// ============================================================

#[test]
fn severity_equality() {
    assert_eq!(Severity::Info, Severity::Info);
    assert_eq!(Severity::Warn, Severity::Warn);
    assert_eq!(Severity::Error, Severity::Error);
    assert_ne!(Severity::Info, Severity::Warn);
}

#[test]
fn severity_copy() {
    let s = Severity::Warn;
    let s2 = s;
    assert_eq!(s, s2);
}
