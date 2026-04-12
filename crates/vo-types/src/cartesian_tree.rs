//! Cartesian tree: BST by key, min-heap by priority.

use serde::{Deserialize, Serialize};

/// A single node in the Cartesian tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartesianNode<K, P> {
    pub key: K,
    pub priority: P,
    pub left: Option<Box<CartesianNode<K, P>>>,
    pub right: Option<Box<CartesianNode<K, P>>>,
}

/// Cartesian tree: BST by key, min-heap by priority.
///
/// Keys must be sorted in strictly ascending order at construction time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartesianTree<K, P> {
    root: Option<CartesianNode<K, P>>,
    len: usize,
}

/// Error from Cartesian tree operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CartesianTreeError {
    #[error("cannot build a Cartesian tree from zero entries")]
    EmptyInput,

    #[error("keys must be sorted in strictly ascending order; violation at index {index}")]
    UnsortedKeys { index: usize },

    #[error("key not found")]
    KeyNotFound,

    #[error("invalid range: start {start} > end {end}")]
    InvalidRange { start: usize, end: usize },

    #[error("range end {end} exceeds tree length {len}")]
    RangeOverflow { end: usize, len: usize },
}

impl<K: Ord + Clone, P: Ord + Clone> CartesianTree<K, P> {
    /// Build a Cartesian tree from sorted (key, priority) pairs in O(n).
    pub fn build(entries: Vec<(K, P)>) -> Result<Self, CartesianTreeError> {
        if entries.is_empty() {
            return Err(CartesianTreeError::EmptyInput);
        }
        for i in 1..entries.len() {
            if entries[i].0 <= entries[i - 1].0 {
                return Err(CartesianTreeError::UnsortedKeys { index: i });
            }
        }

        let len = entries.len();

        // Index-based: track child indices, then convert to owned tree.
        let mut left: Vec<Option<usize>> = vec![None; len];
        let mut right: Vec<Option<usize>> = vec![None; len];
        let keys: Vec<K> = entries.iter().map(|(k, _)| k.clone()).collect();
        let priorities: Vec<P> = entries.iter().map(|(_, p)| p.clone()).collect();

        let mut stack: Vec<usize> = Vec::with_capacity(len);
        for i in 0..len {
            let mut last_popped: Option<usize> = None;
            while let Some(&top) = stack.last() {
                if priorities[top] > priorities[i] {
                    last_popped = stack.pop();
                } else {
                    break;
                }
            }
            if let Some(popped) = last_popped {
                left[i] = Some(popped);
            }
            if let Some(&top) = stack.last() {
                right[top] = Some(i);
            }
            stack.push(i);
        }

        let root_idx = stack[0];

        // Recursively convert index graph to owned tree nodes.
        fn to_node<K, P>(
            keys: &[K],
            priorities: &[P],
            left: &[Option<usize>],
            right: &[Option<usize>],
            idx: usize,
        ) -> CartesianNode<K, P>
        where
            K: Clone,
            P: Clone,
        {
            CartesianNode {
                key: keys[idx].clone(),
                priority: priorities[idx].clone(),
                left: left[idx].map(|l| Box::new(to_node(keys, priorities, left, right, l))),
                right: right[idx].map(|r| Box::new(to_node(keys, priorities, left, right, r))),
            }
        }

        let root = to_node(&keys, &priorities, &left, &right, root_idx);

        Ok(Self {
            root: Some(root),
            len,
        })
    }

    /// Look up the priority for `key` via BST search.
    pub fn find(&self, key: &K) -> Result<&P, CartesianTreeError> {
        let mut node = self.root.as_ref();
        while let Some(n) = node {
            match key.cmp(&n.key) {
                std::cmp::Ordering::Less => node = n.left.as_deref(),
                std::cmp::Ordering::Equal => return Ok(&n.priority),
                std::cmp::Ordering::Greater => node = n.right.as_deref(),
            }
        }
        Err(CartesianTreeError::KeyNotFound)
    }

    /// Find the key with minimum priority in the inclusive index range [start, end].
    pub fn range_min(
        &self,
        start: usize,
        end: usize,
    ) -> Result<(&K, &P), CartesianTreeError> {
        if start > end {
            return Err(CartesianTreeError::InvalidRange { start, end });
        }
        if end >= self.len {
            return Err(CartesianTreeError::RangeOverflow {
                end,
                len: self.len,
            });
        }
        let root = self.root.as_ref().ok_or(CartesianTreeError::EmptyInput)?;
        let mut entries: Vec<(&K, &P)> = Vec::with_capacity(self.len);
        Self::collect_entries(root, &mut entries);
        entries[start..=end]
            .iter()
            .min_by_key(|(_, p)| *p)
            .copied()
            .ok_or(CartesianTreeError::InvalidRange { start, end })
    }

    fn collect_entries<'a>(node: &'a CartesianNode<K, P>, out: &mut Vec<(&'a K, &'a P)>) {
        if let Some(ref l) = node.left {
            Self::collect_entries(l, out);
        }
        out.push((&node.key, &node.priority));
        if let Some(ref r) = node.right {
            Self::collect_entries(r, out);
        }
    }
}

impl<K, P> CartesianTree<K, P> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn root(&self) -> Option<&CartesianNode<K, P>> {
        self.root.as_ref()
    }
}

impl<K, P> Default for CartesianTree<K, P> {
    fn default() -> Self {
        Self {
            root: None,
            len: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_empty_input_returns_error() {
        let result = CartesianTree::<i32, i32>::build(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn build_valid_sorted_input_succeeds() {
        let result = CartesianTree::build(vec![(1, 5), (2, 3), (3, 7)]);
        assert!(result.is_ok());
    }

    #[test]
    fn len_returns_entry_count() {
        let tree = CartesianTree::build(vec![(1, 5), (2, 3), (3, 7)]).unwrap();
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn build_rejects_unsorted_keys() {
        let entries = vec![(1, 5), (3, 2), (2, 7)];
        let result = CartesianTree::build(entries);
        assert!(result.is_err());
    }

    #[test]
    fn root_has_minimum_priority() {
        let tree = CartesianTree::build(vec![(1, 5), (2, 2), (3, 7)]).unwrap();
        let root = tree.root().unwrap();
        assert_eq!(root.key, 2);
        assert_eq!(root.priority, 2);
    }

    #[test]
    fn single_element_tree() {
        let tree = CartesianTree::build(vec![(42, 7)]).unwrap();
        assert_eq!(tree.len(), 1);
        let root = tree.root().unwrap();
        assert_eq!(root.key, 42);
        assert!(root.left.is_none());
        assert!(root.right.is_none());
    }

    #[test]
    fn min_heap_invariant() {
        let tree = CartesianTree::build(vec![
            (1, 9), (2, 3), (3, 7), (4, 1), (5, 5), (6, 2), (7, 8),
        ])
        .unwrap();

        fn check<K, P: Ord>(node: &CartesianNode<K, P>) {
            if let Some(ref l) = node.left {
                assert!(node.priority <= l.priority);
                check(l);
            }
            if let Some(ref r) = node.right {
                assert!(node.priority <= r.priority);
                check(r);
            }
        }
        check(tree.root().unwrap());
    }

    #[test]
    fn bst_invariant() {
        let tree = CartesianTree::build(vec![
            (1, 9), (2, 3), (3, 7), (4, 1), (5, 5), (6, 2), (7, 8),
        ])
        .unwrap();

        fn check<K: Ord, P>(node: &CartesianNode<K, P>) {
            if let Some(ref l) = node.left {
                assert!(l.key < node.key);
                check(l);
            }
            if let Some(ref r) = node.right {
                assert!(node.key < r.key);
                check(r);
            }
        }
        check(tree.root().unwrap());
    }

    #[test]
    fn find_returns_correct_priority() {
        let tree = CartesianTree::build(vec![
            (1, 9), (2, 3), (3, 7), (4, 1), (5, 5), (6, 2), (7, 8),
        ])
        .unwrap();
        assert_eq!(*tree.find(&1).unwrap(), 9);
        assert_eq!(*tree.find(&4).unwrap(), 1);
        assert_eq!(*tree.find(&7).unwrap(), 8);
    }

    #[test]
    fn find_missing_key_returns_error() {
        let tree = CartesianTree::build(vec![(1, 5), (2, 3), (3, 7)]).unwrap();
        assert!(matches!(tree.find(&99), Err(CartesianTreeError::KeyNotFound)));
    }

    #[test]
    fn range_min_full_range() {
        let tree = CartesianTree::build(vec![
            (1, 9), (2, 3), (3, 7), (4, 1), (5, 5), (6, 2), (7, 8),
        ])
        .unwrap();
        let (k, p) = tree.range_min(0, 6).unwrap();
        assert_eq!(*k, 4);
        assert_eq!(*p, 1);
    }

    #[test]
    fn range_min_partial() {
        let tree = CartesianTree::build(vec![(1, 9), (2, 3), (3, 7)]).unwrap();
        let (k, p) = tree.range_min(0, 2).unwrap();
        assert_eq!(*k, 2);
        assert_eq!(*p, 3);
    }

    #[test]
    fn range_min_rejects_invalid_range() {
        let tree = CartesianTree::build(vec![(1, 5), (2, 3)]).unwrap();
        assert!(matches!(
            tree.range_min(5, 3),
            Err(CartesianTreeError::InvalidRange { .. })
        ));
    }

    #[test]
    fn serde_roundtrip() {
        let tree = CartesianTree::build(vec![(1, 9), (2, 3), (3, 7)]).unwrap();
        let json = serde_json::to_string(&tree).unwrap();
        let back: CartesianTree<i32, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(tree, back);
    }

    #[test]
    fn default_is_empty() {
        let tree: CartesianTree<i32, i32> = CartesianTree::default();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }
}
