//! Data structures: WorkflowSnapshot, ExtensionBatchMetadata, HistoryEntry.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::command_metadata::CommandMetadata;
use crate::command_metadata::Issuer;
use crate::types::TimestampMs;
use crate::workflow::{DagNode, Edge};

use super::types::BatchId;
use super::types::CommandHistoryError;
use super::types::CommandId;
use super::types::ExtensionApplyMode;
use super::types::HistoryEntryStatus;
use super::types::SnapshotId;

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
    pub fn compute_checksum(nodes: &[DagNode], edges: &[Edge]) -> u32 {
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
    pub envelope: crate::CommandEnvelope,
    pub kind: super::types::CommandKind,
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
        kind: super::types::CommandKind,
        snapshot_before: Option<WorkflowSnapshot>,
        snapshot_after: Option<WorkflowSnapshot>,
        batch_metadata: Option<ExtensionBatchMetadata>,
        command_id: Option<CommandId>,
    ) -> Self {
        let cmd_id = command_id.unwrap_or_default();
        let metadata = CommandMetadata {
            command_id: crate::IdempotencyKey::parse(cmd_id.as_str())
                .expect("IdempotencyKey parsing from String should succeed"),
            correlation_id: crate::IdempotencyKey::parse(&ulid::Ulid::new().to_string())
                .expect("IdempotencyKey parsing from ULID string should succeed"),
            causation_id: crate::IdempotencyKey::parse(&ulid::Ulid::new().to_string())
                .expect("IdempotencyKey parsing from ULID string should succeed"),
            issuer: Issuer::Operator,
            issued_at: TimestampMs::now(),
        };
        let envelope = crate::CommandEnvelope {
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
