mod error;
mod inverted_index;
mod query;
mod scoring;

pub use error::SearchError;
pub use inverted_index::{InvertedIndex, Posting, PostingList};
pub use query::{Query, QueryParser};
pub use scoring::{Bm25Scorer, Scorer, TfIdfScorer};

use serde::{Deserialize, Serialize};

use crate::workspace::{WorkspaceId, WorkspaceIndex};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub workspace_id: WorkspaceId,
    pub score: f64,
    pub matched_terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchEngine {
    index: InvertedIndex,
    document_count: usize,
    avg_doc_len: f64,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            index: InvertedIndex::new(),
            document_count: 0,
            avg_doc_len: 0.0,
        }
    }

    pub fn index_workspace(&mut self, id: WorkspaceId, name: &str, metadata_entries: &[String]) {
        let mut terms = Vec::new();
        terms.extend(tokenize(name));
        for entry in metadata_entries {
            terms.extend(tokenize(entry));
        }
        let doc_len = terms.len() as f64;
        for term in terms {
            self.index.insert(&term, id, doc_len);
        }
        self.document_count += 1;
        self.avg_doc_len = self.compute_avg_doc_len();
    }

    pub fn remove_workspace(&mut self, id: WorkspaceId) {
        self.index.remove_document(id);
    }

    pub fn search(&self, query: &Query) -> Result<Vec<SearchResult>, SearchError> {
        let mut results: Vec<SearchResult> = Vec::new();
        let mut doc_scores: std::collections::HashMap<WorkspaceId, (f64, Vec<String>)> =
            std::collections::HashMap::new();

        for term in &query.terms {
            if let Some(postings) = self.index.get(term) {
                let idf = self.idf(postings.len());
                for posting in postings {
                    let tf = posting.term_frequency as f64;
                    let doc_len = posting.document_length as f64;
                    let score = idf * tf / doc_len;
                    let entry = doc_scores
                        .entry(posting.document_id)
                        .or_insert((0.0, Vec::new()));
                    entry.0 += score;
                    if !entry.1.contains(term) {
                        entry.1.push(term.clone());
                    }
                }
            }
        }

        for (doc_id, (score, matched_terms)) in doc_scores {
            if score > 0.0 {
                results.push(SearchResult {
                    workspace_id: doc_id,
                    score,
                    matched_terms,
                });
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    pub fn search_with_index(
        &self,
        query: &Query,
        workspace_index: &WorkspaceIndex,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let results = self.search(query)?;
        let mut validated = Vec::new();
        for result in results {
            if workspace_index.find_by_id(result.workspace_id).is_ok() {
                validated.push(result);
            }
        }
        Ok(validated)
    }

    fn idf(&self, doc_frequency: usize) -> f64 {
        if doc_frequency == 0 {
            return 0.0;
        }
        let n = self.document_count as f64;
        let df = doc_frequency as f64;
        (n - df + 0.5) / (df + 0.5) + 1.0
    }

    fn compute_avg_doc_len(&self) -> f64 {
        if self.document_count == 0 {
            return 0.0;
        }
        let total: usize = self
            .index
            .document_lengths
            .values()
            .copied()
            .map(|v| v as usize)
            .sum();
        total as f64 / self.document_count as f64
    }

    pub fn from_workspace_index(workspace_index: &WorkspaceIndex) -> Self {
        let mut engine = Self::new();
        for (id, node) in &workspace_index.nodes {
            let name_str = node.name.as_str();
            let metadata_values: Vec<String> = node.metadata.entries.values().cloned().collect();
            engine.index_workspace(*id, name_str, &metadata_values);
        }
        engine
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_001_empty_index_search_returns_empty() {
        let engine = SearchEngine::new();
        let query = QueryParser::new().parse("test").unwrap();
        let results = engine.search(&query).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn se_002_index_and_search_single_term() {
        let mut engine = SearchEngine::new();
        let id = WorkspaceId::generate();
        engine.index_workspace(id, "hello-world", &[]);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].workspace_id, id);
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn se_003_search_multiple_terms() {
        let mut engine = SearchEngine::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        engine.index_workspace(id1, "hello world", &[]);
        engine.index_workspace(id2, "foo bar", &[]);
        let query = QueryParser::new().parse("hello world").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].workspace_id, id1);
    }

    #[test]
    fn se_004_search_nonexistent_term() {
        let mut engine = SearchEngine::new();
        let id = WorkspaceId::generate();
        engine.index_workspace(id, "hello", &[]);
        let query = QueryParser::new().parse("nonexistent").unwrap();
        let results = engine.search(&query).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn se_005_tokenize_lowercase() {
        let terms = tokenize("Hello WORLD");
        assert_eq!(terms.len(), 2);
        assert!(terms.contains(&"hello".to_string()));
        assert!(terms.contains(&"world".to_string()));
    }

    #[test]
    fn se_006_tokenize_special_chars() {
        let terms = tokenize("hello-world_test");
        assert_eq!(terms.len(), 3);
        assert!(terms.contains(&"hello".to_string()));
        assert!(terms.contains(&"world".to_string()));
        assert!(terms.contains(&"test".to_string()));
    }

    #[test]
    fn se_007_search_with_metadata() {
        let mut engine = SearchEngine::new();
        let id = WorkspaceId::generate();
        engine.index_workspace(
            id,
            "workspace",
            &["env=prod".to_string(), "region=us-east".to_string()],
        );
        let query = QueryParser::new().parse("prod").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].workspace_id, id);
    }

    #[test]
    fn se_008_remove_workspace() {
        let mut engine = SearchEngine::new();
        let id = WorkspaceId::generate();
        engine.index_workspace(id, "hello", &[]);
        engine.remove_workspace(id);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine.search(&query).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn se_009_search_ranking() {
        let mut engine = SearchEngine::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        engine.index_workspace(id1, "hello world", &[]);
        engine.index_workspace(id2, "hello world hello", &[]);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn se_010_from_workspace_index() {
        let mut workspace_index = WorkspaceIndex::new();
        let now = crate::types::TimestampMs::try_from(1000u64).unwrap();
        let id1 = workspace_index
            .insert(
                None,
                crate::workspace::WorkspaceName::parse("test-workspace").unwrap(),
                crate::workspace::WorkspaceMetadata::empty(),
                now,
            )
            .unwrap();
        let mut meta = crate::workspace::WorkspaceMetadata::empty();
        meta.entries.insert("env".to_string(), "prod".to_string());
        let _id2 = workspace_index
            .insert(
                None,
                crate::workspace::WorkspaceName::parse("another-workspace").unwrap(),
                meta,
                now,
            )
            .unwrap();
        let engine = SearchEngine::from_workspace_index(&workspace_index);
        let query = QueryParser::new().parse("test").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].workspace_id, id1);
    }
}
