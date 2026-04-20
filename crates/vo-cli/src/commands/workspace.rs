use std::path::PathBuf;
use vo_types::workspace::{WorkspaceId, WorkspaceIndex, WorkspaceMetadata, WorkspaceName};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace not found: {0}")]
    NotFound(String),
    #[error("invalid workspace name: {0}")]
    InvalidName(String),
    #[error("IO error: {0}")]
    Io(std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub storage_path: PathBuf,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(".vo/workspace.json"),
        }
    }
}

pub fn load_index(path: &PathBuf) -> Result<WorkspaceIndex, WorkspaceError> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let index: WorkspaceIndex = serde_json::from_str(&content)?;
        Ok(index)
    } else {
        Ok(WorkspaceIndex::new())
    }
}

pub fn save_index(index: &WorkspaceIndex, path: &PathBuf) -> Result<(), WorkspaceError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(index)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub async fn list_workspaces(config: WorkspaceConfig) -> Result<(), WorkspaceError> {
    let index = load_index(&config.storage_path)?;
    let roots = index.root_ids.clone();
    if roots.is_empty() {
        println!("No workspaces found.");
        return Ok(());
    }
    println!("Workspaces:");
    for root_id in &roots {
        let node = index.find_by_id(*root_id).map_err(|e| WorkspaceError::NotFound(e.to_string()))?;
        println!("  {} ({})", node.name, root_id);
    }
    Ok(())
}

pub async fn create_workspace(
    config: WorkspaceConfig,
    name: String,
) -> Result<(), WorkspaceError> {
    let mut index = load_index(&config.storage_path)?;
    let ws_name = WorkspaceName::parse(&name)
        .map_err(|_| WorkspaceError::InvalidName(name.clone()))?;
    let metadata = WorkspaceMetadata::empty();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    match index.insert(None, ws_name, metadata, now) {
        Ok(id) => {
            save_index(&index, &config.storage_path)?;
            println!("Created workspace '{}' with ID {}", name, id);
        }
        Err(e) => {
            eprintln!("Failed to create workspace: {}", e);
        }
    }
    Ok(())
}

pub async fn delete_workspace(
    config: WorkspaceConfig,
    id_str: String,
) -> Result<(), WorkspaceError> {
    let mut index = load_index(&config.storage_path)?;
    let id = WorkspaceId::parse(&id_str)
        .map_err(|_| WorkspaceError::NotFound(format!("invalid workspace ID: {}", id_str)))?;
    match index.delete(id) {
        Ok(()) => {
            save_index(&index, &config.storage_path)?;
            println!("Deleted workspace {}", id_str);
        }
        Err(e) => {
            eprintln!("Failed to delete workspace: {}", e);
        }
    }
    Ok(())
}

pub async fn show_workspace(
    config: WorkspaceConfig,
    id_str: String,
) -> Result<(), WorkspaceError> {
    let index = load_index(&config.storage_path)?;
    let id = WorkspaceId::parse(&id_str)
        .map_err(|_| WorkspaceError::NotFound(format!("invalid workspace ID: {}", id_str)))?;
    match index.find_by_id(id) {
        Ok(node) => {
            println!("Workspace: {}", node.name);
            println!("ID: {}", id);
            if let Some(parent) = node.parent_id {
                println!("Parent: {}", parent);
            }
            println!("Children: {}", node.children.len());
            println!("Metadata keys: {}", node.metadata.keys().len());
        }
        Err(e) => {
            eprintln!("Workspace not found: {}", e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_list_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("workspace.json");
        let config = WorkspaceConfig {
            storage_path: storage_path.clone(),
        };

        create_workspace(config.clone(), "test-workspace".to_string())
            .await
            .unwrap();

        let content = std::fs::read_to_string(&storage_path).unwrap();
        let index: WorkspaceIndex = serde_json::from_str(&content).unwrap();
        assert_eq!(index.root_ids.len(), 1);

        list_workspaces(config).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("workspace.json");
        let config = WorkspaceConfig {
            storage_path: storage_path.clone(),
        };

        create_workspace(config.clone(), "to-delete".to_string())
            .await
            .unwrap();

        let content = std::fs::read_to_string(&storage_path).unwrap();
        let index: WorkspaceIndex = serde_json::from_str(&content).unwrap();
        let id = index.root_ids[0];

        delete_workspace(config.clone(), id.to_string())
            .await
            .unwrap();

        let content = std::fs::read_to_string(&storage_path).unwrap();
        let index: WorkspaceIndex = serde_json::from_str(&content).unwrap();
        assert_eq!(index.root_ids.len(), 0);
    }
}
