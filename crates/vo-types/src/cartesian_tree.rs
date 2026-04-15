use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartesianNode<T: Clone> {
    pub value: T,
    pub priority: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartesianTree<T: Clone> {
    _root: Option<CartesianNode<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CartesianTreeError {
    #[error("empty tree")]
    EmptyTree,
    #[error("duplicate key")]
    DuplicateKey,
}
