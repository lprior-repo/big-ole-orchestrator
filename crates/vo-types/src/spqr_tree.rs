//! SPQR Tree — Triconnected Components Decomposition
//!
//! Provides O(n) construction of the SPQR tree representing the triconnected
//! components of a biconnected graph. Used for dynamic graph connectivity
//! and planar graph embedding queries.
//!
//! Based on Gutwenger & Mutzel (2001) linear-time algorithm.

use std::fmt;

pub use spqr_tree::decomposition::SPQRDecomposition;
pub use spqr_tree::decomposition::{Block, Component, CutNode, SPQREdge, SPQRNode, SPQRNodeType};
pub use spqr_tree::graph::StaticGraph;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpqrError {
    GraphNotBiconnected,
    InvalidNode(usize),
    InvalidEdge(usize),
    BuildError(String),
}

impl fmt::Display for SpqrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpqrError::GraphNotBiconnected => write!(f, "graph is not biconnected"),
            SpqrError::InvalidNode(i) => write!(f, "invalid node index: {i}"),
            SpqrError::InvalidEdge(i) => write!(f, "invalid edge index: {i}"),
            SpqrError::BuildError(s) => write!(f, "SPQR build error: {s}"),
        }
    }
}

impl std::error::Error for SpqrError {}

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
