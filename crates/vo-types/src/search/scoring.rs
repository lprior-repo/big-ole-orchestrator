use crate::search::inverted_index::Posting;

pub trait Scorer {
    fn score(&self, posting: &Posting, idf: f64, avg_doc_len: f64) -> f64;
}

pub struct TfIdfScorer;

impl Scorer for TfIdfScorer {
    fn score(&self, posting: &Posting, idf: f64, _avg_doc_len: f64) -> f64 {
        let tf = posting.term_frequency as f64;
        idf * tf
    }
}

pub struct Bm25Scorer {
    pub k1: f64,
    pub b: f64,
}

impl Default for Bm25Scorer {
    fn default() -> Self {
        Self::new()
    }
}

impl Bm25Scorer {
    pub fn new() -> Self {
        Self { k1: 1.5, b: 0.75 }
    }
}

impl Scorer for Bm25Scorer {
    fn score(&self, posting: &Posting, idf: f64, avg_doc_len: f64) -> f64 {
        let tf = posting.term_frequency as f64;
        let doc_len = posting.document_length as f64;
        let numerator = tf * (self.k1 + 1.0);
        let denominator = tf + self.k1 * (1.0 - self.b + self.b * doc_len / avg_doc_len);
        idf * numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceId;

    fn make_posting(term_freq: u32, doc_len: u32) -> Posting {
        Posting {
            document_id: WorkspaceId::generate(),
            term_frequency: term_freq,
            document_length: doc_len,
            positions: Vec::new(),
        }
    }

    #[test]
    fn tfidf_001_basic_scoring() {
        let scorer = TfIdfScorer;
        let posting = make_posting(2, 10);
        let score = scorer.score(&posting, 1.0, 10.0);
        assert_eq!(score, 2.0);
    }

    #[test]
    fn bm25_001_basic_scoring() {
        let scorer = Bm25Scorer::new();
        let posting = make_posting(2, 10);
        let score = scorer.score(&posting, 1.0, 10.0);
        assert!(score > 0.0);
    }

    #[test]
    fn bm25_002_higher_tf_higher_score() {
        let scorer = Bm25Scorer::new();
        let posting1 = make_posting(1, 10);
        let posting2 = make_posting(3, 10);
        let score1 = scorer.score(&posting1, 1.0, 10.0);
        let score2 = scorer.score(&posting2, 1.0, 10.0);
        assert!(score2 > score1);
    }

    #[test]
    fn bm25_003_longer_doc_lower_score() {
        let scorer = Bm25Scorer::new();
        let short_posting = make_posting(2, 5);
        let long_posting = make_posting(2, 20);
        let score_short = scorer.score(&short_posting, 1.0, 10.0);
        let score_long = scorer.score(&long_posting, 1.0, 10.0);
        assert!(score_short > score_long);
    }
}
