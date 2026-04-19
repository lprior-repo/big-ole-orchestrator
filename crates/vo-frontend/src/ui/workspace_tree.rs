use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTreeNode {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub metadata: WorkspaceMetadata,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTree {
    pub nodes: Vec<WorkspaceTreeNode>,
    pub root_ids: Vec<String>,
    pub version: u64,
}

impl WorkspaceTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root_ids: Vec::new(),
            version: 0,
        }
    }

    pub fn get_node(&self, id: &str) -> Option<&WorkspaceTreeNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_children(&self, id: &str) -> Vec<&WorkspaceTreeNode> {
        self.nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some(id))
            .collect()
    }

    pub fn get_roots(&self) -> Vec<&WorkspaceTreeNode> {
        self.root_ids
            .iter()
            .filter_map(|id| self.get_node(id))
            .collect()
    }

    pub fn is_leaf(&self, id: &str) -> bool {
        self.get_children(id).is_empty()
    }

    pub fn has_children(&self, id: &str) -> bool {
        !self.is_leaf(id)
    }
}

impl Default for WorkspaceTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_tree_new_is_empty() {
        let tree = WorkspaceTree::new();
        assert!(tree.nodes.is_empty());
        assert!(tree.root_ids.is_empty());
        assert_eq!(tree.version, 0);
    }

    #[test]
    fn workspace_tree_get_node_returns_none_for_missing() {
        let tree = WorkspaceTree::new();
        assert!(tree.get_node("nonexistent").is_none());
    }

    #[test]
    fn workspace_tree_is_leaf_for_node_without_children() {
        let node = WorkspaceTreeNode {
            id: "test-1".to_string(),
            name: "test".to_string(),
            parent_id: None,
            children: vec![],
            metadata: WorkspaceMetadata {
                description: None,
                tags: vec![],
            },
            created_at: 0,
            updated_at: 0,
        };
        let tree = WorkspaceTree {
            nodes: vec![node],
            root_ids: vec!["test-1".to_string()],
            version: 1,
        };
        assert!(tree.is_leaf("test-1"));
        assert!(!tree.has_children("test-1"));
    }

    #[test]
    fn workspace_tree_get_children_filters_correctly() {
        let nodes = vec![
            WorkspaceTreeNode {
                id: "root-1".to_string(),
                name: "root".to_string(),
                parent_id: None,
                children: vec!["child-1".to_string()],
                metadata: WorkspaceMetadata {
                    description: None,
                    tags: vec![],
                },
                created_at: 0,
                updated_at: 0,
            },
            WorkspaceTreeNode {
                id: "child-1".to_string(),
                name: "child".to_string(),
                parent_id: Some("root-1".to_string()),
                children: vec![],
                metadata: WorkspaceMetadata {
                    description: None,
                    tags: vec![],
                },
                created_at: 0,
                updated_at: 0,
            },
        ];
        let tree = WorkspaceTree {
            nodes,
            root_ids: vec!["root-1".to_string()],
            version: 1,
        };
        let children = tree.get_children("root-1");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "child-1");
    }

    #[test]
    fn workspace_tree_get_roots_returns_root_nodes() {
        let nodes = vec![
            WorkspaceTreeNode {
                id: "root-1".to_string(),
                name: "root1".to_string(),
                parent_id: None,
                children: vec![],
                metadata: WorkspaceMetadata {
                    description: None,
                    tags: vec![],
                },
                created_at: 0,
                updated_at: 0,
            },
            WorkspaceTreeNode {
                id: "root-2".to_string(),
                name: "root2".to_string(),
                parent_id: None,
                children: vec![],
                metadata: WorkspaceMetadata {
                    description: None,
                    tags: vec![],
                },
                created_at: 0,
                updated_at: 0,
            },
        ];
        let tree = WorkspaceTree {
            nodes,
            root_ids: vec!["root-1".to_string(), "root-2".to_string()],
            version: 1,
        };
        let roots = tree.get_roots();
        assert_eq!(roots.len(), 2);
    }

    #[test]
    fn workspace_tree_serde_roundtrip() {
        let nodes = vec![WorkspaceTreeNode {
            id: "test-1".to_string(),
            name: "test-node".to_string(),
            parent_id: None,
            children: vec![],
            metadata: WorkspaceMetadata {
                description: Some("A test node".to_string()),
                tags: vec!["test".to_string()],
            },
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
        }];
        let tree = WorkspaceTree {
            nodes,
            root_ids: vec!["test-1".to_string()],
            version: 42,
        };

        let json = serde_json::to_string(&tree).unwrap();
        let restored: WorkspaceTree = serde_json::from_str(&json).unwrap();

        assert_eq!(tree.nodes.len(), restored.nodes.len());
        assert_eq!(tree.root_ids, restored.root_ids);
        assert_eq!(tree.version, restored.version);
    }
}
