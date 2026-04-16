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
}

#[derive(Debug, Clone)]
pub struct SearchEngine {
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
}
