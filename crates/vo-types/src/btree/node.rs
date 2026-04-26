use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BTreeNode<K, V> {
    pub keys: Vec<K>,
    pub values: Vec<V>,
    pub children: Vec<BTreeNode<K, V>>,
}

impl<K, V> BTreeNode<K, V> {
    pub(crate) fn leaf(keys: Vec<K>, values: Vec<V>) -> Self {
        Self {
            keys,
            values,
            children: Vec::new(),
        }
    }

    pub(crate) fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub(crate) fn search_index(&self, key: &K) -> usize
    where
        K: Ord,
    {
        self.keys
            .iter()
            .position(|k| k >= key)
            .unwrap_or(self.keys.len())
    }
}

pub(crate) enum InsertResult<K, V> {
    Done(BTreeNode<K, V>),
    Updated(BTreeNode<K, V>),
    Split(BTreeNode<K, V>, K, V, BTreeNode<K, V>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BTreeError {
    #[error("key not found")]
    KeyNotFound,
}
