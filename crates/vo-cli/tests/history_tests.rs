#![allow(clippy::redundant_pattern_matching)]
use std::fs;
use std::path::PathBuf;

use vo_cli::commands::history::{
    get_history, load_history, redo_command, save_history, undo_command, HistoryConfig,
};
use vo_types::{CommandHistory, CommandKind, DagNode, NodeName, RetryPolicy, WorkflowSnapshot};

#[test]
fn history_load_nonexistent_returns_new() {
    let path = PathBuf::from("/tmp/vo-test-noexist-history.json");
    let _ = fs::remove_file(&path);
    let history = load_history(&path).unwrap();
    assert!(history.entries().is_empty());
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn history_save_and_reload_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.json");

    let mut history = CommandHistory::new();
    let snapshot = WorkflowSnapshot::new(
        "wf-test".into(),
        vec![DagNode {
            compensation_policy: None,
            node_name: NodeName::parse("node-1").unwrap(),
            retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
        }],
        vec![],
    );
    history
        .save_undo_point(CommandKind::NodeCreate, snapshot)
        .unwrap();

    save_history(&path, &history).unwrap();
    let loaded = load_history(&path).unwrap();
    assert_eq!(loaded.entries().len(), 1);
}

#[test]
fn history_output_format() {
    let history = CommandHistory::new();
    let output = get_history(&history);
    assert!(!output.can_undo);
    assert!(!output.can_redo);
    assert_eq!(output.undo_stack_depth, 0);
    assert_eq!(output.redo_stack_depth, 0);
    assert!(output.entries.is_empty());
}

#[test]
fn history_undo_empty() {
    let mut history = CommandHistory::new();
    let result = undo_command(&mut history);
    assert!(!result.success);
    assert_eq!(result.message, "Nothing to undo");
}

#[test]
fn history_redo_empty() {
    let mut history = CommandHistory::new();
    let result = redo_command(&mut history);
    assert!(!result.success);
    assert_eq!(result.message, "Nothing to redo");
}

#[test]
fn history_config_default() {
    let config = HistoryConfig::default();
    assert_eq!(
        config.history_path,
        PathBuf::from(".vo/command_history.json")
    );
    assert_eq!(config.workflow_name, "default");
}
