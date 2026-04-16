use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewNode<T: Ord + Clone> {
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewHeap<T: Ord + Clone> {
    _root: Option<SkewNode<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkewHeapError {
    #[error("empty heap")]
    EmptyHeap,
}
