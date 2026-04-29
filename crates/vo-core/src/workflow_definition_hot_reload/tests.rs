#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use crate::workflow_definition_hot_reload::{WorkflowDefinitionRegistry, create_shared_registry};
    use vo_types::WorkflowName;

    #[test]
    fn registry_starts_empty() {
        let registry = create_shared_registry();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registry_is_empty_after_creation() {
        let registry = WorkflowDefinitionRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_get_returns_none_for_unknown_workflow() {
        let registry = create_shared_registry();
        let unknown_name = WorkflowName::parse("unknown").unwrap();
        assert!(registry.get(&unknown_name).is_none());
    }

    #[test]
    fn registry_contains_returns_false_for_unknown_workflow() {
        let registry = create_shared_registry();
        let unknown_name = WorkflowName::parse("unknown").unwrap();
        assert!(!registry.contains(&unknown_name));
    }

    #[test]
    fn registry_get_binary_path_returns_none_for_unknown_workflow() {
        let registry = create_shared_registry();
        let unknown_name = WorkflowName::parse("unknown").unwrap();
        assert!(registry.get_binary_path(&unknown_name).is_none());
    }

    #[test]
    fn registry_get_by_binary_path_returns_none_for_unknown_path() {
        let registry = create_shared_registry();
        let unknown_path = PathBuf::from("/unknown/path");
        assert!(registry.get_by_binary_path(&unknown_path).is_none());
    }

    #[test]
    fn registry_list_workflows_returns_empty_for_new_registry() {
        let registry = create_shared_registry();
        assert!(registry.list_workflows().is_empty());
    }
}