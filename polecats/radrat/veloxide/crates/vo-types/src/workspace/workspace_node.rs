use serde::{Deserialize, Serialize};

use crate::types::TimestampMs;
use crate::workspace::workspace_id::WorkspaceId;
use crate::workspace::workspace_metadata::WorkspaceMetadata;
use crate::workspace::workspace_name::WorkspaceName;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceNode {
    pub id: WorkspaceId,
    pub name: WorkspaceName,
    pub parent_id: Option<WorkspaceId>,
    pub children: Vec<WorkspaceId>,
    pub metadata: WorkspaceMetadata,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl WorkspaceNode {
    pub fn new_root(
        id: WorkspaceId,
        name: WorkspaceName,
        metadata: WorkspaceMetadata,
        now: TimestampMs,
    ) -> Self {
        Self {
            id,
            name,
            parent_id: None,
            children: Vec::new(),
            metadata,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_child(
        id: WorkspaceId,
        name: WorkspaceName,
        parent_id: WorkspaceId,
        metadata: WorkspaceMetadata,
        now: TimestampMs,
    ) -> Self {
        Self {
            id,
            name,
            parent_id: Some(parent_id),
            children: Vec::new(),
            metadata,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}
