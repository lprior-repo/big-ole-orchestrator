use serde::{Deserialize, Serialize};

use crate::workspace::workspace_id::WorkspaceId;
use crate::workspace::workspace_name::WorkspaceName;
use crate::workspace::workspace_path::WorkspacePath;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum WorkspaceIndexError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(WorkspaceId),

    #[error("path not found: {0}")]
    PathNotFound(WorkspacePath),

    #[error("parent not found: {0}")]
    ParentNotFound(WorkspaceId),

    #[error(
        "cyclic move detected: workspace {workspace_id} cannot become child of {attempted_parent}"
    )]
    CyclicMoveDetected {
        workspace_id: WorkspaceId,
        attempted_parent: WorkspaceId,
    },

    #[error("duplicate path: {0}")]
    DuplicatePath(WorkspacePath),

    #[error("duplicate name under parent {parent_id}: {name}")]
    DuplicateName {
        parent_id: WorkspaceId,
        name: WorkspaceName,
    },

    #[error("cannot delete workspace {workspace_id} with {instance_count} active instances")]
    CannotDeleteWorkspaceWithInstances {
        workspace_id: WorkspaceId,
        instance_count: u32,
    },

    #[error("cannot delete workspace {workspace_id} with {child_count} children")]
    CannotDeleteWorkspaceWithChildren {
        workspace_id: WorkspaceId,
        child_count: u32,
    },

    #[error("invalid workspace name: {0}")]
    InvalidWorkspaceName(String),

    #[error("empty path segment")]
    EmptyPathSegment,

    #[error("path too deep: max {max_depth}, got {actual_depth}")]
    PathTooDeep { max_depth: u32, actual_depth: u32 },

    #[error("metadata key too long: max {max_length}, got {actual_length}")]
    MetadataKeyTooLong {
        max_length: usize,
        actual_length: usize,
    },

    #[error("metadata value too long: max {max_length}, got {actual_length}")]
    MetadataValueTooLong {
        max_length: usize,
        actual_length: usize,
    },

    #[error("too many metadata entries: max {max}, got {actual}")]
    TooManyMetadataEntries { max: usize, actual: usize },

    #[error("index not initialized")]
    IndexNotInitialized,

    #[error("snapshot corrupted: expected checksum {expected_checksum}, got {actual_checksum}")]
    SnapshotCorrupted {
        expected_checksum: u64,
        actual_checksum: u64,
    },

    #[error("version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u64, actual: u64 },

    #[error("storage write failed: {0}")]
    StorageWriteFailed(String),

    #[error("storage read failed: {0}")]
    StorageReadFailed(String),
}
