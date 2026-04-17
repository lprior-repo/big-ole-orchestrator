use vo_types::search::{SearchEngine, SearchError, SearchResult};
use vo_types::workspace::WorkspaceIndex;

pub fn search_workflows(
    engine: &SearchEngine,
    query_str: &str,
) -> Result<Vec<SearchResult>, SearchError> {
    let _ = (engine, query_str);
    todo!("wire SearchEngine into vo-api search endpoint")
}

pub fn search_workflows_with_workspace_filter(
    engine: &SearchEngine,
    query_str: &str,
    workspace_index: &WorkspaceIndex,
) -> Result<Vec<SearchResult>, SearchError> {
    let _ = (engine, query_str, workspace_index);
    todo!("wire SearchEngine with workspace filter into vo-api search endpoint")
}

pub fn build_search_engine_from_workspace(workspace_index: &WorkspaceIndex) -> SearchEngine {
    let _ = workspace_index;
    todo!("wire workspace index into SearchEngine::from_workspace_index")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_workflows_returns_results_for_valid_query() {
        let mut engine = SearchEngine::new();
        let id = vo_types::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "workflow step completed", &[]);
        let results = search_workflows(&engine, "workflow");
        assert!(results.is_ok(), "search should succeed for valid query");
    }

    #[test]
    fn search_workflows_returns_empty_for_no_match() {
        let mut engine = SearchEngine::new();
        let id = vo_types::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "hello world", &[]);
        let results = search_workflows(&engine, "nonexistent");
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }

    #[test]
    fn search_workflows_with_workspace_filter_validates() {
        let mut engine = SearchEngine::new();
        let mut ws_index = WorkspaceIndex::new();
        let now = vo_types::TimestampMs::try_from(1000u64).unwrap();
        let id = ws_index
            .insert(
                None,
                vo_types::workspace::WorkspaceName::parse("test").unwrap(),
                vo_types::workspace::WorkspaceMetadata::empty(),
                now,
            )
            .unwrap();
        engine.index_workspace(id, "workflow execution", &[]);
        let results = search_workflows_with_workspace_filter(&engine, "workflow", &ws_index);
        assert!(results.is_ok());
    }

    #[test]
    fn build_search_engine_from_workspace_works() {
        let mut ws_index = WorkspaceIndex::new();
        let now = vo_types::TimestampMs::try_from(1000u64).unwrap();
        let _id = ws_index
            .insert(
                None,
                vo_types::workspace::WorkspaceName::parse("my-workspace").unwrap(),
                vo_types::workspace::WorkspaceMetadata::empty(),
                now,
            )
            .unwrap();
        let engine = build_search_engine_from_workspace(&ws_index);
        let _ = &engine;
    }
}
