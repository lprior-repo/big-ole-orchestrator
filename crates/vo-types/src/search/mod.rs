mod error;
<<<<<<< HEAD
mod inverted_index;
mod query;
mod scoring;
=======
pub mod inverted_index;
pub mod query;
pub mod scoring;
>>>>>>> origin/polecat/guzzle-veloxide-4wc

pub use error::SearchError;
pub use inverted_index::{InvertedIndex, Posting, PostingList};
pub use query::{Query, QueryParser};
pub use scoring::{Bm25Scorer, Scorer, TfIdfScorer};

<<<<<<< HEAD
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
    #[allow(dead_code)]
    index: InvertedIndex,
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
        }
    }

    pub fn search(&self, query: &Query) -> Result<Vec<SearchResult>, SearchError> {
        let _ = query;
        Ok(vec![])
    }

    pub fn index_workspace(&mut self, id: crate::workspace::WorkspaceId, text: &str, tags: &[String]) {
        let _ = (id, text, tags);
    }
}
=======
pub struct SearchEngine;
pub struct SearchResult;
>>>>>>> origin/polecat/guzzle-veloxide-4wc
