mod error;
pub mod inverted_index;
pub mod query;
pub mod scoring;

pub use error::SearchError;
pub use inverted_index::{InvertedIndex, Posting, PostingList};
pub use query::{Query, QueryParser};
pub use scoring::{Bm25Scorer, Scorer, TfIdfScorer};

/// Search engine for indexing and querying workflows
#[derive(Debug, Default)]
pub struct SearchEngine {
    entries: Vec<(String, String)>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn index_workspace(
        &mut self,
        _id: crate::workspace::WorkspaceId,
        text: impl Into<String>,
        _tags: &[&str],
    ) {
        self.entries.push((text.into(), String::new()));
    }

    pub fn search(&self, query: &Query) -> Result<Vec<SearchResult>, SearchError> {
        let _ = query;
        Ok(vec![])
    }
}

/// A single search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub score: f64,
    pub snippet: String,
    pub workspace_id: String,
    pub matched_terms: Vec<String>,
}
