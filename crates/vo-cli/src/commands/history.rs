use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntryOutput {
    pub command_id: String,
    pub kind: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryOutput {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_stack_depth: usize,
    pub redo_stack_depth: usize,
    pub entries: Vec<HistoryEntryOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedoResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("history file not found: {path}")]
    HistoryFileNotFound { path: PathBuf },

    #[error("failed to read history: {reason}")]
    ReadFailed { reason: String },

    #[error("failed to write history: {reason}")]
    WriteFailed { reason: String },

    #[error("invalid history format: {reason}")]
    InvalidFormat { reason: String },
}

pub struct HistoryConfig {
    pub history_path: PathBuf,
    pub workflow_name: String,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            history_path: PathBuf::from(".vo/command_history.json"),
            workflow_name: "default".to_string(),
        }
    }
}

pub fn load_history(
    path: &PathBuf,
) -> Result<vo_types::command_history::CommandHistory, HistoryError> {
    if !path.exists() {
        return Ok(vo_types::command_history::CommandHistory::new());
    }

    let content = std::fs::read_to_string(path).map_err(|e| HistoryError::ReadFailed {
        reason: e.to_string(),
    })?;

    serde_json::from_str(&content).map_err(|e| HistoryError::InvalidFormat {
        reason: e.to_string(),
    })
}

pub fn save_history(
    path: &PathBuf,
    history: &vo_types::command_history::CommandHistory,
) -> Result<(), HistoryError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| HistoryError::WriteFailed {
            reason: format!("failed to create directory: {}", e),
        })?;
    }

    let content = serde_json::to_string_pretty(history).map_err(|e| HistoryError::WriteFailed {
        reason: e.to_string(),
    })?;

    std::fs::write(path, content).map_err(|e| HistoryError::WriteFailed {
        reason: e.to_string(),
    })
}

pub fn get_history(history: &vo_types::command_history::CommandHistory) -> HistoryOutput {
    let entries: Vec<HistoryEntryOutput> = history
        .entries()
        .iter()
        .map(|e| HistoryEntryOutput {
            command_id: e.envelope.metadata.command_id.as_str().to_string(),
            kind: format!("{:?}", e.kind),
            status: format!("{}", e.status),
        })
        .collect();

    HistoryOutput {
        can_undo: history.can_undo(),
        can_redo: history.can_redo(),
        undo_stack_depth: history.undo_stack().len(),
        redo_stack_depth: history.redo_stack().len(),
        entries,
    }
}

pub fn undo_command(history: &mut vo_types::command_history::CommandHistory) -> UndoResult {
    match history.undo() {
        Ok(true) => UndoResult {
            success: true,
            message: "Undo successful".to_string(),
        },
        Ok(false) => UndoResult {
            success: false,
            message: "Nothing to undo".to_string(),
        },
        Err(e) => UndoResult {
            success: false,
            message: format!("Undo failed: {}", e),
        },
    }
}

pub fn redo_command(history: &mut vo_types::command_history::CommandHistory) -> RedoResult {
    match history.redo() {
        Ok(true) => RedoResult {
            success: true,
            message: "Redo successful".to_string(),
        },
        Ok(false) => RedoResult {
            success: false,
            message: "Nothing to redo".to_string(),
        },
        Err(e) => RedoResult {
            success: false,
            message: format!("Redo failed: {}", e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::{CommandHistory, CommandKind, DagNode, NodeCapability, NodeKind, NodeName, RetryPolicy, WorkflowSnapshot};

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                node_name: NodeName::parse("test-node").unwrap(),
                retry_policy: RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
                capability: NodeCapability::new(NodeKind::Pure),
            }],
            vec![],
        )
    }

    #[test]
    fn test_history_output_structure() {
        let history = CommandHistory::new();
        let output = get_history(&history);

        assert!(!output.can_undo);
        assert!(!output.can_redo);
        assert_eq!(output.undo_stack_depth, 0);
        assert_eq!(output.redo_stack_depth, 0);
        assert!(output.entries.is_empty());
    }

    #[test]
    fn test_undo_with_empty_history() {
        let mut history = CommandHistory::new();
        let result = undo_command(&mut history);

        assert!(!result.success);
        assert_eq!(result.message, "Nothing to undo");
    }

    #[test]
    fn test_redo_with_empty_history() {
        let mut history = CommandHistory::new();
        let result = redo_command(&mut history);

        assert!(!result.success);
        assert_eq!(result.message, "Nothing to redo");
    }

    #[test]
    fn test_save_and_load_history() {
        let mut history = CommandHistory::new();
        history
            .save_undo_point(CommandKind::NodeCreate, test_snapshot())
            .unwrap();

        let path = PathBuf::from("/tmp/test_history.json");
        save_history(&path, &history).unwrap();

        let loaded = load_history(&path).unwrap();
        assert_eq!(loaded.entries().len(), 1);

        std::fs::remove_file(&path).ok();
    }
}
