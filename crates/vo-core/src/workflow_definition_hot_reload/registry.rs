use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use vo_types::{WorkflowDefinition, WorkflowName};

pub struct WorkflowDefinitionRegistry {
    definitions: RwLock<HashMap<WorkflowName, WorkflowDefinition>>,
    binary_paths: RwLock<HashMap<WorkflowName, PathBuf>>,
    reverse_binary_lookup: RwLock<HashMap<PathBuf, WorkflowName>>,
}

impl WorkflowDefinitionRegistry {
    pub fn new() -> Self {
        Self {
            definitions: RwLock::new(HashMap::new()),
            binary_paths: RwLock::new(HashMap::new()),
            reverse_binary_lookup: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(
        &self,
        workflow_name: WorkflowName,
        definition: WorkflowDefinition,
        binary_path: PathBuf,
    ) {
        let mut definitions = self.definitions.write().expect(
            "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
        );
        definitions.insert(workflow_name.clone(), definition);
        drop(definitions);

        let mut binary_paths = self.binary_paths.write().expect(
            "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
        );
        binary_paths.insert(workflow_name.clone(), binary_path.clone());
        drop(binary_paths);

        let mut reverse = self.reverse_binary_lookup.write().expect(
            "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
        );
        reverse.insert(binary_path, workflow_name);
    }

    pub fn get(&self, name: &WorkflowName) -> Option<WorkflowDefinition> {
        self.definitions
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .get(name)
            .cloned()
    }

    pub fn get_binary_path(&self, name: &WorkflowName) -> Option<PathBuf> {
        self.binary_paths
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .get(name)
            .cloned()
    }

    pub fn get_by_binary_path(&self, path: &PathBuf) -> Option<WorkflowName> {
        self.reverse_binary_lookup
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .get(path)
            .cloned()
    }

    pub fn contains(&self, name: &WorkflowName) -> bool {
        self.definitions
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .contains_key(name)
    }

    pub fn remove(&self, name: &WorkflowName) {
        let mut definitions = self.definitions.write().expect(
            "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
        );
        definitions.remove(name);
        drop(definitions);

        let mut binary_paths = self.binary_paths.write().expect(
            "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
        );
        if let Some(path) = binary_paths.remove(name) {
            drop(binary_paths);
            let mut reverse = self.reverse_binary_lookup.write().expect(
                "SAFETY: RwLock not poisoned — no code path panics while holding this lock",
            );
            reverse.remove(&path);
        }
    }

    pub fn list_workflows(&self) -> Vec<WorkflowName> {
        self.definitions
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .keys()
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        self.definitions
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions
            .read()
            .expect("SAFETY: RwLock not poisoned")
            .is_empty()
    }
}

impl Default for WorkflowDefinitionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedWorkflowRegistry = Arc<WorkflowDefinitionRegistry>;

pub fn create_shared_registry() -> SharedWorkflowRegistry {
    Arc::new(WorkflowDefinitionRegistry::new())
}