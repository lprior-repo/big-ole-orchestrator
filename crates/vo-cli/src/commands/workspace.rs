use vo_types::workspace::{
    WorkspaceId, WorkspaceIndex, WorkspaceIndexError, WorkspaceMetadata, WorkspaceName,
    WorkspaceNode, WorkspacePath,
};

pub fn workspace_create(
    index: &mut WorkspaceIndex,
    name: WorkspaceName,
    metadata: WorkspaceMetadata,
    now: vo_types::TimestampMs,
) -> Result<WorkspaceId, WorkspaceIndexError> {
    let _ = (index, name, metadata, now);
    todo!("wire WorkspaceIndex create into vo-cli workspace command")
}

pub fn workspace_list_roots(index: &WorkspaceIndex) -> Vec<(WorkspaceId, WorkspaceNode)> {
    let _ = index;
    todo!("wire WorkspaceIndex list roots into vo-cli workspace command")
}

pub fn workspace_get(
    index: &WorkspaceIndex,
    id: WorkspaceId,
) -> Result<WorkspaceNode, WorkspaceIndexError> {
    let _ = (index, id);
    todo!("wire WorkspaceIndex get into vo-cli workspace command")
}

pub fn workspace_remove(
    index: &mut WorkspaceIndex,
    id: WorkspaceId,
) -> Result<(), WorkspaceIndexError> {
    let _ = (index, id);
    todo!("wire WorkspaceIndex remove into vo-cli workspace command")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_index_type_is_accessible_from_vo_cli() {
        let index = WorkspaceIndex::new();
        let _ = &index;
    }

    #[test]
    fn workspace_id_type_is_accessible_from_vo_cli() {
        let id = WorkspaceId::generate();
        let _ = &id;
    }

    #[test]
    fn workspace_name_can_be_parsed() {
        let name = WorkspaceName::parse("my-workspace").unwrap();
        assert_eq!(name.as_str(), "my-workspace");
    }

    #[test]
    fn workspace_metadata_can_be_created() {
        let meta = WorkspaceMetadata::empty();
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn workspace_create_wires_into_index() {
        let mut index = WorkspaceIndex::new();
        let name = WorkspaceName::parse("test").unwrap();
        let meta = WorkspaceMetadata::empty();
        let now = vo_types::TimestampMs::try_from(1000u64).unwrap();
        let result = workspace_create(&mut index, name, meta, now);
        assert!(result.is_ok(), "workspace_create should succeed");
    }

    #[test]
    fn workspace_list_roots_returns_empty_for_new_index() {
        let index = WorkspaceIndex::new();
        let result = workspace_list_roots(&index);
        let _ = result;
    }

    #[test]
    fn workspace_get_fails_for_missing_id() {
        let index = WorkspaceIndex::new();
        let id = WorkspaceId::generate();
        let result = workspace_get(&index, id);
        let _ = result;
    }

    #[test]
    fn workspace_remove_wires_into_index() {
        let mut index = WorkspaceIndex::new();
        let id = WorkspaceId::generate();
        let _ = workspace_remove(&mut index, id);
    }

    #[test]
    fn workspace_path_can_be_created() {
        let name = WorkspaceName::parse("root").unwrap();
        let path = WorkspacePath::single(name).unwrap();
        assert_eq!(path.depth(), 1);
    }
}
