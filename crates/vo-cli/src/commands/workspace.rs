use std::collections::BTreeMap;
use std::path::PathBuf;

use vo_types::workspace::{
    WorkspaceId, WorkspaceIndex, WorkspaceIndexError, WorkspaceMetadata, WorkspaceName,
    WorkspacePath,
};

use crate::cli::WorkspaceSubcommand;

#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub project_dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace not found: {0}")]
    NotFound(WorkspaceId),
    #[error("path not found: {0}")]
    PathNotFound(WorkspacePath),
    #[error("parent not found: {0}")]
    ParentNotFound(WorkspaceId),
    #[error("index not initialized")]
    IndexNotInitialized,
    #[error("invalid workspace name: {0}")]
    InvalidName(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("duplicate name under parent {parent_id}: {name}")]
    DuplicateName {
        parent_id: WorkspaceId,
        name: WorkspaceName,
    },
    #[error("cyclic move detected")]
    CyclicMove,
    #[error("cannot delete workspace with {child_count} children")]
    HasChildren { child_count: usize },
    #[error("metadata error: {0}")]
    MetadataError(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<WorkspaceIndexError> for WorkspaceError {
    fn from(err: WorkspaceIndexError) -> Self {
        match err {
            WorkspaceIndexError::WorkspaceNotFound(id) => WorkspaceError::NotFound(id),
            WorkspaceIndexError::PathNotFound(path) => WorkspaceError::PathNotFound(path),
            WorkspaceIndexError::ParentNotFound(id) => WorkspaceError::ParentNotFound(id),
            WorkspaceIndexError::IndexNotInitialized => WorkspaceError::IndexNotInitialized,
            WorkspaceIndexError::InvalidWorkspaceName(s) => WorkspaceError::InvalidName(s),
            WorkspaceIndexError::DuplicateName { parent_id, name } => {
                WorkspaceError::DuplicateName { parent_id, name }
            }
            WorkspaceIndexError::CyclicMoveDetected { .. } => WorkspaceError::CyclicMove,
            WorkspaceIndexError::DuplicatePath(path) => {
                WorkspaceError::InvalidPath(path.to_string())
            }
            WorkspaceIndexError::CannotDeleteWorkspaceWithChildren { child_count, .. } => {
                WorkspaceError::HasChildren {
                    child_count: child_count as usize,
                }
            }
            WorkspaceIndexError::MetadataKeyTooLong { .. } => {
                WorkspaceError::MetadataError("key too long".to_string())
            }
            WorkspaceIndexError::MetadataValueTooLong { .. } => {
                WorkspaceError::MetadataError("value too long".to_string())
            }
            WorkspaceIndexError::TooManyMetadataEntries { .. } => {
                WorkspaceError::MetadataError("too many entries".to_string())
            }
            WorkspaceIndexError::EmptyPathSegment => {
                WorkspaceError::InvalidPath("empty segment".to_string())
            }
            WorkspaceIndexError::PathTooDeep { .. } => {
                WorkspaceError::InvalidPath("too deep".to_string())
            }
            _ => WorkspaceError::IndexNotInitialized,
        }
    }
}

fn load_index(project_dir: &PathBuf) -> Result<WorkspaceIndex, WorkspaceError> {
    let index_path = project_dir.join(".vo").join("workspace_index.json");
    if index_path.exists() {
        let content = std::fs::read_to_string(&index_path)?;
        let index: WorkspaceIndex = serde_json::from_str(&content)
            .map_err(|e| WorkspaceError::Io(std::io::Error::other(e.to_string())))?;
        Ok(index)
    } else {
        Ok(WorkspaceIndex::new())
    }
}

fn save_index(project_dir: &PathBuf, index: &WorkspaceIndex) -> Result<(), WorkspaceError> {
    let index_path = project_dir.join(".vo").join("workspace_index.json");
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| WorkspaceError::Io(std::io::Error::other(e.to_string())))?;
    std::fs::write(&index_path, content)?;
    Ok(())
}

pub fn run_workspace(
    config: &WorkspaceConfig,
    subcmd: WorkspaceSubcommand,
) -> Result<String, WorkspaceError> {
    let now = vo_types::TimestampMs::now();

    let mut index = load_index(&config.project_dir)?;

    match subcmd {
        WorkspaceSubcommand::Create {
            name,
            parent_id,
            metadata,
        } => {
            let meta = WorkspaceMetadata { entries: metadata };
            let id = index.insert(parent_id, name.clone(), meta, now)?;
            save_index(&config.project_dir, &index)?;
            Ok(format!("Created workspace '{}' with id {}", name, id))
        }
        WorkspaceSubcommand::List { workspace_id } => {
            if let Some(pid) = workspace_id {
                let children = index.list_children(pid)?;
                let mut output = String::new();
                for cid in children {
                    if let Ok(node) = index.find_by_id(cid) {
                        output.push_str(&format!("{} ({})\n", node.name, cid));
                    }
                }
                Ok(output)
            } else {
                let mut output = String::new();
                for rid in &index.root_ids {
                    if let Ok(node) = index.find_by_id(*rid) {
                        output.push_str(&format!("{} ({})\n", node.name, rid));
                    }
                }
                Ok(output)
            }
        }
        WorkspaceSubcommand::Delete { id, force } => {
            if !force {
                let node = index.find_by_id(id)?;
                if !node.children.is_empty() {
                    return Err(WorkspaceError::HasChildren {
                        child_count: node.children.len(),
                    });
                }
            }
            index.delete(id)?;
            save_index(&config.project_dir, &index)?;
            Ok(format!("Deleted workspace {}", id))
        }
        WorkspaceSubcommand::Move { id, new_parent_id } => {
            index.move_workspace(id, new_parent_id, now)?;
            save_index(&config.project_dir, &index)?;
            Ok(format!("Moved workspace {}", id))
        }
        WorkspaceSubcommand::Show { id } => {
            let node = index.find_by_id(id)?;
            let path = {
                let mut segments = vec![node.name.as_str().to_string()];
                let mut current_parent = node.parent_id;
                while let Some(pid) = current_parent {
                    if let Ok(parent_node) = index.find_by_id(pid) {
                        segments.insert(0, parent_node.name.as_str().to_string());
                        current_parent = parent_node.parent_id;
                    } else {
                        break;
                    }
                }
                segments.join("/")
            };
            let mut output = String::new();
            output.push_str(&format!("ID: {}\n", id));
            output.push_str(&format!("Name: {}\n", node.name));
            output.push_str(&format!("Path: {}\n", path));
            if let Some(pid) = node.parent_id {
                output.push_str(&format!("Parent: {}\n", pid));
            }
            output.push_str(&format!("Children: {}\n", node.children.len()));
            output.push_str("Metadata:\n");
            for (k, v) in &node.metadata.entries {
                output.push_str(&format!("  {}: {}\n", k, v));
            }
            Ok(output)
        }
        WorkspaceSubcommand::Find { path } => {
            let id = index.find_by_path(&path)?;
            Ok(format!("{}", id))
        }
    }
}
