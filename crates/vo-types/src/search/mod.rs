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
pub enum DocumentType {
    Workspace,
    Workflow,
    Execution,
}

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

        let avg_doc_len =
            self.index.document_lengths.values().copied().sum::<u32>() as f64 / total_docs;

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
                let terms: Vec<String> = matched_terms.get(&doc_id)?.iter().cloned().collect();
                Some(SearchResult {
                    document_id: doc_id,
                    score,
                    workspace_id: doc.workspace_id.clone(),
                    matched_terms: terms,
                    document_type: doc.doc_type.clone(),
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    pub fn index_workspace(
        &mut self,
        id: crate::workspace::WorkspaceId,
        text: &str,
        tags: &[String],
    ) {
        let _ = (id, text, tags);
    }

    #[must_use]
    pub fn remove_workspace(&mut self, _id: crate::workspace::WorkspaceId) -> bool {
        // Stub implementation - workspace removal not yet implemented
        true
    }
}
