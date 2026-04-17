<<<<<<< HEAD
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticGraph {
    _node_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SPQRNodeType {
    Series,
    Parallel,
    Rigid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SPQRNode {
    pub node_type: SPQRNodeType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SPQREdge {
    pub source: usize,
    pub target: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub nodes: Vec<SPQRNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub components: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutNode {
    pub vertex: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SPQRDecomposition {
    _tree: Option<SPQRNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpqrError {
    #[error("invalid graph")]
    InvalidGraph,
    #[error("decomposition failed")]
    DecompositionFailed,
}
=======
#![allow(dead_code)]

pub struct Block;
pub struct Component;
pub struct CutNode;
pub struct SPQRDecomposition;
pub struct SPQREdge;
pub struct SPQRNode;
pub struct SPQRNodeType;
pub struct SpqrError;
pub struct StaticGraph;
>>>>>>> origin/polecat/guzzle-veloxide-4wc
