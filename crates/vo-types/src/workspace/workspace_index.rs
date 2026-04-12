use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::TimestampMs;
use crate::workspace::workspace_id::WorkspaceId;
use crate::workspace::workspace_index_error::WorkspaceIndexError;
use crate::workspace::workspace_metadata::WorkspaceMetadata;
use crate::workspace::workspace_name::WorkspaceName;
use crate::workspace::workspace_node::WorkspaceNode;
use crate::workspace::workspace_path::WorkspacePath;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceIndex {
    pub nodes: BTreeMap<WorkspaceId, WorkspaceNode>,
    pub root_ids: Vec<WorkspaceId>,
    pub path_index: BTreeMap<WorkspacePath, WorkspaceId>,
    pub version: u64,
    pub initialized: bool,
}

impl Default for WorkspaceIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceIndex {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            root_ids: Vec::new(),
            path_index: BTreeMap::new(),
            version: 0,
            initialized: true,
        }
    }

    pub fn insert(
        &mut self,
        parent_id: Option<WorkspaceId>,
        name: WorkspaceName,
        metadata: WorkspaceMetadata,
        now: TimestampMs,
    ) -> Result<WorkspaceId, WorkspaceIndexError> {
        let _ = (parent_id, name, metadata, now);
        todo!("TDD Red: implementation pending")
    }

    pub fn delete(&mut self, id: WorkspaceId) -> Result<(), WorkspaceIndexError> {
        let _ = id;
        todo!("TDD Red: implementation pending")
    }

    pub fn move_workspace(
        &mut self,
        id: WorkspaceId,
        new_parent_id: Option<WorkspaceId>,
        now: TimestampMs,
    ) -> Result<(), WorkspaceIndexError> {
        let _ = (id, new_parent_id, now);
        todo!("TDD Red: implementation pending")
    }

    pub fn update_metadata(
        &mut self,
        id: WorkspaceId,
        metadata: WorkspaceMetadata,
        now: TimestampMs,
    ) -> Result<(), WorkspaceIndexError> {
        let _ = (id, metadata, now);
        todo!("TDD Red: implementation pending")
    }

    pub fn find_by_path(&self, path: &WorkspacePath) -> Result<WorkspaceId, WorkspaceIndexError> {
        let _ = path;
        todo!("TDD Red: implementation pending")
    }

    pub fn find_by_id(&self, id: WorkspaceId) -> Result<WorkspaceNode, WorkspaceIndexError> {
        let _ = id;
        todo!("TDD Red: implementation pending")
    }

    pub fn list_children(&self, id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let _ = id;
        todo!("TDD Red: implementation pending")
    }

    pub fn get_ancestors(&self, id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let _ = id;
        todo!("TDD Red: implementation pending")
    }

    pub fn get_descendants(
        &self,
        id: WorkspaceId,
    ) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let _ = id;
        todo!("TDD Red: implementation pending")
    }

    pub fn verify_invariants(&self) -> Result<(), WorkspaceIndexError> {
        todo!("TDD Red: implementation pending")
    }
}
