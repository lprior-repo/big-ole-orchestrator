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

        let posting_list = self
            .index
            .entry(term.to_string())
            .or_insert_with(PostingList::new);

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
}
