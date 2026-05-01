mod error;
pub mod inverted_index;
pub mod query;
pub mod scoring;

pub use error::SearchError;
pub use inverted_index::{InvertedIndex, Posting, PostingList};
pub use query::{Query, QueryParser};
pub use scoring::{Bm25Scorer, Scorer, TfIdfScorer};

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub document_id: WorkspaceId,
    pub score: f64,
    pub workspace_id: String,
    pub matched_terms: Vec<String>,
    pub document_type: DocumentType,
}

#[derive(Debug, Clone)]
struct DocumentEntry {
    workspace_id: String,
    doc_type: DocumentType,
    #[allow(dead_code)]
    text: String,
    #[allow(dead_code)]
    tags: Vec<String>,
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

#[derive(Debug, Clone)]
struct DocumentEntry {
    workspace_id: String,
    #[allow(dead_code)]
    text: String,
    #[allow(dead_code)]
    tags: Vec<String>,
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

#[derive(Debug, Clone)]
pub struct SearchEngine {
    index: InvertedIndex,
    documents: HashMap<WorkspaceId, DocumentEntry>,
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
            documents: HashMap::new(),
        }
    }

    pub fn search(&self, query: &Query) -> Result<Vec<SearchResult>, SearchError> {
        if query.terms.is_empty() {
            return Err(SearchError::EmptyQuery);
        }

        let total_docs = self.documents.len() as f64;
        if total_docs == 0.0 {
            return Ok(vec![]);
        }

        let scorer = Bm25Scorer::new();

        let avg_doc_len = self
            .index
            .document_lengths
            .values()
            .copied()
            .sum::<u32>() as f64
            / total_docs;

        let mut scores: BTreeMap<WorkspaceId, f64> = BTreeMap::new();
        let mut matched_terms: HashMap<WorkspaceId, HashSet<String>> = HashMap::new();

        for term in &query.terms {
            let postings = match self.index.get(term) {
                Some(p) => p,
                None => continue,
            };

            let df = self.index.document_frequency(term) as f64;
            let idf = ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln();

            for posting in postings {
                let term_score = scorer.score(posting, idf, avg_doc_len);
                *scores.entry(posting.document_id).or_insert(0.0) += term_score;
                matched_terms
                    .entry(posting.document_id)
                    .or_default()
                    .insert(term.clone());
            }
        }

        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .filter_map(|(doc_id, score)| {
                let doc = self.documents.get(&doc_id)?;
                let terms: Vec<String> = matched_terms
                    .get(&doc_id)?
                    .iter()
                    .cloned()
                    .collect();
                Some(SearchResult {
                    document_id: doc_id,
                    score,
                    workspace_id: doc.workspace_id.clone(),
                    matched_terms: terms,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results)
    }

    pub fn index_workspace(
        &mut self,
        id: WorkspaceId,
        text: &str,
        tags: &[String],
    ) {
        let all_text = if tags.is_empty() {
            text.to_string()
        } else {
            let tags_str = tags.join(" ");
            format!("{} {}", text, tags_str)
        };

        let tokens = tokenize(&all_text);
        let doc_length = tokens.len() as f64;

        for token in &tokens {
            self.index.insert(token, id, doc_length);
        }

        self.documents.insert(
            id,
            DocumentEntry {
                workspace_id: id.to_string(),
                text: text.to_string(),
                tags: tags.to_vec(),
            },
        );
    }

    pub fn remove_workspace(&mut self, id: WorkspaceId) {
        self.index.remove_document(id);
        self.documents.remove(&id);
    }

    #[must_use]
    pub fn remove_workspace(&mut self, _id: crate::workspace::WorkspaceId) -> bool {
        // Stub implementation - workspace removal not yet implemented
        true
    }
}

#[cfg(test)]
mod tests {
        use super::*;

        #[test]
        fn search_returns_matching_results() {
            let mut engine = SearchEngine::new();
            let id = WorkspaceId::generate();
            engine.index_workspace(id, "workflow step completed", &[]);
            let query = QueryParser::new().parse("workflow").unwrap();
            let results = engine.search(&query).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].document_id, id);
            assert!(results[0].score > 0.0);
            assert!(results[0].matched_terms.contains(&"workflow".to_string()));
        }

        #[test]
        fn search_returns_empty_for_no_match() {
            let mut engine = SearchEngine::new();
            let id = WorkspaceId::generate();
            engine.index_workspace(id, "hello world", &[]);
            let query = QueryParser::new().parse("nonexistent").unwrap();
            let results = engine.search(&query).unwrap();
            assert!(results.is_empty());
        }

        #[test]
        fn search_ranks_by_relevance() {
            let mut engine = SearchEngine::new();
            let id1 = WorkspaceId::generate();
            let id2 = WorkspaceId::generate();
            engine.index_workspace(id1, "workflow workflow workflow", &[]);
            engine.index_workspace(id2, "workflow other", &[]);
            let query = QueryParser::new().parse("workflow").unwrap();
            let results = engine.search(&query).unwrap();
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].document_id, id1);
            assert!(results[0].score > results[1].score);
        }

        #[test]
        fn search_matches_tags() {
            let mut engine = SearchEngine::new();
            let id = WorkspaceId::generate();
            engine.index_workspace(id, "some content", &["important".to_string(), "urgent".to_string()]);
            let query = QueryParser::new().parse("urgent").unwrap();
            let results = engine.search(&query).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].document_id, id);
        }

        #[test]
        fn search_multi_term_query() {
            let mut engine = SearchEngine::new();
            let id = WorkspaceId::generate();
            engine.index_workspace(id, "payment processing workflow", &[]);
            let query = QueryParser::new().parse("payment workflow").unwrap();
            let results = engine.search(&query).unwrap();
            assert_eq!(results.len(), 1);
            assert!(results[0].matched_terms.contains(&"payment".to_string()));
            assert!(results[0].matched_terms.contains(&"workflow".to_string()));
        }

        #[test]
        fn search_empty_index() {
            let engine = SearchEngine::new();
            let query = QueryParser::new().parse("test").unwrap();
            let results = engine.search(&query).unwrap();
            assert!(results.is_empty());
        }

        #[test]
        fn remove_workspace_excludes_from_search() {
            let mut engine = SearchEngine::new();
            let id = WorkspaceId::generate();
            engine.index_workspace(id, "test content", &[]);
            engine.remove_workspace(id);
            let query = QueryParser::new().parse("test").unwrap();
            let results = engine.search(&query).unwrap();
            assert!(results.is_empty());
        }

        #[test]
        fn search_case_insensitive() {
            let mut engine = SearchEngine::new();
            let id = WorkspaceId::generate();
            engine.index_workspace(id, "Hello World", &[]);
            let query = QueryParser::new().parse("hello").unwrap();
            let results = engine.search(&query).unwrap();
            assert_eq!(results.len(), 1);
        }
    }
