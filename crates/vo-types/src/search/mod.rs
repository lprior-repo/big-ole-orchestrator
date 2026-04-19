mod error;
mod inverted_index;
mod query;
mod scoring;

pub use error::SearchError;
pub use inverted_index::{InvertedIndex, Posting, PostingList};
pub use query::{Query, QueryParser};
pub use scoring::{Bm25Scorer, Scorer, TfIdfScorer};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub document_id: crate::workspace::WorkspaceId,
    pub score: f64,
    pub workspace_id: String,
    pub matched_terms: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SearchEngine {
    index: InvertedIndex,
    documents: std::collections::HashMap<crate::workspace::WorkspaceId, WorkspaceDocument>,
}

#[derive(Debug, Clone)]
struct WorkspaceDocument {
    text: String,
    tags: Vec<String>,
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
            documents: std::collections::HashMap::new(),
        }
    }

    pub fn search(&self, query: &Query) -> Result<Vec<SearchResult>, SearchError> {
        if query.terms.is_empty() {
            return Err(SearchError::EmptyQuery);
        }

        let mut results: std::collections::HashMap<
            crate::workspace::WorkspaceId,
            SearchResultBuilder,
        > = std::collections::HashMap::new();

        let total_docs = self.documents.len().max(1) as f64;

        for term in &query.terms {
            if let Some(posting_list) = self.index.index.get(term.as_str()) {
                let doc_freq = posting_list.document_frequency as f64;
                for posting in &posting_list.postings {
                    let idf = (total_docs / doc_freq).ln() + 1.0;
                    let avg_doc_len = if self.documents.is_empty() {
                        10.0
                    } else {
                        self.documents
                            .values()
                            .map(|d| d.text.split_whitespace().count() as f64)
                            .sum::<f64>()
                            / total_docs
                    };
                    let scorer = TfIdfScorer;
                    let score = scorer.score(posting, idf, avg_doc_len);

                    results
                        .entry(posting.document_id)
                        .or_insert_with(|| SearchResultBuilder {
                            document_id: posting.document_id,
                            score: 0.0,
                            workspace_id: posting.document_id.to_string(),
                            matched_terms: Vec::new(),
                        })
                        .add_score(score, term.clone());
                }
            }
        }

        let mut sorted_results: Vec<SearchResult> =
            results.into_values().map(|b| b.build()).collect();

        sorted_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(sorted_results)
    }

    pub fn search_with_tags(
        &self,
        query: &Query,
        required_tags: &[String],
    ) -> Result<Vec<SearchResult>, SearchError> {
        let all_results = self.search(query)?;

        if required_tags.is_empty() {
            return Ok(all_results);
        }

        let filtered_results: Vec<SearchResult> = all_results
            .into_iter()
            .filter(|result| {
                if let Some(doc) = self.documents.get(&result.document_id) {
                    required_tags.iter().all(|tag| doc.tags.contains(tag))
                } else {
                    false
                }
            })
            .collect();

        Ok(filtered_results)
    }

    pub fn index_workspace(
        &mut self,
        id: crate::workspace::WorkspaceId,
        text: &str,
        tags: &[String],
    ) {
        let doc_len = text.split_whitespace().count() as f64;

        self.documents.insert(
            id,
            WorkspaceDocument {
                text: text.to_string(),
                tags: tags.to_vec(),
            },
        );

        for word in text.to_lowercase().split_whitespace() {
            let term = word.trim_matches(|c: char| !c.is_alphanumeric());
            if !term.is_empty() {
                self.index.insert(term, id, doc_len);
            }
        }

        for tag in tags {
            let tag_term = format!("tag:{}", tag.to_lowercase());
            self.index.insert(&tag_term, id, doc_len);
        }
    }

    pub fn remove_workspace(&mut self, id: crate::workspace::WorkspaceId) {
        self.documents.remove(&id);
        self.index.remove_document(id);
    }
}

struct SearchResultBuilder {
    document_id: crate::workspace::WorkspaceId,
    score: f64,
    workspace_id: String,
    matched_terms: Vec<String>,
}

impl SearchResultBuilder {
    fn add_score(&mut self, score: f64, term: String) {
        self.score += score;
        if !self.matched_terms.contains(&term) {
            self.matched_terms.push(term);
        }
    }

    fn build(self) -> SearchResult {
        SearchResult {
            document_id: self.document_id,
            score: self.score,
            workspace_id: self.workspace_id,
            matched_terms: self.matched_terms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_001_index_workspace_and_query_finds_it() {
        let mut engine = SearchEngine::new();
        let id = crate::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "hello world", &[]);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_id, id);
    }

    #[test]
    fn se_002_query_with_tag_filter_works() {
        let mut engine = SearchEngine::new();
        let id1 = crate::workspace::WorkspaceId::generate();
        let id2 = crate::workspace::WorkspaceId::generate();
        engine.index_workspace(id1, "hello world", &["python".to_string()]);
        engine.index_workspace(id2, "hello world", &["rust".to_string()]);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine
            .search_with_tags(&query, &["python".to_string()])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_id, id1);
    }

    #[test]
    fn se_003_query_with_invalid_tag_returns_empty() {
        let mut engine = SearchEngine::new();
        let id = crate::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "hello world", &["python".to_string()]);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine
            .search_with_tags(&query, &["nonexistent".to_string()])
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn se_004_search_with_multiple_tags_filter() {
        let mut engine = SearchEngine::new();
        let id1 = crate::workspace::WorkspaceId::generate();
        let id2 = crate::workspace::WorkspaceId::generate();
        engine.index_workspace(
            id1,
            "hello world",
            &["python".to_string(), "web".to_string()],
        );
        engine.index_workspace(id2, "hello world", &["python".to_string()]);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine
            .search_with_tags(&query, &["python".to_string(), "web".to_string()])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document_id, id1);
    }

    #[test]
    fn se_005_concurrent_index_updates_are_handled() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let engine = Arc::new(Mutex::new(SearchEngine::new()));
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let engine_clone = Arc::clone(&engine);
                thread::spawn(move || {
                    let mut eng = engine_clone.lock().unwrap();
                    let id = crate::workspace::WorkspaceId::generate();
                    eng.index_workspace(id, &format!("workspace {} content", i), &[]);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let eng = engine.lock().unwrap();
        let query = QueryParser::new().parse("workspace").unwrap();
        let results = eng.search(&query).unwrap();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn se_006_search_empty_query_returns_error() {
        let engine = SearchEngine::new();
        let empty_query = Query::new(vec![]);
        let result = engine.search(&empty_query);
        assert!(result.is_err());
    }

    #[test]
    fn se_007_remove_workspace_removes_from_search() {
        let mut engine = SearchEngine::new();
        let id = crate::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "hello world", &[]);
        engine.remove_workspace(id);
        let query = QueryParser::new().parse("hello").unwrap();
        let results = engine.search(&query).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn se_008_multiple_terms_search() {
        let mut engine = SearchEngine::new();
        let id = crate::workspace::WorkspaceId::generate();
        engine.index_workspace(id, "hello world foo bar", &[]);
        let query = QueryParser::new().parse("hello world").unwrap();
        let results = engine.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].matched_terms.contains(&"hello".to_string()));
        assert!(results[0].matched_terms.contains(&"world".to_string()));
    }
}
