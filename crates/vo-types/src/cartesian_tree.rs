//! Cartesian tree (treap): a randomized binary search tree with O(log n) expected
//! insert, search, and delete. Maintains the BST invariant on keys and the min-heap
//! invariant on priorities. Uses priority-based rotations to self-balance.
//!
//! # Invariants
//! - **BST property**: for every node, all keys in the left subtree are less than
//!   the node's key, and all keys in the right subtree are greater.
//! - **Heap property (min-heap)**: every node's priority is less than or equal to
//!   the priorities of its children.
//!
//! # Complexity (expected, with random priorities)
//! - `insert`: O(log n)
//! - `delete`: O(log n)
//! - `search`: O(log n)
//! - `split` / `merge`: O(log n)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartesianNode<K, T: Clone> {
    pub key: K,
    pub value: T,
    pub priority: u64,
    pub left: Option<Box<CartesianNode<K, T>>>,
    pub right: Option<Box<CartesianNode<K, T>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartesianTree<K, T: Clone> {
    root: Option<Box<CartesianNode<K, T>>>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CartesianTreeError {
    #[error("empty tree")]
    EmptyTree,
    #[error("duplicate key")]
    DuplicateKey,
    #[error("key not found")]
    KeyNotFound,
}

impl<K, T: Clone> CartesianNode<K, T> {
    fn new(key: K, value: T, priority: u64) -> Self {
        Self {
            key,
            value,
            priority,
            left: None,
            right: None,
        }
    }
}

impl<K, T: Clone> CartesianTree<K, T> {
    #[must_use]
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn height(&self) -> usize {
        Self::node_height(&self.root)
    }

    fn node_height(node: &Option<Box<CartesianNode<K, T>>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let lh = Self::node_height(&n.left);
                let rh = Self::node_height(&n.right);
                1 + lh.max(rh)
            }
        }
    }
}

impl<K: Ord, T: Clone> CartesianTree<K, T> {
    #[must_use]
    pub fn search(&self, key: &K) -> Option<&T> {
        Self::search_node(&self.root, key)
    }

    fn search_node<'a>(node: &'a Option<Box<CartesianNode<K, T>>>, key: &K) -> Option<&'a T> {
        node.as_ref().and_then(|n| {
            if key == &n.key {
                Some(&n.value)
            } else if key < &n.key {
                Self::search_node(&n.left, key)
            } else {
                Self::search_node(&n.right, key)
            }
        })
    }

    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.search(key).is_some()
    }

    pub fn insert(&mut self, key: K, value: T, priority: u64) -> Result<(), CartesianTreeError> {
        let new_node = Box::new(CartesianNode::new(key, value, priority));
        match self.root.take() {
            None => {
                self.root = Some(new_node);
                self.len = 1;
                Ok(())
            }
            Some(root) => {
                let (inserted, result) = Self::insert_node(root, new_node)?;
                match result {
                    None => {
                        self.root = Some(inserted);
                        self.len += 1;
                        Ok(())
                    }
                    Some(dup) => {
                        self.root = Some(inserted);
                        Err(dup)
                    }
                }
            }
        }
    }

    fn insert_node(
        mut root: Box<CartesianNode<K, T>>,
        mut new_node: Box<CartesianNode<K, T>>,
    ) -> Result<(Box<CartesianNode<K, T>>, Option<CartesianTreeError>), CartesianTreeError> {
        if new_node.key == root.key {
            root.value = new_node.value;
            root.priority = new_node.priority;
            root.left = new_node.left.take();
            root.right = new_node.right.take();
            Ok((root, Some(CartesianTreeError::DuplicateKey)))
        } else if new_node.priority < root.priority {
            let (left, right) = Self::split_node(Some(root), &new_node.key);
            new_node.left = left;
            new_node.right = right;
            Ok((new_node, None))
        } else if new_node.key < root.key {
            match root.left.take() {
                None => {
                    root.left = Some(new_node);
                }
                Some(left) => {
                    let (inserted, result) = Self::insert_node(left, new_node)?;
                    root.left = Some(inserted);
                    if result.is_some() {
                        return Ok((root, result));
                    }
                }
            }
            if let Some(ref left) = root.left {
                if left.priority < root.priority {
                    root = Self::rotate_right(root);
                }
            }
            Ok((root, None))
        } else {
            match root.right.take() {
                None => {
                    root.right = Some(new_node);
                }
                Some(right) => {
                    let (inserted, result) = Self::insert_node(right, new_node)?;
                    root.right = Some(inserted);
                    if result.is_some() {
                        return Ok((root, result));
                    }
                }
            }
            if let Some(ref right) = root.right {
                if right.priority < root.priority {
                    root = Self::rotate_left(root);
                }
            }
            Ok((root, None))
        }
    }

    pub fn delete(&mut self, key: &K) -> Result<T, CartesianTreeError> {
        let root = self.root.take().ok_or(CartesianTreeError::EmptyTree)?;
        let (new_root, result) = Self::delete_node(root, key);
        self.root = new_root;
        match result {
            Some(value) => {
                self.len -= 1;
                Ok(value)
            }
            None => Err(CartesianTreeError::KeyNotFound),
        }
    }

    fn delete_node(
        mut node: Box<CartesianNode<K, T>>,
        key: &K,
    ) -> (Option<Box<CartesianNode<K, T>>>, Option<T>) {
        if key == &node.key {
            let value = node.value.clone();
            let merged = Self::merge_nodes(node.left.take(), node.right.take());
            (merged, Some(value))
        } else if key < &node.key {
            match node.left.take() {
                None => (Some(node), None),
                Some(left) => {
                    let (new_left, result) = Self::delete_node(left, key);
                    node.left = new_left;
                    (Some(node), result)
                }
            }
        } else {
            match node.right.take() {
                None => (Some(node), None),
                Some(right) => {
                    let (new_right, result) = Self::delete_node(right, key);
                    node.right = new_right;
                    (Some(node), result)
                }
            }
        }
    }

    pub fn split(self, key: &K) -> (Self, Self) {
        let (left, right) = Self::split_node(self.root, key);
        let left_len = Self::count_nodes(&left);
        let right_len = self.len.saturating_sub(left_len);
        (
            Self {
                root: left,
                len: left_len,
            },
            Self {
                root: right,
                len: right_len,
            },
        )
    }

    fn split_node(
        node: Option<Box<CartesianNode<K, T>>>,
        key: &K,
    ) -> (
        Option<Box<CartesianNode<K, T>>>,
        Option<Box<CartesianNode<K, T>>>,
    ) {
        match node {
            None => (None, None),
            Some(mut n) => {
                if key <= &n.key {
                    let (left, right) = Self::split_node(n.left.take(), key);
                    n.left = right;
                    (left, Some(n))
                } else {
                    let (left, right) = Self::split_node(n.right.take(), key);
                    n.right = left;
                    (Some(n), right)
                }
            }
        }
    }

    pub fn merge(self, other: Self) -> Self {
        let merged = Self::merge_nodes(self.root, other.root);
        let len = self.len + other.len;
        Self { root: merged, len }
    }

    fn merge_nodes(
        left: Option<Box<CartesianNode<K, T>>>,
        right: Option<Box<CartesianNode<K, T>>>,
    ) -> Option<Box<CartesianNode<K, T>>> {
        match (left, right) {
            (None, right) => right,
            (left, None) => left,
            (Some(mut l), Some(mut r)) => {
                if l.priority <= r.priority {
                    l.right = Self::merge_nodes(l.right.take(), Some(r));
                    Some(l)
                } else {
                    r.left = Self::merge_nodes(Some(l), r.left.take());
                    Some(r)
                }
            }
        }
    }

    #[expect(clippy::expect_used)]
    fn rotate_left(mut node: Box<CartesianNode<K, T>>) -> Box<CartesianNode<K, T>> {
        #[allow(clippy::expect_used)]
        let mut new_root = node
            .right
            .take()
            .expect("rotate_left called on node with no right child");
        node.right = new_root.left.take();
        new_root.left = Some(node);
        new_root
    }

    #[expect(clippy::expect_used)]
    fn rotate_right(mut node: Box<CartesianNode<K, T>>) -> Box<CartesianNode<K, T>> {
        #[allow(clippy::expect_used)]
        let mut new_root = node
            .left
            .take()
            .expect("rotate_right called on node with no left child");
        node.left = new_root.right.take();
        new_root.right = Some(node);
        new_root
    }

    fn count_nodes(node: &Option<Box<CartesianNode<K, T>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::count_nodes(&n.left) + Self::count_nodes(&n.right),
        }
    }

    #[must_use]
    pub fn min(&self) -> Option<(&K, &T)> {
        let mut node = self.root.as_ref()?;
        loop {
            match node.left.as_ref() {
                Some(left) => node = left,
                None => return Some((&node.key, &node.value)),
            }
        }
    }

    #[must_use]
    pub fn max(&self) -> Option<(&K, &T)> {
        let mut node = self.root.as_ref()?;
        loop {
            match node.right.as_ref() {
                Some(right) => node = right,
                None => return Some((&node.key, &node.value)),
            }
        }
    }

    #[must_use]
    pub fn in_order(&self) -> Vec<(&K, &T)> {
        let mut result = Vec::with_capacity(self.len);
        Self::collect_in_order(&self.root, &mut result);
        result
    }

    fn collect_in_order<'a>(
        node: &'a Option<Box<CartesianNode<K, T>>>,
        result: &mut Vec<(&'a K, &'a T)>,
    ) {
        if let Some(n) = node {
            Self::collect_in_order(&n.left, result);
            result.push((&n.key, &n.value));
            Self::collect_in_order(&n.right, result);
        }
    }

    #[must_use]
    pub fn verify(&self) -> bool {
        match self.root.as_ref() {
            None => true,
            Some(root) => {
                let mut keys_ok = true;
                let mut prio_ok = true;
                Self::verify_node(root.as_ref(), &mut keys_ok, &mut prio_ok);
                keys_ok && prio_ok
            }
        }
    }

    fn verify_node<'a>(
        node: &'a CartesianNode<K, T>,
        keys_ok: &mut bool,
        prio_ok: &mut bool,
    ) -> (Option<&'a K>, Option<&'a K>) {
        let (left_min, _left_max) = if let Some(ref left) = node.left {
            if left.priority < node.priority {
                *prio_ok = false;
            }
            let (lmin, lmax) = Self::verify_node(left.as_ref(), keys_ok, prio_ok);
            if let Some(lmax_key) = lmax {
                if lmax_key >= &node.key {
                    *keys_ok = false;
                }
            }
            (lmin, lmax)
        } else {
            (None, None)
        };

        let (_right_min, right_max) = if let Some(ref right) = node.right {
            if right.priority < node.priority {
                *prio_ok = false;
            }
            let (rmin, rmax) = Self::verify_node(right.as_ref(), keys_ok, prio_ok);
            if let Some(rmin_key) = rmin {
                if rmin_key <= &node.key {
                    *keys_ok = false;
                }
            }
            (rmin, rmax)
        } else {
            (None, None)
        };

        let min_key = left_min.unwrap_or(&node.key);
        let max_key = right_max.unwrap_or(&node.key);
        (Some(min_key), Some(max_key))
    }
}

impl<K, T: Clone> Default for CartesianTree<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_is_empty() {
        let tree: CartesianTree<i32, String> = CartesianTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn default_is_empty() {
        let tree: CartesianTree<i32, i32> = CartesianTree::default();
        assert!(tree.is_empty());
    }

    #[test]
    fn insert_single_element() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a".to_string(), 10).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.search(&1), Some(&"a".to_string()));
    }

    #[test]
    fn insert_multiple_elements() {
        let mut tree = CartesianTree::new();
        tree.insert(5, "e", 50).unwrap();
        tree.insert(3, "c", 30).unwrap();
        tree.insert(1, "a", 10).unwrap();
        tree.insert(4, "d", 40).unwrap();
        tree.insert(2, "b", 20).unwrap();
        assert_eq!(tree.len(), 5);
        for i in 1..=5 {
            assert!(tree.contains(&i), "key {i} should be present");
        }
    }

    #[test]
    fn insert_duplicate_key_returns_error() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        let result = tree.insert(1, "b", 20);
        assert!(matches!(result, Err(CartesianTreeError::DuplicateKey)));
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.search(&1), Some(&"b"));
    }

    #[test]
    fn search_missing_key_returns_none() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        assert_eq!(tree.search(&99), None);
    }

    #[test]
    fn search_empty_tree_returns_none() {
        let tree: CartesianTree<i32, String> = CartesianTree::new();
        assert_eq!(tree.search(&1), None);
    }

    #[test]
    fn contains_key() {
        let mut tree = CartesianTree::new();
        tree.insert(42, "answer", 1).unwrap();
        assert!(tree.contains(&42));
        assert!(!tree.contains(&1));
    }

    #[test]
    fn delete_existing_key() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        tree.insert(2, "b", 20).unwrap();
        tree.insert(3, "c", 30).unwrap();
        let removed = tree.delete(&2).unwrap();
        assert_eq!(removed, "b");
        assert_eq!(tree.len(), 2);
        assert!(!tree.contains(&2));
        assert!(tree.verify());
    }

    #[test]
    fn delete_missing_key_returns_error() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        assert!(matches!(
            tree.delete(&99),
            Err(CartesianTreeError::KeyNotFound)
        ));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn delete_from_empty_tree_returns_error() {
        let mut tree: CartesianTree<i32, String> = CartesianTree::new();
        assert!(matches!(
            tree.delete(&1),
            Err(CartesianTreeError::EmptyTree)
        ));
    }

    #[test]
    fn delete_all_elements() {
        let mut tree = CartesianTree::new();
        for i in 0..20 {
            tree.insert(i, i, (20 - i) as u64).unwrap();
        }
        for i in 0..20 {
            tree.delete(&i).unwrap();
        }
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn delete_root() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        let removed = tree.delete(&1).unwrap();
        assert_eq!(removed, "a");
        assert!(tree.is_empty());
    }

    #[test]
    fn min_returns_smallest_key() {
        let mut tree = CartesianTree::new();
        tree.insert(5, "e", 50).unwrap();
        tree.insert(3, "c", 30).unwrap();
        tree.insert(1, "a", 10).unwrap();
        tree.insert(4, "d", 40).unwrap();
        tree.insert(2, "b", 20).unwrap();
        let (k, v) = tree.min().unwrap();
        assert_eq!(k, &1);
        assert_eq!(v, &"a");
    }

    #[test]
    fn max_returns_largest_key() {
        let mut tree = CartesianTree::new();
        tree.insert(5, "e", 50).unwrap();
        tree.insert(3, "c", 30).unwrap();
        tree.insert(1, "a", 10).unwrap();
        tree.insert(4, "d", 40).unwrap();
        tree.insert(2, "b", 20).unwrap();
        let (k, v) = tree.max().unwrap();
        assert_eq!(k, &5);
        assert_eq!(v, &"e");
    }

    #[test]
    fn min_max_on_empty_returns_none() {
        let tree: CartesianTree<i32, String> = CartesianTree::new();
        assert!(tree.min().is_none());
        assert!(tree.max().is_none());
    }

    #[test]
    fn height_of_empty_tree() {
        let tree: CartesianTree<i32, i32> = CartesianTree::new();
        assert_eq!(tree.height(), 0);
    }

    #[test]
    fn height_of_single_element() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        assert_eq!(tree.height(), 1);
    }

    #[test]
    fn in_order_traversal_sorted() {
        let mut tree = CartesianTree::new();
        tree.insert(5, "e", 50).unwrap();
        tree.insert(3, "c", 30).unwrap();
        tree.insert(1, "a", 10).unwrap();
        tree.insert(4, "d", 40).unwrap();
        tree.insert(2, "b", 20).unwrap();
        let keys: Vec<&i32> = tree.in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![&1, &2, &3, &4, &5]);
    }

    #[test]
    fn verify_empty_tree() {
        let tree: CartesianTree<i32, i32> = CartesianTree::new();
        assert!(tree.verify());
    }

    #[test]
    fn verify_after_inserts() {
        let mut tree = CartesianTree::new();
        for i in 0..100 {
            tree.insert(i, i, (100 - i) as u64).unwrap();
            assert!(tree.verify(), "tree invalid after inserting {i}");
        }
    }

    #[test]
    fn verify_after_deletes() {
        let mut tree = CartesianTree::new();
        for i in 0..100 {
            tree.insert(i, i, i as u64).unwrap();
        }
        for i in (0..100).rev() {
            tree.delete(&i).unwrap();
            assert!(tree.verify(), "tree invalid after deleting {i}");
        }
    }

    #[test]
    fn bst_property_holds() {
        let mut tree = CartesianTree::new();
        let priorities = [30, 10, 50, 20, 40, 60, 15, 25, 35, 45];
        for (i, &p) in priorities.iter().enumerate() {
            tree.insert(i as i32, i, p).unwrap();
        }
        let items = tree.in_order();
        for window in items.windows(2) {
            assert!(window[0].0 < window[1].0, "BST property violated");
        }
    }

    #[test]
    fn heap_property_holds() {
        let mut tree = CartesianTree::new();
        tree.insert(5, "e", 10).unwrap();
        tree.insert(3, "c", 30).unwrap();
        tree.insert(1, "a", 50).unwrap();
        tree.insert(4, "d", 20).unwrap();
        tree.insert(2, "b", 40).unwrap();
        assert!(tree.verify(), "heap property violated");
    }

    #[test]
    fn split_left_contains_keys_less_than_pivot() {
        let mut tree = CartesianTree::new();
        for i in 0..10 {
            tree.insert(i, i, i as u64).unwrap();
        }
        let (left, right) = tree.split(&5);
        assert!(left.verify());
        assert!(right.verify());
        for i in 0..5 {
            assert!(left.contains(&i), "left should contain {i}");
            assert!(!right.contains(&i), "right should not contain {i}");
        }
        for i in 5..10 {
            assert!(!left.contains(&i), "left should not contain {i}");
            assert!(right.contains(&i), "right should contain {i}");
        }
    }

    #[test]
    fn split_at_min() {
        let mut tree = CartesianTree::new();
        for i in 0..10 {
            tree.insert(i, i, i as u64).unwrap();
        }
        let (left, right) = tree.split(&0);
        assert!(left.is_empty());
        assert_eq!(right.len(), 10);
    }

    #[test]
    fn split_at_max() {
        let mut tree = CartesianTree::new();
        for i in 0..10 {
            tree.insert(i, i, i as u64).unwrap();
        }
        let (left, right) = tree.split(&10);
        assert_eq!(left.len(), 10);
        assert!(right.is_empty());
    }

    #[test]
    fn split_empty_tree() {
        let tree: CartesianTree<i32, i32> = CartesianTree::new();
        let (left, right) = tree.split(&5);
        assert!(left.is_empty());
        assert!(right.is_empty());
    }

    #[test]
    fn merge_two_trees() {
        let mut left = CartesianTree::new();
        for i in 0..5 {
            left.insert(i, i, i as u64).unwrap();
        }
        let mut right = CartesianTree::new();
        for i in 5..10 {
            right.insert(i, i, i as u64).unwrap();
        }
        let merged = left.merge(right);
        assert_eq!(merged.len(), 10);
        assert!(merged.verify());
        for i in 0..10 {
            assert!(merged.contains(&i));
        }
        let keys: Vec<&i32> = merged.in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![&0, &1, &2, &3, &4, &5, &6, &7, &8, &9]);
    }

    #[test]
    fn merge_with_empty_left() {
        let left: CartesianTree<i32, i32> = CartesianTree::new();
        let mut right = CartesianTree::new();
        for i in 0..5 {
            right.insert(i, i, i as u64).unwrap();
        }
        let merged = left.merge(right);
        assert_eq!(merged.len(), 5);
        assert!(merged.verify());
    }

    #[test]
    fn merge_with_empty_right() {
        let mut left = CartesianTree::new();
        for i in 0..5 {
            left.insert(i, i, i as u64).unwrap();
        }
        let right: CartesianTree<i32, i32> = CartesianTree::new();
        let merged = left.merge(right);
        assert_eq!(merged.len(), 5);
        assert!(merged.verify());
    }

    #[test]
    fn split_merge_roundtrip() {
        let mut tree = CartesianTree::new();
        for i in 0..20 {
            tree.insert(i, i * 10, i as u64).unwrap();
        }
        let (left, right) = tree.split(&10);
        let rebuilt = left.merge(right);
        assert_eq!(rebuilt.len(), 20);
        assert!(rebuilt.verify());
        for i in 0..20 {
            assert_eq!(rebuilt.search(&i), Some(&(i * 10)));
        }
    }

    #[test]
    fn insert_reverse_order() {
        let mut tree = CartesianTree::new();
        for i in (0..50).rev() {
            tree.insert(i, i, i as u64).unwrap();
        }
        assert_eq!(tree.len(), 50);
        assert!(tree.verify());
        let keys: Vec<&i32> = tree.in_order().iter().map(|(k, _)| *k).collect();
        for i in 0..50 {
            assert_eq!(*keys[i], i as i32);
        }
    }

    #[test]
    fn insert_sorted_order() {
        let mut tree = CartesianTree::new();
        for i in 0..50 {
            tree.insert(i, i, i as u64).unwrap();
        }
        assert_eq!(tree.len(), 50);
        assert!(tree.verify());
    }

    #[test]
    fn interleaved_insert_delete() {
        let mut tree = CartesianTree::new();
        tree.insert(10, 10, 10).unwrap();
        tree.insert(20, 20, 20).unwrap();
        tree.insert(5, 5, 5).unwrap();
        tree.delete(&10).unwrap();
        tree.insert(15, 15, 15).unwrap();
        tree.delete(&5).unwrap();
        tree.insert(25, 25, 25).unwrap();
        tree.delete(&20).unwrap();
        assert!(tree.verify());
        assert_eq!(tree.len(), 2);
        assert!(tree.contains(&15));
        assert!(tree.contains(&25));
    }

    #[test]
    fn sequential_insert_delete_cycle() {
        let mut tree = CartesianTree::new();
        for round in 0..5 {
            for i in 0..50 {
                tree.insert(i, format!("r{round}_v{i}"), i as u64).unwrap();
            }
            assert!(tree.verify());
            for i in 0..50 {
                tree.delete(&i).unwrap();
            }
            assert!(tree.is_empty());
        }
    }

    #[test]
    fn string_keys() {
        let mut tree = CartesianTree::new();
        tree.insert("banana".to_string(), 2, 20).unwrap();
        tree.insert("apple".to_string(), 1, 10).unwrap();
        tree.insert("cherry".to_string(), 3, 30).unwrap();
        assert_eq!(tree.min().unwrap().0, &"apple".to_string());
        assert_eq!(tree.max().unwrap().0, &"cherry".to_string());
        assert!(tree.verify());
    }

    #[test]
    fn serde_roundtrip() {
        let mut tree = CartesianTree::new();
        for i in 0..20 {
            tree.insert(i, i * 10, i as u64).unwrap();
        }
        let json = serde_json::to_string(&tree).unwrap();
        let back: CartesianTree<i32, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(tree.len(), back.len());
        for i in 0..20 {
            assert_eq!(back.search(&i), Some(&(i * 10)));
        }
        assert!(back.verify());
    }

    #[test]
    fn large_scale_insert_and_search() {
        let mut tree = CartesianTree::new();
        let n = 1000;
        for i in 0..n {
            tree.insert(i, i * 2, (n - i) as u64).unwrap();
        }
        assert_eq!(tree.len(), n);
        assert!(tree.verify());
        for i in 0..n {
            assert_eq!(tree.search(&i), Some(&(i * 2)));
        }
    }

    #[test]
    fn large_scale_delete_and_verify() {
        let mut tree = CartesianTree::new();
        let n = 500;
        for i in 0..n {
            tree.insert(i, i, i as u64).unwrap();
        }
        for i in (0..n).step_by(2) {
            tree.delete(&i).unwrap();
        }
        assert_eq!(tree.len(), n / 2);
        assert!(tree.verify());
        for i in (0..n).step_by(2) {
            assert!(tree.search(&i).is_none());
        }
        for i in (1..n).step_by(2) {
            assert_eq!(tree.search(&i), Some(&i));
        }
    }

    #[test]
    fn insert_delete_stress() {
        let mut tree = CartesianTree::new();
        let mut values: Vec<i32> = Vec::new();
        for i in 0..200 {
            tree.insert(i, i, i as u64).unwrap();
            values.push(i);
            assert!(tree.verify());
        }
        let mut rng_seed = 42u64;
        for _ in 0..100 {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let idx = (rng_seed >> 33) as usize % values.len();
            let val = values.remove(idx);
            tree.delete(&val).unwrap();
            assert!(tree.verify());
        }
        assert_eq!(tree.len(), 100);
        for &v in &values {
            assert_eq!(tree.search(&v), Some(&v));
        }
    }

    #[test]
    fn priority_invariant_after_inserts() {
        let mut tree = CartesianTree::new();
        let priorities = [5, 3, 7, 1, 9, 2, 8, 4, 6, 0];
        for (i, &p) in priorities.iter().enumerate() {
            tree.insert(i as i32, i, p).unwrap();
        }
        assert!(tree.verify());
        assert_eq!(tree.root.as_ref().map(|n| n.priority), Some(0));
    }

    #[test]
    fn rotations_preserve_bst() {
        let mut tree = CartesianTree::new();
        tree.insert(4, "d", 1).unwrap();
        tree.insert(2, "b", 5).unwrap();
        tree.insert(6, "f", 3).unwrap();
        tree.insert(1, "a", 7).unwrap();
        tree.insert(3, "c", 9).unwrap();
        tree.insert(5, "e", 8).unwrap();
        tree.insert(7, "g", 2).unwrap();
        assert!(tree.verify());
        let keys: Vec<&i32> = tree.in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![&1, &2, &3, &4, &5, &6, &7]);
    }

    #[test]
    fn delete_leaf() {
        let mut tree = CartesianTree::new();
        tree.insert(2, "root", 5).unwrap();
        tree.insert(1, "left", 10).unwrap();
        tree.insert(3, "right", 10).unwrap();
        tree.delete(&1).unwrap();
        assert!(tree.verify());
        assert_eq!(tree.len(), 2);
        assert!(tree.contains(&2));
        assert!(tree.contains(&3));
    }

    #[test]
    fn delete_node_with_one_child() {
        let mut tree = CartesianTree::new();
        tree.insert(3, "root", 1).unwrap();
        tree.insert(1, "left", 5).unwrap();
        tree.insert(2, "left-right", 10).unwrap();
        tree.delete(&1).unwrap();
        assert!(tree.verify());
        assert!(tree.contains(&2));
        assert!(tree.contains(&3));
    }

    #[test]
    fn delete_node_with_two_children() {
        let mut tree = CartesianTree::new();
        tree.insert(4, "root", 1).unwrap();
        tree.insert(2, "left", 5).unwrap();
        tree.insert(6, "right", 5).unwrap();
        tree.insert(1, "left-left", 10).unwrap();
        tree.insert(3, "left-right", 10).unwrap();
        tree.delete(&2).unwrap();
        assert!(tree.verify());
        assert!(!tree.contains(&2));
        assert!(tree.contains(&1));
        assert!(tree.contains(&3));
        assert!(tree.contains(&4));
        assert!(tree.contains(&6));
    }

    #[test]
    fn split_and_merge_multiple_times() {
        let mut tree = CartesianTree::new();
        for i in 0..30 {
            tree.insert(i, i, i as u64).unwrap();
        }
        for pivot in [5, 10, 15, 20, 25] {
            let (left, right) = tree.split(&pivot);
            assert!(left.verify());
            assert!(right.verify());
            tree = left.merge(right);
            assert!(tree.verify());
            assert_eq!(tree.len(), 30);
        }
    }

    #[test]
    fn merge_preserves_ordering() {
        let mut left = CartesianTree::new();
        for i in [1, 3, 5, 7, 9] {
            left.insert(i, i, i as u64).unwrap();
        }
        let mut right = CartesianTree::new();
        for i in [10, 12, 14, 16, 18] {
            right.insert(i, i, i as u64).unwrap();
        }
        let merged = left.merge(right);
        let keys: Vec<&i32> = merged.in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![&1, &3, &5, &7, &9, &10, &12, &14, &16, &18]);
    }

    #[test]
    fn height_remains_logarithmic() {
        let mut tree = CartesianTree::new();
        let n = 1000;
        for i in 0..n {
            tree.insert(i, i, ((i as u64) * 2654435761) % (n as u64))
                .unwrap();
        }
        let h = tree.height();
        assert!(
            h <= 40,
            "height {h} is unexpectedly large for {n} nodes (expected ~log2({n}) ≈ {})",
            (n as f64).log2() as usize
        );
    }

    #[test]
    fn single_element_split_merge() {
        let mut tree = CartesianTree::new();
        tree.insert(5, "five", 10).unwrap();
        let (left, right) = tree.split(&5);
        assert!(left.is_empty());
        assert_eq!(right.len(), 1);
        let merged = left.merge(right);
        assert_eq!(merged.len(), 1);
        assert!(merged.contains(&5));
    }

    #[test]
    fn delete_then_reinsert() {
        let mut tree = CartesianTree::new();
        tree.insert(1, "a", 10).unwrap();
        tree.insert(2, "b", 20).unwrap();
        tree.insert(3, "c", 30).unwrap();
        tree.delete(&2).unwrap();
        tree.insert(2, "b2", 15).unwrap();
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.search(&2), Some(&"b2"));
        assert!(tree.verify());
    }

    #[test]
    fn from_vec_builds_correct_tree() {
        let items: Vec<(i32, &str, u64)> = vec![(3, "c", 30), (1, "a", 10), (2, "b", 20)];
        let mut tree = CartesianTree::new();
        for (k, v, p) in items {
            tree.insert(k, v, p).unwrap();
        }
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.search(&1), Some(&"a"));
        assert_eq!(tree.search(&2), Some(&"b"));
        assert_eq!(tree.search(&3), Some(&"c"));
        assert!(tree.verify());
    }

    #[test]
    fn range_after_deletes() {
        let mut tree = CartesianTree::new();
        for i in 0..10 {
            tree.insert(i, i, i as u64).unwrap();
        }
        tree.delete(&3).unwrap();
        tree.delete(&7).unwrap();
        let items = tree.in_order();
        let keys: Vec<&i32> = items.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![&0, &1, &2, &4, &5, &6, &8, &9]);
    }

    #[test]
    fn min_max_after_deletes() {
        let mut tree = CartesianTree::new();
        for i in 0..10 {
            tree.insert(i, i, i as u64).unwrap();
        }
        tree.delete(&0).unwrap();
        tree.delete(&9).unwrap();
        assert_eq!(tree.min().unwrap().0, &1);
        assert_eq!(tree.max().unwrap().0, &8);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_insert_search_delete(
            ops in proptest::collection::vec(
                (any::<u16>(), 0u8..3),
                1..200
            )
        ) {
            let mut tree: CartesianTree<u16, u16> = CartesianTree::new();
            let mut present: std::collections::HashSet<u16> = std::collections::HashSet::new();

            for (key, op) in ops {
                match op {
                    0 | 1 => {
                        if !present.contains(&key) {
                            tree.insert(key, key, key as u64).unwrap();
                            present.insert(key);
                        }
                    }
                    2 => {
                        if present.contains(&key) {
                            let val = tree.delete(&key).unwrap();
                            assert_eq!(val, key);
                            present.remove(&key);
                        } else if !tree.is_empty() {
                            assert!(matches!(
                                tree.delete(&key),
                                Err(CartesianTreeError::KeyNotFound)
                            ));
                        }
                    }
                    _ => {}
                }
                prop_assert!(tree.verify());
            }
            prop_assert_eq!(tree.len(), present.len());
        }

        #[test]
        fn proptest_bst_invariant_after_operations(
            keys in proptest::collection::vec(any::<i32>(), 1..300)
        ) {
            let mut tree: CartesianTree<i32, i32> = CartesianTree::new();
            let mut unique: Vec<i32> = keys.clone();
            unique.sort();
            unique.dedup();

            for &k in &unique {
                tree.insert(k, k.wrapping_mul(2), (k.unsigned_abs() as u64) % 1000).unwrap();
            }

            prop_assert!(tree.verify());
            let items = tree.in_order();
            for window in items.windows(2) {
                prop_assert!(window[0].0 < window[1].0, "BST order violated");
            }
        }

        #[test]
        fn proptest_heap_invariant_holds(
            pairs in proptest::collection::vec((any::<u32>(), any::<u64>()), 1..200)
        ) {
            let mut tree: CartesianTree<u32, u32> = CartesianTree::new();
            let mut seen = std::collections::HashSet::new();
            for (k, p) in pairs {
                if seen.insert(k) {
                    tree.insert(k, k, p).unwrap();
                }
            }
            prop_assert!(tree.verify());
        }

        #[test]
        fn proptest_split_merge_roundtrip(
            keys in proptest::collection::vec(any::<i32>(), 1..100),
            pivot in any::<i32>()
        ) {
            let mut tree: CartesianTree<i32, i32> = CartesianTree::new();
            let mut seen = std::collections::HashSet::new();
            for &k in &keys {
                if seen.insert(k) {
                    tree.insert(k, k, k.unsigned_abs() as u64).unwrap();
                }
            }
            let original_len = tree.len();
            let (left, right) = tree.split(&pivot);
            let rebuilt = left.merge(right);
            prop_assert_eq!(rebuilt.len(), original_len);
            prop_assert!(rebuilt.verify());
            for &k in &seen {
                prop_assert!(rebuilt.contains(&k));
            }
        }

        #[test]
        fn proptest_random_sequence_invariants(
            ops in proptest::collection::vec(
                (any::<u8>(), 0u16..8u16),
                1..500
            )
        ) {
            let mut tree: CartesianTree<u16, u16> = CartesianTree::new();
            let mut present: std::collections::HashSet<u16> = std::collections::HashSet::new();

            for (op, key) in ops {
                match op % 3 {
                    0 => {
                        if !present.contains(&key) {
                            tree.insert(key, key, key as u64).unwrap();
                            present.insert(key);
                        }
                    }
                    1 => {
                        if present.contains(&key) {
                            tree.delete(&key).ok();
                            present.remove(&key);
                        }
                    }
                    _ => {
                        if present.contains(&key) {
                            prop_assert_eq!(tree.search(&key), Some(&key));
                        } else {
                            prop_assert!(tree.search(&key).is_none());
                        }
                    }
                }
                prop_assert!(tree.verify());
            }
            prop_assert_eq!(tree.len(), present.len());
            for &k in &present {
                prop_assert!(tree.contains(&k), "present key {k} not found");
            }
        }

        #[test]
        fn proptest_delete_all_remains_valid(
            keys in proptest::collection::vec(any::<u32>(), 1..100)
        ) {
            let mut tree: CartesianTree<u32, u32> = CartesianTree::new();
            let mut seen: Vec<u32> = Vec::new();
            let mut set = std::collections::HashSet::new();
            for &k in &keys {
                if set.insert(k) {
                    tree.insert(k, k, k as u64).unwrap();
                    seen.push(k);
                }
            }
            for &k in &seen {
                tree.delete(&k).unwrap();
                prop_assert!(tree.verify());
            }
            prop_assert!(tree.is_empty());
        }

        #[test]
        fn proptest_min_max_consistency(
            keys in proptest::collection::vec(any::<i32>(), 1..100)
        ) {
            let mut tree: CartesianTree<i32, i32> = CartesianTree::new();
            let mut set = std::collections::HashSet::new();
            for &k in &keys {
                if set.insert(k) {
                    tree.insert(k, k, k.unsigned_abs() as u64).unwrap();
                }
            }
            if !set.is_empty() {
                let min_key = *set.iter().min().unwrap();
                let max_key = *set.iter().max().unwrap();
                prop_assert_eq!(tree.min().unwrap().0, &min_key);
                prop_assert_eq!(tree.max().unwrap().0, &max_key);
            }
        }

        #[test]
        fn proptest_in_order_matches_sorted_keys(
            keys in proptest::collection::vec(any::<i32>(), 1..200)
        ) {
            let mut tree: CartesianTree<i32, i32> = CartesianTree::new();
            let mut set = std::collections::HashSet::new();
            for &k in &keys {
                if set.insert(k) {
                    tree.insert(k, k, k.unsigned_abs() as u64).unwrap();
                }
            }
            let mut expected: Vec<i32> = set.into_iter().collect();
            expected.sort();
            let actual: Vec<i32> = tree.in_order().iter().map(|(k, _)| **k).collect();
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn proptest_serde_roundtrip(
            keys in proptest::collection::vec(any::<u32>(), 1..50)
        ) {
            let mut tree: CartesianTree<u32, u32> = CartesianTree::new();
            let mut set = std::collections::HashSet::new();
            for &k in &keys {
                if set.insert(k) {
                    tree.insert(k, k.wrapping_mul(3), k as u64).unwrap();
                }
            }
            let json = serde_json::to_string(&tree).unwrap();
            let back: CartesianTree<u32, u32> = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(back.len(), tree.len());
            prop_assert!(back.verify());
            for k in &set {
                prop_assert_eq!(back.search(k), Some(&k.wrapping_mul(3)));
            }
        }
    }
}
