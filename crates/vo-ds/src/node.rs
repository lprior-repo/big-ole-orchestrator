//! BTree node implementation.

use serde::{Deserialize, Serialize};

/// Default B-tree order (maximum children per node).
pub const DEFAULT_ORDER: usize = 4;

/// A node in a B-tree.
///
/// Each node contains keys and values, plus optional child pointers.
/// For leaf nodes, `children` is empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BTreeNode<K, V> {
    /// Sorted keys stored in this node.
    pub keys: Vec<K>,
    /// Values associated with keys.
    pub values: Vec<V>,
    /// Child node pointers. Empty for leaf nodes.
    pub children: Vec<BTreeNode<K, V>>,
}

impl<K, V> BTreeNode<K, V> {
    /// Creates a new leaf node with the given keys and values.
    #[must_use]
    pub fn leaf(keys: Vec<K>, values: Vec<V>) -> Self {
        Self {
            keys,
            values,
            children: Vec::new(),
        }
    }

    /// Returns `true` if this node is a leaf (has no children).
    #[must_use]
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Searches for the index where `key` should be inserted or would be found.
    ///
    /// Returns the first index `i` such that `keys[i] >= key`, or `keys.len()` if all keys are less than `key`.
    #[must_use]
    pub fn search_index(&self, key: &K) -> usize
    where
        K: Ord,
    {
        self.keys
            .iter()
            .position(|k| k >= key)
            .unwrap_or(self.keys.len())
    }
}
