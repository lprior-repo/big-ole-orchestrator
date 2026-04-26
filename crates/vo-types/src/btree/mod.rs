//! B-tree: balanced search tree with O(log n) insert, search, delete, and range queries.
//!
//! Maintains sorted key-value pairs with guaranteed logarithmic depth through
//! node splitting and merging. Internal nodes hold keys and child pointers;
//! leaf nodes hold keys and values.
//!
//! # Invariants
//! - Every node (except root) has at least `min_keys()` entries.
//! - Every node has at most `max_keys()` entries.
//! - All leaves are at the same depth.
//! - Internal node keys partition child subtrees: keys[i] < all keys in subtree[i+1].
//!
//! # Complexity
//! - `search`: O(log_b n)
//! - `insert`: O(log_b n)
//! - `delete`: O(log_b n)
//! - `range_scan`: O(k + log_b n) where k is the number of results

mod delete;
mod insert;
mod node;
mod query;

#[cfg(test)]
mod tests;

pub use node::{BTreeError, BTreeNode};

use node::{BTreeNode as Node, InsertResult};
use serde::{Deserialize, Serialize};

const DEFAULT_ORDER: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BTree<K, V> {
    root: Option<Node<K, V>>,
    order: usize,
    len: usize,
}

impl<K, V> BTree<K, V> {
    #[must_use]
    pub fn new() -> Self {
        Self::with_order(DEFAULT_ORDER)
    }

    #[must_use]
    pub fn with_order(order: usize) -> Self {
        assert!(order >= 3, "B-tree order must be at least 3");
        Self {
            root: None,
            order,
            len: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn max_keys(&self) -> usize {
        self.order - 1
    }

    fn min_keys(&self) -> usize {
        (self.order - 1) / 2
    }
}

impl<K: Ord + Clone, V: Clone> BTree<K, V> {
    pub fn insert(&mut self, key: K, value: V) {
        if self.root.is_none() {
            self.root = Some(Node::leaf(vec![key], vec![value]));
            self.len = 1;
            return;
        }

        let root = self
            .root
            .take()
            .expect("btree root missing after is_none check");
        let split = self.insert_recursive(root, key, value);

        let is_new_key = !matches!(split, InsertResult::Updated(_));
        match split {
            InsertResult::Done(node) | InsertResult::Updated(node) => {
                self.root = Some(node);
            }
            InsertResult::Split(left, median_key, median_val, right) => {
                let new_root = Node {
                    keys: vec![median_key],
                    values: vec![median_val],
                    children: vec![left, right],
                };
                self.root = Some(new_root);
            }
        }
        if is_new_key {
            self.len += 1;
        }
    }

    pub fn delete(&mut self, key: &K) -> Result<V, BTreeError> {
        if self.root.is_none() {
            return Err(BTreeError::KeyNotFound);
        }

        if self.search(key).is_none() {
            return Err(BTreeError::KeyNotFound);
        }

        let root = self
            .root
            .take()
            .expect("btree root missing after search check");
        let (updated_root, removed) = self
            .delete_recursive(root, key)
            .expect("search confirmed key exists");
        self.len = self.len.saturating_sub(1);

        if updated_root.keys.is_empty() {
            if updated_root.is_leaf() {
                self.root = None;
            } else {
                self.root = Some(
                    updated_root
                        .children
                        .into_iter()
                        .next()
                        .expect("btree children missing despite internal node"),
                );
            }
        } else {
            self.root = Some(updated_root);
        }

        Ok(removed)
    }
}

impl<K, V> Default for BTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone, V: Clone> From<Vec<(K, V)>> for BTree<K, V> {
    fn from(pairs: Vec<(K, V)>) -> Self {
        let mut tree = Self::new();
        for (k, v) in pairs {
            tree.insert(k, v);
        }
        tree
    }
}
