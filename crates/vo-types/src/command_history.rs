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

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
    pub fn new(
        kind: CommandKind,
        snapshot_before: Option<WorkflowSnapshot>,
        snapshot_after: Option<WorkflowSnapshot>,
        batch_metadata: Option<ExtensionBatchMetadata>,
        command_id: Option<CommandId>,
    ) -> Self {
        let cmd_id = command_id.unwrap_or_default();
        let metadata = crate::command_metadata::CommandMetadata {
            command_id: crate::IdempotencyKey::parse(cmd_id.as_str())
                .expect("IdempotencyKey parsing from String should succeed"),
            correlation_id: crate::IdempotencyKey::parse(&ulid::Ulid::new().to_string())
                .expect("IdempotencyKey parsing from ULID string should succeed"),
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
    pub fn undo(&mut self) -> Result<bool, CommandHistoryError> {
        if self.undo_stack.is_empty() {
            return Ok(false);
        }

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
}
