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

mod node;
mod tree;

#[cfg(test)]
mod tests;

pub use node::BTreeNode;
pub use tree::{BTree, BTreeError};
