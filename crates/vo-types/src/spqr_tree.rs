//! SPQR Tree — Triconnected Components Decomposition
//!
//! Provides O(n) construction of the SPQR tree representing the triconnected
//! components of a biconnected graph. Used for dynamic graph connectivity
//! and planar graph embedding queries.
//!
//! Based on Gutwenger & Mutzel (2001) linear-time algorithm.

pub use spqr_tree::decomposition::SPQRDecomposition;
pub use spqr_tree::decomposition::{Block, Component, CutNode, SPQREdge, SPQRNode, SPQRNodeType};
pub use spqr_tree::graph::StaticGraph;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpqrError {
    #[error("graph is not biconnected")]
    GraphNotBiconnected,
    #[error("invalid node index: {0}")]
    InvalidNode(usize),
    #[error("invalid edge index: {0}")]
    InvalidEdge(usize),
    #[error("SPQR build error: {0}")]
    BuildError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        assert_eq!(
            SpqrError::GraphNotBiconnected.to_string(),
            "graph is not biconnected"
        );
        assert_eq!(
            SpqrError::InvalidNode(5).to_string(),
            "invalid node index: 5"
        );
        assert_eq!(
            SpqrError::InvalidEdge(3).to_string(),
            "invalid edge index: 3"
        );
    }

    #[test]
    fn error_eq() {
        assert_eq!(
            SpqrError::GraphNotBiconnected,
            SpqrError::GraphNotBiconnected
        );
        assert_eq!(SpqrError::InvalidNode(1), SpqrError::InvalidNode(1));
        assert_ne!(SpqrError::InvalidNode(1), SpqrError::InvalidNode(2));
    }
}
