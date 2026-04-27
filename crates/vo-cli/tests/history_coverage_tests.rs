use std::path::PathBuf;
use vo_cli::commands::history::{
    get_history, load_history, redo_command, save_history, undo_command, HistoryConfig,
    HistoryError, HistoryOutput,
};
use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

fn test_snapshot() -> WorkflowSnapshot {
    WorkflowSnapshot::new(
        "test-wf".into(),
        vec![DagNode {
            compensation_policy: None,
            node_name: NodeName::parse("node-a").unwrap(),
            retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        }],
        vec![],
    )
}

#[test]
fn history_error_read_failed_display() {
    let err = HistoryError::ReadFailed {
        reason: "disk error".to_string(),
    };
    assert!(err.to_string().contains("disk error"));
    assert!(err.to_string().contains("read"));
}

#[test]
fn history_error_write_failed_display() {
    let err = HistoryError::WriteFailed {
        reason: "no space".to_string(),
    };
    assert!(err.to_string().contains("no space"));
    assert!(err.to_string().contains("write"));
}

#[test]
fn history_error_invalid_format_display() {
    let err = HistoryError::InvalidFormat {
        reason: "expected array".to_string(),
    };
    assert!(err.to_string().contains("expected array"));
    assert!(err.to_string().contains("format"));
}

#[test]
fn history_error_not_found_display() {
    let err = HistoryError::HistoryFileNotFound {
        path: PathBuf::from("/tmp/missing.json"),
    };
    let msg = err.to_string();
    assert!(msg.contains("not found"));
    assert!(msg.contains("/tmp/missing.json"));
}

#[test]
fn history_config_default_values() {
    let config = HistoryConfig::default();
    assert_eq!(
        config.history_path,
        PathBuf::from(".vo/command_history.json")
    );
    assert_eq!(config.workflow_name, "default");
}

#[test]
fn load_history_missing_file_returns_empty() {
    let path = PathBuf::from("/tmp/vo-cli-history-test-nonexistent.json");
    let history = load_history(&path).unwrap();
    assert!(history.entries().is_empty());
}

#[test]
fn save_and_load_preserves_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");
    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    history
        .save_undo_point(CommandKind::NodeDelete, test_snapshot())
        .unwrap();

    save_history(&path, &history).unwrap();
    let loaded = load_history(&path).unwrap();
    assert_eq!(loaded.entries().len(), 2);
}

#[test]
fn load_history_invalid_json_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not valid json{{{").unwrap();
    let result = load_history(&path);
    assert!(result.is_err());
    match result.unwrap_err() {
        HistoryError::InvalidFormat { .. } => {}
        other => panic!("expected InvalidFormat, got {other}"),
    }
}

#[test]
fn undo_with_entries_succeeds() {
    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let result = undo_command(&mut history);
    assert!(result.success);
    assert!(result.message.contains("Undo"));
}

#[test]
fn redo_after_undo_succeeds() {
    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    undo_command(&mut history);
    let result = redo_command(&mut history);
    assert!(result.success);
    assert!(result.message.contains("Redo"));
}

#[test]
fn get_history_empty_shows_zero_depths() {
    let history = CommandHistory::new();
    let output = get_history(&history);
    assert_eq!(output.undo_stack_depth, 0);
    assert_eq!(output.redo_stack_depth, 0);
    assert!(!output.can_undo);
    assert!(!output.can_redo);
}

#[test]
fn get_history_after_save_has_entry() {
    let mut history = CommandHistory::new();
    history
        .save_undo_point(CommandKind::NodeCreate, test_snapshot())
        .unwrap();
    let output = get_history(&history);
    assert_eq!(output.entries.len(), 1);
    assert!(!output.entries[0].command_id.is_empty());
}

#[test]
fn save_history_creates_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("deep").join("history.json");
    let history = CommandHistory::new();
    save_history(&path, &history).unwrap();
    assert!(path.exists());
}
