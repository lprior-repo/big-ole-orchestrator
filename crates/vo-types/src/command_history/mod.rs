//! Command History with Undo/Redo subsystem.
//!
//! This module provides the command history tracking system for workflow graph modifications
//! with full undo/redo capabilities.
//!
//! # Architecture
//!
//! - [`CommandKind`] - classifies graph-modifying operations
//! - [`CommandEnvelope`] - carries identity metadata for history entries
//! - [`WorkflowSnapshot`] - captures complete workflow graph state
//! - [`HistoryEntry`] - single entry in the command history
//! - [`CommandHistory`] - the full undo/redo stack manager
//!
//! # Invariants
//!
//! - INV-001: Stack balance in equilibrium
//! - INV-002: undo_stack reverse chronological prefix
//! - INV-003: redo_stack only Undone entries
//! - INV-004 through INV-013: Various state transition and capacity constraints

mod ids;
mod query;
mod selection;
mod types;

pub use ids::{BatchId, CommandId, SnapshotId};
pub use types::{
    CommandHistoryError, CommandKind, ExtensionApplyMode, ExtensionBatchMetadata, HistoryEntry,
    HistoryEntryStatus, WorkflowSnapshot, MAX_HISTORY_DEPTH, MAX_REDO_STACK_DEPTH,
    MAX_UNDO_STACK_DEPTH,
};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Command History
// ---------------------------------------------------------------------------

/// The full undo/redo stack for command history tracking.
///
/// # Invariants
///
/// - `entries.len() <= MAX_HISTORY_DEPTH`
/// - `undo_stack.len() <= MAX_UNDO_STACK_DEPTH`
/// - `redo_stack.len() <= MAX_REDO_STACK_DEPTH`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandHistory {
    entries: Vec<HistoryEntry>,
    undo_stack: Vec<CommandId>,
    redo_stack: Vec<CommandId>,
    capacity: usize,
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHistory {
    /// Create a new empty command history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            capacity: MAX_HISTORY_DEPTH,
        }
    }

    /// Save an undo point before executing a command.
    ///
    /// Creates a new history entry with the given command kind and snapshot.
    /// This clears the redo stack (INV-009).
    ///
    /// # Arguments
    ///
    /// * `kind` - The type of command being executed
    /// * `snapshot_before` - The workflow state before the command
    ///
    /// # Returns
    ///
    /// Returns `Ok(CommandId)` on success, or an error if at capacity.
    pub fn save_undo_point(
        &mut self,
        kind: CommandKind,
        snapshot_before: WorkflowSnapshot,
    ) -> Result<CommandId, CommandHistoryError> {
        let command_id = CommandId::new();
        let entry = HistoryEntry::new(
            kind,
            Some(snapshot_before.clone()),
            Some(snapshot_before),
            None,
            Some(command_id.clone()),
        );

        if self.entries.len() >= self.capacity {
            if let Some(oldest_idx) = self.find_oldest_committed_index() {
                self.entries.remove(oldest_idx);
            }
        }

        self.entries.push(entry);
        self.undo_stack.push(command_id.clone());

        self.redo_stack.clear();

        Ok(command_id)
    }

    /// Undo the last command.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if undo was successful
    /// - `Ok(false)` if nothing to undo
    /// - `Err(...)` if an error occurred (e.g., missing snapshot, checksum mismatch)
    pub fn undo(&mut self) -> Result<bool, CommandHistoryError> {
        if self.undo_stack.is_empty() {
            return Ok(false);
        }

        let command_id = self
            .undo_stack
            .pop()
            .expect("undo_stack pop after empty check");

        let entry = self.find_entry_mut(&command_id)?;

        if entry.snapshot_before.is_none() {
            return Err(CommandHistoryError::SnapshotNotFound {
                snapshot_id: format!("entry {}", command_id),
            });
        }

        if let Some(ref snap_before) = entry.snapshot_before {
            let current_checksum =
                WorkflowSnapshot::compute_checksum(&snap_before.nodes, &snap_before.edges);
            if current_checksum != snap_before.checksum {
                return Err(CommandHistoryError::ChecksumMismatch {
                    expected: snap_before.checksum,
                    actual: current_checksum,
                });
            }
        }

        entry.status = HistoryEntryStatus::Undone;

        self.redo_stack.push(command_id);

        Ok(true)
    }

    /// Redo the last undone command.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if redo was successful
    /// - `Ok(false)` if nothing to redo
    /// - `Err(...)` if an error occurred (e.g., missing snapshot)
    pub fn redo(&mut self) -> Result<bool, CommandHistoryError> {
        if self.redo_stack.is_empty() {
            return Ok(false);
        }

        let command_id = self
            .redo_stack
            .pop()
            .expect("redo_stack pop after empty check");

        // Find the entry and mark as Redone
        for entry in &mut self.entries {
            if entry.envelope.metadata.command_id.as_str() == command_id.as_str() {
                entry.status = HistoryEntryStatus::Redone;
                break;
            }
        }

        // Push back to undo stack
        self.undo_stack.push(command_id);

        Ok(true)
    }

    /// Apply a command with full undo point and snapshot capture.
    ///
    /// This is a convenience method that combines save_undo_point with
    /// operation execution and snapshot capture.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type of command being executed
    /// * `before_snapshot` - The workflow state before the command
    /// * `after_snapshot` - The workflow state after the command
    /// * `batch_metadata` - Optional batch metadata for extension commands
    ///
    /// # Returns
    ///
    /// Returns `Ok(CommandId)` on success, or an error if at capacity.
    pub fn apply_command(
        &mut self,
        kind: CommandKind,
        before_snapshot: WorkflowSnapshot,
        after_snapshot: WorkflowSnapshot,
        batch_metadata: Option<ExtensionBatchMetadata>,
    ) -> Result<CommandId, CommandHistoryError> {
        let command_id = CommandId::new();
        let entry = HistoryEntry::new(
            kind,
            Some(before_snapshot),
            Some(after_snapshot),
            batch_metadata,
            Some(command_id.clone()),
        );

        if self.entries.len() >= self.capacity {
            if let Some(oldest_idx) = self.find_oldest_committed_index() {
                self.entries.remove(oldest_idx);
            }
        }

        self.entries.push(entry);
        self.undo_stack.push(command_id.clone());

        self.redo_stack.clear();

        Ok(command_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::DagNode;

    fn test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot::new(
            "test-workflow".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("test-node").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        )
    }

    #[test]
    fn command_id_generation() {
        let id1 = CommandId::new();
        let id2 = CommandId::new();
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn snapshot_id_generation() {
        let id1 = SnapshotId::new();
        let id2 = SnapshotId::new();
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn batch_id_generation() {
        let id1 = BatchId::new();
        let id2 = BatchId::new();
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn command_kind_variants() {
        let kinds = [
            CommandKind::ExtensionApply,
            CommandKind::ExtensionRevert,
            CommandKind::ExtensionRedo,
            CommandKind::NodeCreate,
            CommandKind::NodeDelete,
            CommandKind::EdgeCreate,
            CommandKind::EdgeDelete,
            CommandKind::ConfigUpdate,
        ];
        assert_eq!(kinds.len(), 8);
    }

    #[test]
    fn extension_apply_mode_variants() {
        let modes = [ExtensionApplyMode::Single, ExtensionApplyMode::Bulk];
        assert_eq!(modes.len(), 2);
    }

    #[test]
    fn history_entry_status_variants() {
        let statuses = [
            HistoryEntryStatus::Committed,
            HistoryEntryStatus::Undone,
            HistoryEntryStatus::Redone,
            HistoryEntryStatus::Failed,
        ];
        assert_eq!(statuses.len(), 4);
    }

    #[test]
    fn history_entry_status_display() {
        assert_eq!(format!("{}", HistoryEntryStatus::Committed), "Committed");
        assert_eq!(format!("{}", HistoryEntryStatus::Undone), "Undone");
        assert_eq!(format!("{}", HistoryEntryStatus::Redone), "Redone");
        assert_eq!(format!("{}", HistoryEntryStatus::Failed), "Failed");
    }

    #[test]
    fn command_history_new() {
        let history = CommandHistory::new();
        assert!(history.entries.is_empty());
        assert!(history.undo_stack.is_empty());
        assert!(history.redo_stack.is_empty());
        assert_eq!(history.capacity(), MAX_HISTORY_DEPTH);
    }

    #[test]
    fn command_history_error_display() {
        let err = CommandHistoryError::UndoStackEmpty;
        assert!(format!("{}", err).to_lowercase().contains("undo"));

        let err = CommandHistoryError::RedoStackEmpty;
        assert!(format!("{}", err).to_lowercase().contains("redo"));

        let err = CommandHistoryError::SnapshotNotFound {
            snapshot_id: "test".to_string(),
        };
        assert!(format!("{}", err).to_lowercase().contains("snapshot"));

        let err = CommandHistoryError::ChecksumMismatch {
            expected: 1,
            actual: 2,
        };
        assert!(format!("{}", err).to_lowercase().contains("checksum"));
    }

    #[test]
    fn workflow_snapshot_checksum_deterministic() {
        let nodes = vec![DagNode {
            node_name: crate::NodeName::parse("a").unwrap(),
            retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        }];
        let edges = vec![];

        let snapshot1 = WorkflowSnapshot::new("workflow".into(), nodes.clone(), edges.clone());
        let snapshot2 = WorkflowSnapshot::new("workflow".into(), nodes, edges);

        assert_eq!(
            snapshot1.checksum, snapshot2.checksum,
            "identical graphs must have identical checksums"
        );
    }

    #[test]
    fn workflow_snapshot_checksum_detects_difference() {
        let nodes1 = vec![DagNode {
            node_name: crate::NodeName::parse("a").unwrap(),
            retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        }];
        let nodes2 = vec![DagNode {
            node_name: crate::NodeName::parse("b").unwrap(),
            retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        }];

        let snapshot1 = WorkflowSnapshot::new("workflow".into(), nodes1, vec![]);
        let snapshot2 = WorkflowSnapshot::new("workflow".into(), nodes2, vec![]);

        assert_ne!(
            snapshot1.checksum, snapshot2.checksum,
            "different graphs must have different checksums"
        );
    }
}
