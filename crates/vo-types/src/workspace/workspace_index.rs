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
        if !self.initialized {
            return Err(WorkspaceIndexError::IndexNotInitialized);
        }

        metadata.validate()?;

        let path = if let Some(pid) = &parent_id {
            if !self.nodes.contains_key(pid) {
                return Err(WorkspaceIndexError::ParentNotFound(*pid));
            }
            let parent_path = self.compute_path(pid)?;
            parent_path.child(name.clone())?
        } else {
            WorkspacePath::single(name.clone())?
        };

        if self.path_index.contains_key(&path) {
            return Err(WorkspaceIndexError::DuplicatePath(path));
        }

        let id = WorkspaceId::generate();

        let node = if let Some(pid) = parent_id {
            WorkspaceNode::new_child(id, name.clone(), pid, metadata, now)
        } else {
            WorkspaceNode::new_root(id, name.clone(), metadata, now)
        };

        self.nodes.insert(id, node);

        self.path_index.insert(path, id);

        if let Some(pid) = parent_id {
            if let Some(parent_node) = self.nodes.get_mut(&pid) {
                parent_node.children.push(id);
            }
        } else {
            self.root_ids.push(id);
        }

        self.version += 1;

        Ok(id)
    }

    pub fn delete(&mut self, id: WorkspaceId) -> Result<(), WorkspaceIndexError> {
        if !self.initialized {
            return Err(WorkspaceIndexError::IndexNotInitialized);
        }

        let node = self
            .nodes
            .get(&id)
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))?;

        let descendants = self.collect_descendants(id)?;

        let mut all_ids = vec![id];
        all_ids.extend(descendants);

        for &del_id in &all_ids {
            if let Ok(path) = self.compute_path(&del_id) {
                self.path_index.remove(&path);
            }
        }

        if let Some(parent_id) = node.parent_id {
            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                parent_node.children.retain(|&cid| cid != id);
            }
        } else {
            self.root_ids.retain(|&rid| rid != id);
        }

        for &del_id in &all_ids {
            self.nodes.remove(&del_id);
        }

        self.version += 1;

        Ok(())
    }

    #[allow(clippy::expect_used)]
    pub fn move_workspace(
        &mut self,
        id: WorkspaceId,
        new_parent_id: Option<WorkspaceId>,
        now: TimestampMs,
    ) -> Result<(), WorkspaceIndexError> {
        if !self.initialized {
            return Err(WorkspaceIndexError::IndexNotInitialized);
        }

        let (old_parent_id, node_name) = {
            let node = self
                .nodes
                .get(&id)
                .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))?;
            (node.parent_id, node.name.clone())
        };

        if old_parent_id == new_parent_id {
            self.version += 1;
            return Ok(());
        }

        if let Some(npid) = &new_parent_id {
            if *npid == id {
                return Err(WorkspaceIndexError::CyclicMoveDetected {
                    workspace_id: id,
                    attempted_parent: *npid,
                });
            }

            if !self.nodes.contains_key(npid) {
                return Err(WorkspaceIndexError::ParentNotFound(*npid));
            }

            if self.is_descendant(*npid, id) {
                return Err(WorkspaceIndexError::CyclicMoveDetected {
                    workspace_id: id,
                    attempted_parent: *npid,
                });
            }

            if let Some(new_parent) = self.nodes.get(npid) {
                for &child_id in &new_parent.children {
                    if let Some(child_node) = self.nodes.get(&child_id) {
                        if child_node.name == node_name {
                            return Err(WorkspaceIndexError::DuplicateName {
                                parent_id: *npid,
                                name: node_name.clone(),
                            });
                        }
                    }
                }
            }
        }

        let descendants = self.collect_descendants(id)?;

        if let Some(opid) = old_parent_id {
            if let Some(old_parent) = self.nodes.get_mut(&opid) {
                old_parent.children.retain(|&cid| cid != id);
            }
        } else {
            self.root_ids.retain(|&rid| rid != id);
        }

        if let Some(npid) = new_parent_id {
            if let Some(new_parent) = self.nodes.get_mut(&npid) {
                new_parent.children.push(id);
            }
        } else {
            self.root_ids.push(id);
        }

        {
            let new_node = self
                .nodes
                .get_mut(&id)
                .expect("workspace node must exist after move operations");
            new_node.parent_id = new_parent_id;
            new_node.updated_at = now;
        }

        let old_paths: Vec<WorkspacePath> = std::iter::once(id)
            .chain(descendants.iter().copied())
            .filter_map(|nid| self.compute_path(&nid).ok())
            .collect();

        for path in &old_paths {
            self.path_index.remove(path);
        }

        let new_path_segments: Vec<WorkspaceName> = if let Some(npid) = new_parent_id {
            let new_parent_path = self.compute_path(&npid)?;
            let mut segs: Vec<WorkspaceName> = new_parent_path.segments().to_vec();
            segs.push(node_name.clone());
            segs
        } else {
            vec![node_name.clone()]
        };

        let mut current_path_segments = new_path_segments.clone();
        let all_ids: Vec<WorkspaceId> = std::iter::once(id)
            .chain(descendants.iter().copied())
            .collect();

        for (i, desc_id) in all_ids.iter().enumerate() {
            let desc_new_path = WorkspacePath::new(crate::NonEmptyVec::new_unchecked(
                current_path_segments.clone(),
            ))
            .expect("workspace path segments should be non-empty");
            self.path_index.insert(desc_new_path, *desc_id);

            if i < descendants.len() {
                if let Some(desc_node) = self.nodes.get(&descendants[i]) {
                    if !desc_node.children.is_empty() {
                        let child_id = desc_node.children[0];
                        if let Some(child_node) = self.nodes.get(&child_id) {
                            current_path_segments.push(child_node.name.clone());
                        }
                    }
                }
            }
        }

        self.version += 1;

        Ok(())
    }

    fn is_descendant(&self, id: WorkspaceId, ancestor_id: WorkspaceId) -> bool {
        let mut current = self.nodes.get(&id);
        while let Some(node) = current {
            if node.parent_id == Some(ancestor_id) {
                return true;
            }
            current = node.parent_id.and_then(|pid| self.nodes.get(&pid));
        }
        false
    }

    fn collect_descendants(
        &self,
        id: WorkspaceId,
    ) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let mut result = Vec::new();
        let mut stack: Vec<WorkspaceId> = self
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        while let Some(child_id) = stack.pop() {
            result.push(child_id);
            if let Some(child_node) = self.nodes.get(&child_id) {
                for grandchild in &child_node.children {
                    stack.push(*grandchild);
                }
            }
        }

        Ok(result)
    }

    fn compute_path(&self, id: &WorkspaceId) -> Result<WorkspacePath, WorkspaceIndexError> {
        let node = self
            .nodes
            .get(id)
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(*id))?;
        let mut segments = vec![node.name.clone()];

        let mut current_id = node.parent_id;
        while let Some(pid) = current_id {
            let parent = self
                .nodes
                .get(&pid)
                .ok_or(WorkspaceIndexError::WorkspaceNotFound(pid))?;
            segments.insert(0, parent.name.clone());
            current_id = parent.parent_id;
        }

        WorkspacePath::new(crate::NonEmptyVec::new_unchecked(segments))
    }

    pub fn update_metadata(
        &mut self,
        id: WorkspaceId,
        metadata: WorkspaceMetadata,
        now: TimestampMs,
    ) -> Result<(), WorkspaceIndexError> {
        if !self.initialized {
            return Err(WorkspaceIndexError::IndexNotInitialized);
        }

        metadata.validate()?;

        let node = self
            .nodes
            .get_mut(&id)
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))?;
        node.metadata = metadata;
        node.updated_at = now;

        self.version += 1;

        Ok(())
    }

    pub fn find_by_path(&self, path: &WorkspacePath) -> Result<WorkspaceId, WorkspaceIndexError> {
        self.path_index
            .get(path)
            .copied()
            .ok_or_else(|| WorkspaceIndexError::PathNotFound(path.clone()))
    }

    pub fn find_by_id(&self, id: WorkspaceId) -> Result<WorkspaceNode, WorkspaceIndexError> {
        self.nodes
            .get(&id)
            .cloned()
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))
    }

    pub fn list_children(&self, id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let node = self
            .nodes
            .get(&id)
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))?;
        Ok(node.children.clone())
    }

    pub fn get_ancestors(&self, id: WorkspaceId) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let node = self
            .nodes
            .get(&id)
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))?;
        let mut ancestors = Vec::new();
        let mut current = node.parent_id;
        while let Some(pid) = current {
            ancestors.push(pid);
            let parent = self
                .nodes
                .get(&pid)
                .ok_or(WorkspaceIndexError::WorkspaceNotFound(pid))?;
            current = parent.parent_id;
        }
        ancestors.reverse();
        Ok(ancestors)
    }

    pub fn get_descendants(
        &self,
        id: WorkspaceId,
    ) -> Result<Vec<WorkspaceId>, WorkspaceIndexError> {
        let node = self
            .nodes
            .get(&id)
            .ok_or(WorkspaceIndexError::WorkspaceNotFound(id))?;

        let mut result = Vec::new();
        let mut stack: Vec<WorkspaceId> = node.children.clone();

        while let Some(child_id) = stack.pop() {
            result.push(child_id);
            if let Some(child) = self.nodes.get(&child_id) {
                for grandchild in &child.children {
                    stack.push(*grandchild);
                }
            }
        }

        Ok(result)
    }

    pub fn verify_invariants(&self) -> Result<(), WorkspaceIndexError> {
        if !self.initialized {
            return Err(WorkspaceIndexError::IndexNotInitialized);
        }

        for (id, node) in &self.nodes {
            if node.parent_id.is_none() {
                if !self.root_ids.contains(id) {
                    return Err(WorkspaceIndexError::DuplicatePath(WorkspacePath::single(
                        node.name.clone(),
                    )?));
                }
            } else {
                if let Some(parent_id) = node.parent_id {
                    if !self.nodes.contains_key(&parent_id) {
                        return Err(WorkspaceIndexError::ParentNotFound(parent_id));
                    }
                    let parent = self
                        .nodes
                        .get(&parent_id)
                        .ok_or(WorkspaceIndexError::ParentNotFound(parent_id))?;
                    if !parent.children.contains(id) {
                        return Err(WorkspaceIndexError::DuplicatePath(self.compute_path(id)?));
                    }
                }
            }

            for &child_id in &node.children {
                if !self.nodes.contains_key(&child_id) {
                    return Err(WorkspaceIndexError::WorkspaceNotFound(child_id));
                }
            }
        }

        for &root_id in &self.root_ids {
            let node = self
                .nodes
                .get(&root_id)
                .ok_or(WorkspaceIndexError::WorkspaceNotFound(root_id))?;
            if node.parent_id.is_some() {
                return Err(WorkspaceIndexError::DuplicatePath(WorkspacePath::single(
                    node.name.clone(),
                )?));
            }
        }

        Ok(())
    }
}
