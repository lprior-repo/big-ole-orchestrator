mod error;
pub mod inverted_index;
pub mod query;
pub mod scoring;

pub use error::SearchError;
pub use inverted_index::{InvertedIndex, Posting, PostingList};
pub use query::{Query, QueryParser};
pub use scoring::{Bm25Scorer, Scorer, TfIdfScorer};

pub struct SearchEngine;
pub struct SearchResult;
