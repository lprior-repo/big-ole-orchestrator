use vo_types::search::{QueryParser, SearchEngine, SearchError, SearchResult};
use vo_types::workspace::WorkspaceIndex;

pub fn search_workflows(
    engine: &SearchEngine,
    query_str: &str,
) -> Result<Vec<SearchResult>, SearchError> {
    let parsed = QueryParser::new().parse(query_str)?;
    engine.search(&parsed)
}

pub fn search_workflows_with_workspace_filter(
    engine: &SearchEngine,
    query_str: &str,
    workspace_index: &WorkspaceIndex,
) -> Result<Vec<SearchResult>, SearchError> {
    let all_results = search_workflows(engine, query_str)?;
    let workspace_ids: std::collections::HashSet<_> = workspace_index.nodes.keys().collect();
    Ok(all_results
        .into_iter()
        .filter(|r| workspace_ids.contains(&r.document_id))
        .collect())
}

pub fn build_search_engine_from_workspace(workspace_index: &WorkspaceIndex) -> SearchEngine {
    let mut engine = SearchEngine::new();
    for (id, node) in &workspace_index.nodes {
        let text = node.name.to_string();
        let tags: Vec<String> = node
            .metadata
            .entries
            .iter()
            .flat_map(|(k, v)| [k.clone(), v.clone()])
            .collect();
        engine.index_workspace(*id, &text, &tags);
    }
    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::search::QueryParser;
    use vo_types::TimestampMs;

    #[test]
    fn search_workflows_returns_results_for_valid_query() {
        let mut engine = SearchEngine::new();
        let id = vo_types::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "workflow step completed", &[]);
        let results = search_workflows(&engine, "workflow");
        assert!(results.is_ok(), "search should succeed for valid query");
        assert_eq!(results.unwrap().len(), 1);
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
        let now = TimestampMs::try_from(1000u64).unwrap();
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
        assert_eq!(results.unwrap().len(), 1);
    }

    #[test]
    fn search_workflows_with_workspace_filter_excludes_unknown() {
        let mut engine = SearchEngine::new();
        let mut ws_index = WorkspaceIndex::new();
        let now = TimestampMs::try_from(1000u64).unwrap();
        let indexed_id = vo_types::workspace::WorkspaceId::generate();
        engine.index_workspace(indexed_id, "workflow execution", &[]);
        let _ws_id = ws_index
            .insert(
                None,
                vo_types::workspace::WorkspaceName::parse("test").unwrap(),
                vo_types::workspace::WorkspaceMetadata::empty(),
                now,
            )
            .unwrap();
        let results = search_workflows_with_workspace_filter(&engine, "workflow", &ws_index);
        assert!(results.is_ok());
        assert!(results.unwrap().is_empty());
    }

    #[test]
    fn build_search_engine_from_workspace_works() {
        let mut ws_index = WorkspaceIndex::new();
        let now = TimestampMs::try_from(1000u64).unwrap();
        let id = ws_index
            .insert(
                None,
                vo_types::workspace::WorkspaceName::parse("my-workspace").unwrap(),
                vo_types::workspace::WorkspaceMetadata::empty(),
                now,
            )
            .unwrap();
        let engine = build_search_engine_from_workspace(&ws_index);
        let query = QueryParser::new().parse("my-workspace").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_id, id);
    }

    #[test]
    fn search_engine_type_accessible() {
        let engine = SearchEngine::new();
        let _ = &engine;
    }

    #[test]
    fn query_parser_type_accessible() {
        let parser = QueryParser::new();
        let _ = &parser;
    }

    #[test]
    fn search_result_type_accessible() {
        let mut engine = SearchEngine::new();
        let id = vo_types::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "test", &[]);
        let query = QueryParser::new().parse("test").unwrap();
        let results: Vec<SearchResult> = engine.search(&query).unwrap();
        assert!(
            !results.is_empty(),
            "SearchEngine is wired — returns results for indexed content"
        );
    }
}
