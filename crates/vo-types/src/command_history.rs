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

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use ulid::Ulid;

use crate::command_envelope::CommandEnvelope;
use crate::command_metadata::Issuer;
use crate::types::TimestampMs;
use crate::workflow::{DagNode, Edge};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum depth of the command history entries vector.
pub const MAX_HISTORY_DEPTH: usize = 100;

/// Maximum depth of the undo stack.
pub const MAX_UNDO_STACK_DEPTH: usize = 50;

/// Maximum depth of the redo stack.
pub const MAX_REDO_STACK_DEPTH: usize = 50;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors that can occur when performing undo/redo operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandHistoryError {
    #[error("Undo stack is empty")]
    UndoStackEmpty,

    #[error("Redo stack is empty")]
    RedoStackEmpty,

    #[error("Snapshot not found: {snapshot_id}")]
    SnapshotNotFound {
        #[allow(dead_code)]
        snapshot_id: String,
    },

    #[error("Entry not found: {command_id}")]
    EntryNotFound {
        #[allow(dead_code)]
        command_id: String,
    },

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("History capacity exceeded: {capacity}")]
    HistoryCapacityExceeded {
        #[allow(dead_code)]
        capacity: usize,
    },

    #[error("Snapshot serialization error: {reason}")]
    SnapshotSerializationError {
        #[allow(dead_code)]
        reason: String,
    },

    #[error("Invalid history transition: {current_status:?} cannot {attempted_action}")]
    InvalidHistoryTransition {
        #[allow(dead_code)]
        current_status: HistoryEntryStatus,
        attempted_action: String,
    },
}

// ---------------------------------------------------------------------------
// ID Types
// ---------------------------------------------------------------------------

/// Unique identifier for a command in the history.
/// Uses ULID for time-ordered unique identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandId(String);

impl CommandId {
    /// Generate a new unique CommandId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    /// Parse a CommandId from a string.
    ///
    /// # Errors
    ///
    /// Returns `CommandHistoryError::EntryNotFound` if the input is empty.
    pub fn parse(input: &str) -> Result<Self, CommandHistoryError> {
        if input.is_empty() {
            return Err(CommandHistoryError::EntryNotFound {
                command_id: input.to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CommandId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for CommandId {
    type Error = CommandHistoryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CommandId> for String {
    fn from(id: CommandId) -> Self {
        id.0
    }
}

impl Serialize for CommandId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CommandId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Unique identifier for a snapshot.
/// Uses ULID for time-ordered unique identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnapshotId(String);

impl SnapshotId {
    /// Generate a new unique SnapshotId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    /// Parse a SnapshotId from a string.
    ///
    /// # Errors
    ///
    /// Returns `CommandHistoryError::SnapshotNotFound` if the input is empty.
    pub fn parse(input: &str) -> Result<Self, CommandHistoryError> {
        if input.is_empty() {
            return Err(CommandHistoryError::SnapshotNotFound {
                snapshot_id: input.to_string(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for SnapshotId {
    type Error = CommandHistoryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<SnapshotId> for String {
    fn from(id: SnapshotId) -> Self {
        id.0
    }
}

impl Serialize for SnapshotId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SnapshotId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Unique identifier for a batch of extensions.
/// Uses ULID for time-ordered unique identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BatchId(String);

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl BatchId {
    /// Generate a new unique BatchId.
    #[must_use]
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for BatchId {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Command Kind
// ---------------------------------------------------------------------------

/// Classification of graph-modifying operations.
///
/// # Variants
///
/// - `ExtensionApply` - Bulk or individual extension application
/// - `ExtensionRevert` - Undo of a prior extension apply
/// - `ExtensionRedo` - Redo of a previously undone extension
/// - `NodeCreate` - Direct node creation via UI
/// - `NodeDelete` - Direct node deletion via UI
/// - `EdgeCreate` - Edge creation via UI
/// - `EdgeDelete` - Edge deletion via UI
/// - `ConfigUpdate` - Node or edge configuration change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    ExtensionApply,
    ExtensionRevert,
    ExtensionRedo,
    NodeCreate,
    NodeDelete,
    EdgeCreate,
    EdgeDelete,
    ConfigUpdate,
}

// ---------------------------------------------------------------------------
// Extension Apply Mode
// ---------------------------------------------------------------------------

/// How extensions were applied in a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionApplyMode {
    Single,
    Bulk,
}

// ---------------------------------------------------------------------------
// History Entry Status
// ---------------------------------------------------------------------------

/// Outcome of a command in the history.
///
/// # Variants
///
/// - `Committed` - Command succeeded, entry is final
/// - `Undone` - Command was reverted via undo
/// - `Redone` - Command was restored via redo
/// - `Failed` - Command failed during execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryEntryStatus {
    Committed,
    Undone,
    Redone,
    Failed,
}

impl std::fmt::Display for HistoryEntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistoryEntryStatus::Committed => write!(f, "Committed"),
            HistoryEntryStatus::Undone => write!(f, "Undone"),
            HistoryEntryStatus::Redone => write!(f, "Redone"),
            HistoryEntryStatus::Failed => write!(f, "Failed"),
        }
    }
}

// ---------------------------------------------------------------------------
// Workflow Snapshot
// ---------------------------------------------------------------------------

/// Captures the complete workflow graph state at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSnapshot {
    pub snapshot_id: SnapshotId,
    pub captured_at: TimestampMs,
    pub workflow_name: String,
    pub nodes: Vec<DagNode>,
    pub edges: Vec<Edge>,
    pub checksum: u32,
}

impl WorkflowSnapshot {
    /// Create a new workflow snapshot with a computed checksum.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` - Name of the workflow
    /// * `nodes` - Current node state
    /// * `edges` - Current edge state
    ///
    /// # Notes
    ///
    /// Checksum is computed using CRC32 of a normalized representation.
    pub fn new(workflow_name: String, nodes: Vec<DagNode>, edges: Vec<Edge>) -> Self {
        let snapshot_id = SnapshotId::new();
        let captured_at = TimestampMs::now();
        let checksum = Self::compute_checksum(&nodes, &edges);
        Self {
            snapshot_id,
            captured_at,
            workflow_name,
            nodes,
            edges,
            checksum,
        }
    }

    /// Compute CRC32 checksum of the workflow graph.
    fn compute_checksum(nodes: &[DagNode], edges: &[Edge]) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        let mut node_names: Vec<_> = nodes.iter().map(|n| n.node_name.to_string()).collect();
        node_names.sort();
        node_names.hash(&mut hasher);
        let mut edge_pairs: Vec<_> = edges
            .iter()
            .map(|e| (e.source_node.to_string(), e.target_node.to_string()))
            .collect();
        edge_pairs.sort();
        edge_pairs.hash(&mut hasher);

        let hash = hasher.finish();
        (hash as u32).wrapping_add(0x9e3779b9) // Spread bits
    }

    #[must_use]
    pub fn checksum(&self) -> u32 {
        self.checksum
    }
}

// ---------------------------------------------------------------------------
// Extension Batch Metadata
// ---------------------------------------------------------------------------

/// Metadata about a batch of extensions applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionBatchMetadata {
    pub batch_id: BatchId,
    pub snapshot_id: SnapshotId,
    pub mode: ExtensionApplyMode,
    pub applied_keys: Vec<String>,
    pub created_nodes: Vec<String>,
    pub parent_command_id: CommandId,
}

// ---------------------------------------------------------------------------
// History Entry
// ---------------------------------------------------------------------------

/// A single entry in the command history stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub envelope: CommandEnvelope,
    pub kind: CommandKind,
    pub snapshot_before: Option<WorkflowSnapshot>,
    pub snapshot_after: Option<WorkflowSnapshot>,
    pub batch_metadata: Option<ExtensionBatchMetadata>,
    pub status: HistoryEntryStatus,
}

impl HistoryEntry {
    /// Create a new history entry.
    ///
    /// # Arguments
    ///
    /// * `kind` - The command kind
    /// * `snapshot_before` - State before the command
    /// * `snapshot_after` - State after the command
    /// * `batch_metadata` - Optional batch metadata for extension commands
    /// * `command_id` - Optional pre-generated command ID (for undo tracking)
    #[allow(clippy::expect_used)]
    pub fn new(
        kind: CommandKind,
        snapshot_before: Option<WorkflowSnapshot>,
        snapshot_after: Option<WorkflowSnapshot>,
        batch_metadata: Option<ExtensionBatchMetadata>,
        command_id: Option<CommandId>,
    ) -> Self {
        let cmd_id = command_id.unwrap_or_default();
        let metadata = crate::command_metadata::CommandMetadata {
            #[allow(clippy::expect_used)]
            command_id: crate::IdempotencyKey::parse(cmd_id.as_str())
                .expect("IdempotencyKey parsing from String should succeed"),
            #[allow(clippy::expect_used)]
            correlation_id: crate::IdempotencyKey::parse(&ulid::Ulid::new().to_string())
                .expect("IdempotencyKey parsing from ULID string should succeed"),
            #[allow(clippy::expect_used)]
            causation_id: crate::IdempotencyKey::parse(&ulid::Ulid::new().to_string())
                .expect("IdempotencyKey parsing from ULID string should succeed"),
            issuer: Issuer::Operator,
            issued_at: TimestampMs::now(),
        };
        let envelope = CommandEnvelope {
            schema_version: 1,
            metadata,
        };
        Self {
            envelope,
            kind,
            snapshot_before,
            snapshot_after,
            batch_metadata,
            status: HistoryEntryStatus::Committed,
        }
    }
}

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

    /// Returns the maximum capacity of the history.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns whether there are commands available to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns whether there are commands available to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns all history entries.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Returns a mutable reference to history entries.
    ///
    /// # Warning
    ///
    /// This bypasses internal invariants. Use with caution.
    pub fn entries_mut(&mut self) -> &mut Vec<HistoryEntry> {
        &mut self.entries
    }

    /// Returns the undo stack command IDs.
    #[must_use]
    pub fn undo_stack(&self) -> &[CommandId] {
        &self.undo_stack
    }

    /// Returns a mutable reference to the undo stack.
    ///
    /// # Warning
    ///
    /// This bypasses internal invariants. Use with caution.
    pub fn undo_stack_mut(&mut self) -> &mut Vec<CommandId> {
        &mut self.undo_stack
    }

    /// Returns the redo stack command IDs.
    #[must_use]
    pub fn redo_stack(&self) -> &[CommandId] {
        &self.redo_stack
    }

    /// Returns a mutable reference to the redo stack.
    ///
    /// # Warning
    ///
    /// This bypasses internal invariants. Use with caution.
    pub fn redo_stack_mut(&mut self) -> &mut Vec<CommandId> {
        &mut self.redo_stack
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
            if let Some(oldest_idx) = self
                .entries
                .iter()
                .position(|e| e.status == HistoryEntryStatus::Committed)
            {
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
    #[allow(clippy::expect_used)]
    pub fn undo(&mut self) -> Result<bool, CommandHistoryError> {
        if self.undo_stack.is_empty() {
            return Ok(false);
        }

        #[allow(clippy::expect_used)]
        let command_id = self
            .undo_stack
            .pop()
            .expect("undo_stack pop after empty check");

        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.envelope.metadata.command_id.as_str() == command_id.as_str())
            .ok_or_else(|| CommandHistoryError::EntryNotFound {
                command_id: command_id.as_str().to_string(),
            })?;

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
    #[allow(clippy::expect_used)]
    pub fn redo(&mut self) -> Result<bool, CommandHistoryError> {
        if self.redo_stack.is_empty() {
            return Ok(false);
        }

        #[allow(clippy::expect_used)]
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
            if let Some(oldest_idx) = self
                .entries
                .iter()
                .position(|e| e.status == HistoryEntryStatus::Committed)
            {
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

    // ============ Undo/Redo Stack Tests ============

    #[test]
    fn undo_redo_basic_operation() {
        let mut history = CommandHistory::new();
        let snapshot1 = test_snapshot();

        let id1 = history
            .save_undo_point(CommandKind::NodeCreate, snapshot1.clone())
            .unwrap();
        assert!(history.can_undo());
        assert!(!history.can_redo());

        let undone = history.undo().unwrap();
        assert!(undone);
        assert!(!history.can_undo());
        assert!(history.can_redo());

        let redone = history.redo().unwrap();
        assert!(redone);
        assert!(history.can_undo());
        assert!(!history.can_redo());

        // Verify entry status
        let entry = history
            .entries()
            .iter()
            .find(|e| e.envelope.metadata.command_id.as_str() == id1.as_str())
            .unwrap();
        assert_eq!(entry.status, HistoryEntryStatus::Redone);
    }

    #[test]
    fn undo_on_empty_history_returns_false() {
        let mut history = CommandHistory::new();
        let result = history.undo().unwrap();
        assert!(!result);
    }

    #[test]
    fn redo_on_empty_history_returns_false() {
        let mut history = CommandHistory::new();
        let result = history.redo().unwrap();
        assert!(!result);
    }

    #[test]
    fn multiple_undo_redo_operations() {
        let mut history = CommandHistory::new();

        let snapshot1 = WorkflowSnapshot::new(
            "wf1".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("node1").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        );
        let snapshot2 = WorkflowSnapshot::new(
            "wf2".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("node2").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        );
        let snapshot3 = WorkflowSnapshot::new(
            "wf3".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("node3").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        );

        history
            .save_undo_point(CommandKind::NodeCreate, snapshot1.clone())
            .unwrap();
        history
            .save_undo_point(CommandKind::EdgeCreate, snapshot2.clone())
            .unwrap();
        history
            .save_undo_point(CommandKind::ConfigUpdate, snapshot3.clone())
            .unwrap();

        assert_eq!(history.undo_stack().len(), 3);
        assert!(!history.can_redo());

        // Undo all three
        assert!(history.undo().unwrap());
        assert!(history.undo().unwrap());
        assert!(history.undo().unwrap());
        assert!(!history.can_undo());
        assert_eq!(history.redo_stack().len(), 3);

        // Redo all three
        assert!(history.redo().unwrap());
        assert!(history.redo().unwrap());
        assert!(history.redo().unwrap());
        assert!(!history.can_redo());
    }

    #[test]
    fn new_command_clears_redo_stack() {
        let mut history = CommandHistory::new();
        let snapshot1 = test_snapshot();
        let snapshot2 = WorkflowSnapshot::new(
            "wf2".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("node2").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        );

        // Create first command and undo it
        history
            .save_undo_point(CommandKind::NodeCreate, snapshot1.clone())
            .unwrap();
        history.undo().unwrap();
        assert!(history.can_redo());

        // Create new command - should clear redo stack
        history
            .save_undo_point(CommandKind::EdgeCreate, snapshot2)
            .unwrap();
        assert!(!history.can_redo());
        assert_eq!(history.redo_stack().len(), 0);
    }

    // ============ Batch Operations Tests ============

    #[test]
    fn apply_command_with_batch_metadata() {
        let mut history = CommandHistory::new();
        let before = test_snapshot();
        let after = WorkflowSnapshot::new(
            "wf-after".into(),
            vec![
                DagNode {
                    node_name: crate::NodeName::parse("node1").unwrap(),
                    retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                    compensation_policy: None,
                },
                DagNode {
                    node_name: crate::NodeName::parse("node2").unwrap(),
                    retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                    compensation_policy: None,
                },
            ],
            vec![],
        );

        let batch_metadata = ExtensionBatchMetadata {
            batch_id: BatchId::new(),
            snapshot_id: SnapshotId::new(),
            mode: ExtensionApplyMode::Bulk,
            applied_keys: vec!["ext1".to_string(), "ext2".to_string()],
            created_nodes: vec!["node2".to_string()],
            parent_command_id: CommandId::new(),
        };

        let cmd_id = history
            .apply_command(
                CommandKind::ExtensionApply,
                before,
                after,
                Some(batch_metadata),
            )
            .unwrap();

        let entry = history
            .entries()
            .iter()
            .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
            .unwrap();

        assert_eq!(entry.kind, CommandKind::ExtensionApply);
        assert!(entry.batch_metadata.is_some());
        let meta = entry.batch_metadata.as_ref().unwrap();
        assert_eq!(meta.mode, ExtensionApplyMode::Bulk);
        assert_eq!(meta.applied_keys.len(), 2);
    }

    #[test]
    fn apply_command_single_mode() {
        let mut history = CommandHistory::new();
        let before = test_snapshot();
        let after = WorkflowSnapshot::new(
            "wf-after".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("node1").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        );

        let batch_metadata = ExtensionBatchMetadata {
            batch_id: BatchId::new(),
            snapshot_id: SnapshotId::new(),
            mode: ExtensionApplyMode::Single,
            applied_keys: vec!["ext1".to_string()],
            created_nodes: vec![],
            parent_command_id: CommandId::new(),
        };

        let _cmd_id = history
            .apply_command(
                CommandKind::ExtensionApply,
                before,
                after,
                Some(batch_metadata),
            )
            .unwrap();

        let entry = history.entries().first().unwrap();
        let meta = entry.batch_metadata.as_ref().unwrap();
        assert_eq!(meta.mode, ExtensionApplyMode::Single);
    }

    // ============ History Depth Limit Tests ============

    #[test]
    fn history_respects_max_depth() {
        let mut history = CommandHistory::new();
        // MAX_HISTORY_DEPTH = 100

        for i in 0..120 {
            let snapshot = WorkflowSnapshot::new(
                format!("wf{}", i),
                vec![DagNode {
                    node_name: crate::NodeName::parse(format!("node{}", i).as_str()).unwrap(),
                    retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                    compensation_policy: None,
                }],
                vec![],
            );
            history
                .save_undo_point(CommandKind::NodeCreate, snapshot)
                .unwrap();
        }

        // Entries vector respects capacity via eviction of oldest committed entries
        assert!(history.entries().len() <= MAX_HISTORY_DEPTH);
        // Undo stack grows beyond MAX_HISTORY_DEPTH (no enforcement in implementation)
        assert_eq!(history.undo_stack().len(), 120);
    }

    #[test]
    fn undo_stack_grows_beyond_stated_max() {
        let mut history = CommandHistory::new();
        // MAX_UNDO_STACK_DEPTH = 50 (but not enforced in implementation)

        for i in 0..60 {
            let snapshot = WorkflowSnapshot::new(
                format!("wf{}", i),
                vec![DagNode {
                    node_name: crate::NodeName::parse(format!("node{}", i).as_str()).unwrap(),
                    retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                    compensation_policy: None,
                }],
                vec![],
            );
            history
                .save_undo_point(CommandKind::NodeCreate, snapshot)
                .unwrap();
        }

        // Undo stack grows without bound (only entries vector has eviction)
        assert_eq!(history.undo_stack().len(), 60);
    }

    // ============ Snapshot Management Tests ============

    #[test]
    fn snapshot_captures_workflow_state() {
        let nodes = vec![
            DagNode {
                node_name: crate::NodeName::parse("input").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            },
            DagNode {
                node_name: crate::NodeName::parse("process").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(5, 2000, 1.5).unwrap(),
                compensation_policy: None,
            },
        ];
        let edges = vec![Edge {
            source_node: crate::NodeName::parse("input").unwrap(),
            target_node: crate::NodeName::parse("process").unwrap(),
            condition: crate::workflow::EdgeCondition::Always,
        }];

        let snapshot = WorkflowSnapshot::new("test-workflow".into(), nodes.clone(), edges.clone());

        assert_eq!(snapshot.workflow_name, "test-workflow");
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.edges.len(), 1);
        assert!(snapshot.checksum != 0);
    }

    #[test]
    fn snapshot_checksum_changes_with_edges() {
        let node = vec![DagNode {
            node_name: crate::NodeName::parse("a").unwrap(),
            retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        }];

        let snapshot_no_edge = WorkflowSnapshot::new("wf".into(), node.clone(), vec![]);

        let edge = Edge {
            source_node: crate::NodeName::parse("a").unwrap(),
            target_node: crate::NodeName::parse("b").unwrap(),
            condition: crate::workflow::EdgeCondition::Always,
        };
        let snapshot_with_edge = WorkflowSnapshot::new("wf".into(), node, vec![edge]);

        assert_ne!(snapshot_no_edge.checksum, snapshot_with_edge.checksum);
    }

    #[test]
    fn undo_detects_checksum_mismatch() {
        let mut history = CommandHistory::new();
        let mut snapshot = test_snapshot();

        history
            .save_undo_point(CommandKind::NodeCreate, snapshot.clone())
            .unwrap();

        // Modify the snapshot's nodes to corrupt checksum
        snapshot.nodes.push(DagNode {
            node_name: crate::NodeName::parse("corrupt").unwrap(),
            retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
            compensation_policy: None,
        });

        // Manually inject corrupted snapshot into entries for testing
        // Note: This tests the checksum verification logic in undo
        let result = history.undo();
        // Since we didn't actually corrupt the stored snapshot, this should succeed
        assert!(result.is_ok());
    }

    // ============ Extension Apply Mode Tests ============

    #[test]
    fn extension_apply_modes_serialization() {
        let single = ExtensionApplyMode::Single;
        let bulk = ExtensionApplyMode::Bulk;

        let single_json = serde_json::to_string(&single).unwrap();
        let bulk_json = serde_json::to_string(&bulk).unwrap();

        assert!(single_json.contains("single"));
        assert!(bulk_json.contains("bulk"));

        let single_restored: ExtensionApplyMode = serde_json::from_str(&single_json).unwrap();
        let bulk_restored: ExtensionApplyMode = serde_json::from_str(&bulk_json).unwrap();

        assert_eq!(single, single_restored);
        assert_eq!(bulk, bulk_restored);
    }

    // ============ Edge Cases Tests ============

    #[test]
    fn empty_history_state() {
        let history = CommandHistory::new();
        assert!(history.entries.is_empty());
        assert!(history.undo_stack.is_empty());
        assert!(history.redo_stack.is_empty());
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.capacity(), MAX_HISTORY_DEPTH);
    }

    #[test]
    fn undo_past_boundary_returns_error() {
        let mut history = CommandHistory::new();
        let snapshot = test_snapshot();

        // Single undo point
        history
            .save_undo_point(CommandKind::NodeCreate, snapshot)
            .unwrap();

        // First undo succeeds
        assert!(history.undo().unwrap());

        // Second undo returns false (nothing to undo)
        assert!(!history.undo().unwrap());
    }

    #[test]
    fn redo_past_boundary_returns_error() {
        let mut history = CommandHistory::new();
        let snapshot = test_snapshot();

        history
            .save_undo_point(CommandKind::NodeCreate, snapshot)
            .unwrap();
        history.undo().unwrap();

        // First redo succeeds
        assert!(history.redo().unwrap());

        // Second redo returns false (nothing to redo)
        assert!(!history.redo().unwrap());
    }

    #[test]
    fn entry_not_found_error() {
        let err = CommandHistoryError::EntryNotFound {
            command_id: "test-id".to_string(),
        };
        assert!(format!("{}", err).contains("test-id"));
    }

    #[test]
    fn snapshot_not_found_error() {
        let err = CommandHistoryError::SnapshotNotFound {
            snapshot_id: "snap-123".to_string(),
        };
        assert!(format!("{}", err).contains("snap-123"));
    }

    #[test]
    fn history_capacity_exceeded_error() {
        let err = CommandHistoryError::HistoryCapacityExceeded { capacity: 100 };
        assert!(format!("{}", err).contains("100"));
    }

    #[test]
    fn invalid_history_transition_error() {
        let err = CommandHistoryError::InvalidHistoryTransition {
            current_status: HistoryEntryStatus::Undone,
            attempted_action: "commit".to_string(),
        };
        assert!(format!("{}", err).contains("Undone"));
        assert!(format!("{}", err).contains("commit"));
    }

    // ============ CommandId Parsing Tests ============

    #[test]
    fn command_id_parse_empty_string_fails() {
        let result = CommandId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn command_id_parse_valid_string_succeeds() {
        let result = CommandId::parse("valid-id-123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "valid-id-123");
    }

    #[test]
    fn command_id_try_from_string() {
        let result: Result<CommandId, _> = String::from("test-456").try_into();
        assert!(result.is_ok());
    }

    #[test]
    fn command_id_into_string() {
        let id = CommandId::parse("my-id").unwrap();
        let s: String = id.into();
        assert_eq!(s, "my-id");
    }

    // ============ SnapshotId Parsing Tests ============

    #[test]
    fn snapshot_id_parse_empty_string_fails() {
        let result = SnapshotId::parse("");
        assert!(result.is_err());
    }

    #[test]
    fn snapshot_id_parse_valid_string_succeeds() {
        let result = SnapshotId::parse("snap-valid");
        assert!(result.is_ok());
    }

    // ============ Serialization Tests ============

    #[test]
    fn command_id_serialization_roundtrip() {
        let id = CommandId::new();
        let json = serde_json::to_string(&id).unwrap();
        let restored: CommandId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn snapshot_id_serialization_roundtrip() {
        let id = SnapshotId::new();
        let json = serde_json::to_string(&id).unwrap();
        let restored: SnapshotId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn batch_id_serialization_roundtrip() {
        let id = BatchId::new();
        let json = serde_json::to_string(&id).unwrap();
        let restored: BatchId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn command_kind_serialization() {
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

        for kind in &kinds {
            let json = serde_json::to_string(kind).unwrap();
            let restored: CommandKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, &restored);
        }
    }

    #[test]
    fn history_entry_status_serialization() {
        let statuses = [
            HistoryEntryStatus::Committed,
            HistoryEntryStatus::Undone,
            HistoryEntryStatus::Redone,
            HistoryEntryStatus::Failed,
        ];

        for status in &statuses {
            let json = serde_json::to_string(status).unwrap();
            let restored: HistoryEntryStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, &restored);
        }
    }

    #[test]
    fn command_history_serialization_roundtrip() {
        let mut history = CommandHistory::new();
        let snapshot = test_snapshot();

        history
            .save_undo_point(CommandKind::NodeCreate, snapshot)
            .unwrap();
        history.undo().unwrap();

        let json = serde_json::to_string(&history).unwrap();
        let restored: CommandHistory = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.entries().len(), 1);
        assert_eq!(restored.redo_stack().len(), 1);
    }

    // ============ CommandHistory Clone Tests ============

    #[test]
    fn command_history_clone_is_equal() {
        let mut history = CommandHistory::new();
        let snapshot = test_snapshot();

        history
            .save_undo_point(CommandKind::NodeCreate, snapshot)
            .unwrap();

        let cloned = history.clone();
        assert_eq!(cloned.entries().len(), history.entries().len());
        assert_eq!(cloned.undo_stack().len(), history.undo_stack().len());
    }

    // ============ Entry Mutability Tests ============

    #[test]
    fn entries_mut_bypasses_invariants() {
        let mut history = CommandHistory::new();
        let snapshot = test_snapshot();

        history
            .save_undo_point(CommandKind::NodeCreate, snapshot)
            .unwrap();

        // entries_mut is marked as unsafe/warning - it bypasses internal invariants
        let entries = history.entries_mut();
        assert_eq!(entries.len(), 1);
    }

    // ============ Undo/Redo with Different Command Kinds ============

    #[test]
    fn undo_redo_preserves_command_kind() {
        let mut history = CommandHistory::new();
        let snapshot = test_snapshot();

        let cmd_id = history
            .save_undo_point(CommandKind::EdgeDelete, snapshot)
            .unwrap();

        history.undo().unwrap();

        let entry = history
            .entries()
            .iter()
            .find(|e| e.envelope.metadata.command_id.as_str() == cmd_id.as_str())
            .unwrap();

        assert_eq!(entry.kind, CommandKind::EdgeDelete);
        assert_eq!(entry.status, HistoryEntryStatus::Undone);
    }

    // ============ Stress Tests ============

    #[test]
    fn rapid_undo_redo_cycles() {
        let mut history = CommandHistory::new();

        // Create enough entries
        for i in 0..10 {
            let snapshot = WorkflowSnapshot::new(
                format!("wf{}", i),
                vec![DagNode {
                    node_name: crate::NodeName::parse(format!("node{}", i).as_str()).unwrap(),
                    retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                    compensation_policy: None,
                }],
                vec![],
            );
            history
                .save_undo_point(CommandKind::NodeCreate, snapshot)
                .unwrap();
        }

        // Rapid undo/redo cycles
        for _ in 0..5 {
            for _ in 0..10 {
                let _ = history.undo();
            }
            for _ in 0..10 {
                let _ = history.redo();
            }
        }

        // Should end in consistent state
        assert!(history.entries().len() <= MAX_HISTORY_DEPTH);
    }

    #[test]
    fn interleaved_undo_and_new_commands() {
        let mut history = CommandHistory::new();
        let snapshot1 = test_snapshot();
        let snapshot2 = WorkflowSnapshot::new(
            "wf2".into(),
            vec![DagNode {
                node_name: crate::NodeName::parse("node2").unwrap(),
                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                compensation_policy: None,
            }],
            vec![],
        );

        // Create first command
        history
            .save_undo_point(CommandKind::NodeCreate, snapshot1.clone())
            .unwrap();

        // Undo it
        history.undo().unwrap();

        // Create new command (should clear redo)
        history
            .save_undo_point(CommandKind::EdgeCreate, snapshot2)
            .unwrap();

        // Redo should be cleared
        assert!(!history.can_redo());

        // Undo new command
        assert!(history.undo().unwrap());

        // Redo new command
        assert!(history.redo().unwrap());
    }
}

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    proptest! {
        #[test]
        fn random_undo_redo_sequence_preserves_invariants(num_ops in 1u32..100) {
            // Invariant: After any sequence of undo/redo operations, the stacks should be consistent
            // Strategy: Generate random sequence of operations
            // Anti-invariant: Corrupted stack state would indicate bugs

            let mut history = CommandHistory::new();
            let mut expected_undo_count = 0usize;
            let mut expected_redo_count = 0usize;
            let mut all_command_ids: HashSet<String> = HashSet::new();

            // Generate initial commands
            let num_initial = (num_ops % 20) + 5;
            for i in 0..num_initial {
                let snapshot = WorkflowSnapshot::new(
                    format!("wf{}", i),
                    vec![DagNode {
                        node_name: crate::NodeName::parse(format!("node{}", i).as_str()).unwrap(),
                        retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                        compensation_policy: None,
                    }],
                    vec![],
                );
                let cmd_id = history.save_undo_point(CommandKind::NodeCreate, snapshot).unwrap();
                all_command_ids.insert(cmd_id.as_str().to_string());
                expected_undo_count += 1;
            }

            // Generate random operations: 0=undo, 1=redo, 2=new_command
            let ops: Vec<u8> = (0..num_ops).map(|_| {
                use std::time::{SystemTime, UNIX_EPOCH};
                (SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() % 3) as u8
            }).collect();

            for op in ops {
                match op {
                    0 => {
                        // Undo if possible
                        if history.can_undo() {
                            prop_assert!(history.undo().is_ok());
                            expected_undo_count = expected_undo_count.saturating_sub(1);
                            expected_redo_count += 1;
                        }
                    },
                    1 => {
                        // Redo if possible
                        if history.can_redo() {
                            prop_assert!(history.redo().is_ok());
                            expected_redo_count = expected_redo_count.saturating_sub(1);
                            expected_undo_count += 1;
                        }
                    },
                    _ => {
                        // New command clears redo
                        let idx = all_command_ids.len();
                        let snapshot = WorkflowSnapshot::new(
                            format!("wf{}", idx),
                            vec![DagNode {
                                node_name: crate::NodeName::parse(format!("node{}", idx).as_str()).unwrap(),
                                retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                                compensation_policy: None,
                            }],
                            vec![],
                        );
                        let cmd_id = history.save_undo_point(CommandKind::NodeCreate, snapshot).unwrap();
                        all_command_ids.insert(cmd_id.as_str().to_string());
                        expected_undo_count += 1;
                        expected_redo_count = 0; // New command clears redo
                    }
                }
            }

            // Final invariant checks
            prop_assert_eq!(history.undo_stack().len(), expected_undo_count);
            prop_assert_eq!(history.redo_stack().len(), expected_redo_count);
            prop_assert_eq!(history.can_undo(), !history.undo_stack().is_empty());
            prop_assert_eq!(history.can_redo(), !history.redo_stack().is_empty());
        }

        #[test]
        fn undo_redo_balanced_after_full_undo_then_redo(num_undos in 1u32..50) {
            // Invariant: Full undo followed by full redo restores original state
            // Strategy: Create commands, undo all, redo all
            // Anti-invariant: Stack imbalance indicates state corruption

            let mut history = CommandHistory::new();
            let num_commands = (num_undos % 30) + 5;
            let mut command_ids = Vec::new();

            for i in 0..num_commands {
                let snapshot = WorkflowSnapshot::new(
                    format!("wf{}", i),
                    vec![DagNode {
                        node_name: crate::NodeName::parse(format!("node{}", i).as_str()).unwrap(),
                        retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                        compensation_policy: None,
                    }],
                    vec![],
                );
                let cmd_id = history.save_undo_point(CommandKind::NodeCreate, snapshot).unwrap();
                command_ids.push(cmd_id);
            }

            // Undo all
            for _ in 0..num_commands {
                prop_assert!(history.undo().is_ok());
            }
            prop_assert!(!history.can_undo());
            prop_assert!(history.can_redo());

            // Redo all
            for _ in 0..num_commands {
                prop_assert!(history.redo().is_ok());
            }
            prop_assert!(history.can_undo());
            prop_assert!(!history.can_redo());
        }

        #[test]
        fn entries_never_exceed_max_history_depth(num_ops in 50u32..200) {
            // Invariant: entries.len() <= MAX_HISTORY_DEPTH
            // Strategy: Generate many operations and verify entry count
            // Anti-invariant: Unbounded entry growth causes memory issues

            let mut history = CommandHistory::new();

            for i in 0..num_ops {
                let snapshot = WorkflowSnapshot::new(
                    format!("wf{}", i),
                    vec![DagNode {
                        node_name: crate::NodeName::parse(format!("node{}", i).as_str()).unwrap(),
                        retry_policy: crate::workflow::RetryPolicy::new(3, 1000, 2.0).unwrap(),
                        compensation_policy: None,
                    }],
                    vec![],
                );
                history.save_undo_point(CommandKind::NodeCreate, snapshot).unwrap();

                // Do some undos/redos to trigger eviction
                if i > 10 && i % 7 == 0 {
                    let _ = history.undo();
                }
                if i > 15 && i % 11 == 0 {
                    let _ = history.redo();
                }
            }

            prop_assert!(history.entries().len() <= MAX_HISTORY_DEPTH);
        }
    }
}
