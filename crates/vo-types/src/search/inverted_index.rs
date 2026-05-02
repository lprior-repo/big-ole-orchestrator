use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::workspace::WorkspaceId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    pub document_id: WorkspaceId,
    pub term_frequency: u32,
    pub document_length: u32,
    pub positions: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostingList {
    pub postings: Vec<Posting>,
    pub document_frequency: usize,
}

impl PostingList {
    pub fn new() -> Self {
        Self {
            postings: Vec::new(),
            document_frequency: 0,
        }
    }
}

impl Default for PostingList {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvertedIndex {
    pub index: BTreeMap<String, PostingList>,
    pub document_lengths: BTreeMap<WorkspaceId, u32>,
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl InvertedIndex {
    pub fn new() -> Self {
        Self {
            index: BTreeMap::new(),
            document_lengths: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, term: &str, document_id: WorkspaceId, document_length: f64) {
        let doc_len = document_length as u32;
        self.document_lengths.insert(document_id, doc_len);

        let posting_list = self.index.entry(term.to_string()).or_default();

        if let Some(existing) = posting_list
            .postings
            .iter_mut()
            .find(|p| p.document_id == document_id)
        {
            existing.term_frequency += 1;
        } else {
            let position = posting_list.document_frequency as u32;
            posting_list.postings.push(Posting {
                document_id,
                term_frequency: 1,
                document_length: doc_len,
                positions: vec![position],
            });
            posting_list.document_frequency += 1;
        }
    }

    pub fn remove_document(&mut self, document_id: WorkspaceId) {
        self.document_lengths.remove(&document_id);
        for posting_list in self.index.values_mut() {
            posting_list
                .postings
                .retain(|p| p.document_id != document_id);
            posting_list.document_frequency = posting_list.postings.len();
        }
    }

    pub fn get(&self, term: &str) -> Option<&Vec<Posting>> {
        self.index.get(term).map(|pl| &pl.postings)
    }

    pub fn contains(&self, term: &str) -> bool {
        self.index.contains_key(term)
    }

    pub fn document_frequency(&self, term: &str) -> usize {
        self.index
            .get(term)
            .map(|pl| pl.document_frequency)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ii_001_insert_and_retrieve() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("hello", id, 10.0);
        let postings = index.get("hello").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].document_id, id);
        assert_eq!(postings[0].term_frequency, 1);
    }

    #[test]
    fn ii_002_multiple_terms() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("hello", id, 10.0);
        index.insert("world", id, 10.0);
        assert!(index.get("hello").is_some());
        assert!(index.get("world").is_some());
        assert!(index.get("missing").is_none());
    }

    #[test]
    fn ii_003_multiple_documents() {
        let mut index = InvertedIndex::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        index.insert("hello", id1, 10.0);
        index.insert("hello", id2, 20.0);
        let postings = index.get("hello").unwrap();
        assert_eq!(postings.len(), 2);
    }

    #[test]
    fn ii_004_remove_document() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("hello", id, 10.0);
        index.remove_document(id);
        let postings = index.get("hello");
        assert!(postings.is_none() || postings.unwrap().is_empty());
    }

    #[test]
    fn ii_005_term_frequency_increment() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("hello", id, 10.0);
        index.insert("hello", id, 10.0);
        let postings = index.get("hello").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].term_frequency, 2);
    }

    #[test]
    fn ii_006_document_frequency() {
        let mut index = InvertedIndex::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        index.insert("hello", id1, 10.0);
        index.insert("hello", id2, 10.0);
        assert_eq!(index.document_frequency("hello"), 2);
    }

    #[test]
    fn ii_007_empty_index() {
        let index = InvertedIndex::new();
        assert!(index.get("hello").is_none());
        assert!(!index.contains("hello"));
    }

    #[test]
    fn ii_008_merge_frequencies_duplicate_term() {
        let mut index = InvertedIndex::new();
        let doc1 = WorkspaceId::generate();
        let doc2 = WorkspaceId::generate();

        for _ in 0..3 {
            index.insert("workflow", doc1, 10.0);
        }
        let postings = index.get("workflow").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].term_frequency, 3);

        for _ in 0..2 {
            index.insert("workflow", doc1, 10.0);
        }
        let postings = index.get("workflow").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].term_frequency, 5);

        index.insert("workflow", doc2, 20.0);
        let postings = index.get("workflow").unwrap();
        assert_eq!(postings.len(), 2);
        assert_eq!(
            postings.iter().find(|p| p.document_id == doc1).unwrap().term_frequency,
            5
        );
        assert_eq!(
            postings.iter().find(|p| p.document_id == doc2).unwrap().term_frequency,
            1
        );

        assert_eq!(index.document_frequency("workflow"), 2);
    }

    #[test]
    fn ii_009_empty_string_term_inserted_as_key() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("", id, 10.0);
        assert!(index.contains(""));
        assert_eq!(index.get("").unwrap().len(), 1);
    }

    #[test]
    fn ii_010_empty_string_does_not_corrupt_other_terms() {
        let mut index = InvertedIndex::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        index.insert("hello", id1, 10.0);
        index.insert("", id2, 5.0);
        assert_eq!(index.get("hello").unwrap().len(), 1);
        assert_eq!(index.get("hello").unwrap()[0].document_id, id1);
        assert!(index.contains(""));
    }

    #[test]
    fn ii_011_zero_document_length() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("hello", id, 0.0);
        let postings = index.get("hello").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].document_length, 0);
    }

    #[test]
    fn ii_012_zero_length_does_not_corrupt_index() {
        let mut index = InvertedIndex::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        index.insert("hello", id1, 10.0);
        index.insert("world", id2, 0.0);
        assert_eq!(index.get("hello").unwrap()[0].document_length, 10);
        assert_eq!(index.get("world").unwrap()[0].document_length, 0);
    }

    #[test]
    fn ii_013_single_char_term() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("a", id, 10.0);
        assert!(index.contains("a"));
        assert_eq!(index.get("a").unwrap().len(), 1);
    }

    #[test]
    fn ii_014_unicode_term() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("café", id, 10.0);
        assert!(index.contains("café"));
        let postings = index.get("café").unwrap();
        assert_eq!(postings.len(), 1);
        assert_eq!(postings[0].term_frequency, 1);
    }

    #[test]
    fn ii_015_unicode_multi_codepoint_term() {
        let mut index = InvertedIndex::new();
        let id = WorkspaceId::generate();
        index.insert("🏳️‍🌈", id, 10.0);
        assert!(index.contains("🏳️‍🌈"));
        assert_eq!(index.get("🏳️‍🌈").unwrap().len(), 1);
    }

    #[test]
    fn ii_016_unicode_and_ascii_coexist() {
        let mut index = InvertedIndex::new();
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        index.insert("hello", id1, 10.0);
        index.insert("café", id2, 15.0);
        assert_eq!(index.get("hello").unwrap().len(), 1);
        assert_eq!(index.get("café").unwrap().len(), 1);
        assert!(index.get("hello").unwrap()[0].document_id != index.get("café").unwrap()[0].document_id);
    }

    #[test]
    fn ii_017_remove_document_cleans_all_postings() {
        let mut index = InvertedIndex::new();
        let doc1 = WorkspaceId::generate();
        let doc2 = WorkspaceId::generate();
        let doc3 = WorkspaceId::generate();

        index.insert("hello", doc1, 10.0);
        index.insert("hello", doc2, 15.0);
        index.insert("hello", doc3, 20.0);
        index.insert("world", doc1, 5.0);
        index.insert("world", doc2, 8.0);
        index.insert("world", doc3, 12.0);
        index.insert("search", doc2, 30.0);

        assert_eq!(index.document_frequency("hello"), 3);
        assert_eq!(index.document_frequency("world"), 3);
        assert_eq!(index.document_frequency("search"), 1);

        index.remove_document(doc2);

        assert!(
            index.get("hello").map(|pl| !pl.iter().any(|p| p.document_id == doc2)).unwrap_or(true),
            "doc2 should have no posting in hello"
        );
        assert!(
            index.get("world").map(|pl| !pl.iter().any(|p| p.document_id == doc2)).unwrap_or(true),
            "doc2 should have no posting in world"
        );
        assert!(
            index.get("search").map(|pl| !pl.iter().any(|p| p.document_id == doc2)).unwrap_or(true),
            "doc2 should have no posting in search"
        );

        let hello_postings = index.get("hello").unwrap();
        assert_eq!(hello_postings.len(), 2);
        assert!(hello_postings.iter().any(|p| p.document_id == doc1));
        assert!(hello_postings.iter().any(|p| p.document_id == doc3));

        let world_postings = index.get("world").unwrap();
        assert_eq!(world_postings.len(), 2);
        assert!(world_postings.iter().any(|p| p.document_id == doc1));
        assert!(world_postings.iter().any(|p| p.document_id == doc3));

        assert!(index.get("search").is_none(), "search term should be gone after removing only doc");

        assert_eq!(index.document_frequency("hello"), 2);
        assert_eq!(index.document_frequency("world"), 2);
        assert!(index.document_lengths.get(&doc2).is_none());
    }
}
