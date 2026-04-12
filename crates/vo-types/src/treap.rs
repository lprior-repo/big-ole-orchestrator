//! Treap: randomized BST with heap property.
//!
//! A treap is a binary search tree where each node has both a key and a priority.
//! - BST property: keys are ordered left < node < right
//! - Heap property: priorities are ordered node < children (min-heap by priority)
//!
//! The priorities are chosen randomly, giving expected O(log n) height for all operations.
//!
//! # Differences from Cartesian Tree
//!
//! - Cartesian Tree: built from sorted input in O(n), priorities determine structure
//! - Treap: nodes inserted one at a time with random priorities, O(log n) expected per insert
//!
//! # Complexity
//!
//! - `insert`: O(log n) expected
//! - `delete`: O(log n) expected
//! - `search`: O(log n) expected
//! - `split`: O(log n) expected
//! - `merge`: O(log n) expected

use rand::Rng;
use serde::{Deserialize, Serialize};

/// A single node in the treap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreapNode<K, P> {
    pub key: K,
    pub priority: P,
    pub left: Option<Box<TreapNode<K, P>>>,
    pub right: Option<Box<TreapNode<K, P>>>,
}

/// Treap: BST by key, min-heap by priority with random priorities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Treap<K, P> {
    root: Option<Box<TreapNode<K, P>>>,
    len: usize,
}

/// Error from treap operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreapError {
    #[error("key not found")]
    KeyNotFound,

    #[error("key already exists")]
    KeyAlreadyExists,
}

impl<K: Ord, P: Ord> Treap<K, P> {
    fn rotate_right(node: Box<TreapNode<K, P>>) -> Box<TreapNode<K, P>> {
        let mut node = node;
        let mut left = node.left.take().unwrap();
        node.left = left.right.take();
        left.right = Some(node);
        left
    }

    fn rotate_left(node: Box<TreapNode<K, P>>) -> Box<TreapNode<K, P>> {
        let mut node = node;
        let mut right = node.right.take().unwrap();
        node.right = right.left.take();
        right.left = Some(node);
        right
    }

    fn insert_rec(
        node: Option<Box<TreapNode<K, P>>>,
        key: K,
        priority: P,
    ) -> Option<Box<TreapNode<K, P>>>
    where
        K: Clone,
        P: Clone,
    {
        match node {
            None => Some(Box::new(TreapNode {
                key,
                priority,
                left: None,
                right: None,
            })),
            Some(mut n) => {
                if key < n.key {
                    n.left = Self::insert_rec(n.left, key, priority);
                    if n.left.as_ref().unwrap().priority < n.priority {
                        n = Self::rotate_right(n);
                    }
                } else if key > n.key {
                    n.right = Self::insert_rec(n.right, key, priority);
                    if n.right.as_ref().unwrap().priority < n.priority {
                        n = Self::rotate_left(n);
                    }
                }
                Some(n)
            }
        }
    }

    /// Insert a key with a given priority.
    pub fn insert(mut self, key: K, priority: P) -> Self {
        self.root = Self::insert_rec(self.root, key, priority);
        self.len += 1;
        self
    }

    fn search_rec(node: &Option<Box<TreapNode<K, P>>>, key: &K) -> Option<&P> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.key {
                    Some(&n.priority)
                } else if *key < n.key {
                    Self::search_rec(&n.left, key)
                } else {
                    Self::search_rec(&n.right, key)
                }
            }
        }
    }

    /// Search for a key, returning its priority if found.
    pub fn search(&self, key: &K) -> Option<&P> {
        Self::search_rec(&self.root, key)
    }

    fn find_min_rec(node: &Option<Box<TreapNode<K, P>>>) -> Option<&K> {
        match node {
            None => None,
            Some(n) => match &n.left {
                None => Some(&n.key),
                Some(_) => Self::find_min_rec(&n.left),
            },
        }
    }

    fn find_max_rec(node: &Option<Box<TreapNode<K, P>>>) -> Option<&K> {
        match node {
            None => None,
            Some(n) => match &n.right {
                None => Some(&n.key),
                Some(_) => Self::find_max_rec(&n.right),
            },
        }
    }

    /// Find the minimum key.
    pub fn find_min(&self) -> Option<&K> {
        Self::find_min_rec(&self.root)
    }

    /// Find the maximum key.
    pub fn find_max(&self) -> Option<&K> {
        Self::find_max_rec(&self.root)
    }

    fn erase_rec(node: Option<Box<TreapNode<K, P>>>, key: &K) -> Option<Box<TreapNode<K, P>>> {
        match node {
            None => None,
            Some(mut n) => {
                if *key < n.key {
                    n.left = Self::erase_rec(n.left, key);
                } else if *key > n.key {
                    n.right = Self::erase_rec(n.right, key);
                } else {
                    if n.left.is_none() {
                        return n.right;
                    } else if n.right.is_none() {
                        return n.left;
                    } else if n.left.as_ref().unwrap().priority < n.right.as_ref().unwrap().priority
                    {
                        n = Self::rotate_right(n);
                        n.right = Self::erase_rec(n.right, key);
                    } else {
                        n = Self::rotate_left(n);
                        n.left = Self::erase_rec(n.left, key);
                    }
                }
                Some(n)
            }
        }
    }

    /// Delete a key from the treap.
    pub fn delete(mut self, key: &K) -> Self {
        self.root = Self::erase_rec(self.root, key);
        self.len -= 1;
        self
    }

    /// Split treap into two: left has keys < split_key, right has keys >= split_key.
    pub fn split(self, split_key: &K) -> (Self, Self) {
        let mut visited: Vec<Box<TreapNode<K, P>>> = Vec::new();
        let mut current = self.root;
        let mut left_tree: Option<Box<TreapNode<K, P>>> = None;
        let mut right_tree: Option<Box<TreapNode<K, P>>> = None;

        while let Some(mut node) = current {
            if *split_key <= node.key {
                right_tree =
                    Self::merge_into(left_tree.take(), right_tree.take(), node.right.take());
                left_tree = node.left.take();
                visited.push(node);
                current = node.right.take();
            } else {
                left_tree = Self::merge_into(left_tree.take(), None, node.left.take());
                right_tree = node.right.take();
                visited.push(node);
                current = node.left.take();
            }
        }

        for node in visited.into_iter().rev() {
            if left_tree.is_none() {
                left_tree = Some(node);
            } else if right_tree.is_none() {
                right_tree = Some(node);
            } else {
                let mut n = node;
                n.left = left_tree.take();
                n.right = right_tree.take();
                if n.priority < n.left.as_ref().unwrap().priority {
                    n = Self::rotate_right(n);
                } else if n.priority < n.right.as_ref().unwrap().priority {
                    n = Self::rotate_left(n);
                }
                left_tree = Some(n);
            }
        }

        (
            Self {
                root: left_tree,
                len: self.len,
            },
            Self {
                root: right_tree,
                len: 0,
            },
        )
    }

    fn merge_into(
        left: Option<Box<TreapNode<K, P>>>,
        _intermediate: Option<Box<TreapNode<K, P>>>,
        right: Option<Box<TreapNode<K, P>>>,
    ) -> Option<Box<TreapNode<K, P>>> {
        match (left, right) {
            (None, None) => None,
            (Some(l), None) | (None, Some(l)) => Some(l),
            (Some(mut l), Some(mut r)) => {
                if l.priority < r.priority {
                    l.right = Self::merge_into(l.right.take(), None, Some(r));
                    Some(l)
                } else {
                    r.left = Self::merge_into(Some(l), None, r.left.take());
                    Some(r)
                }
            }
        }
    }

    /// Merge two treaps where all keys in left < all keys in right.
    pub fn merge(self, other: Self) -> Self {
        if self.root.is_none() {
            return other;
        }
        if other.root.is_none() {
            return self;
        }

        let mut visited: Vec<Box<TreapNode<K, P>>> = Vec::new();
        let mut current: Option<Box<TreapNode<K, P>>> = None;
        let mut left_stack: Vec<Option<Box<TreapNode<K, P>>>> = Vec::new();
        let mut right_stack: Vec<Option<Box<TreapNode<K, P>>>> = Vec::new();

        let mut left = self.root;
        let mut right = other.root;

        loop {
            if let Some(mut node) = left {
                left_stack.push(node.right.take());
                visited.push(node);
                left = node.left.take();
            } else if let Some(mut node) = right {
                right_stack.push(node.left.take());
                visited.push(node);
                right = node.right.take();
            } else if let Some(n) = visited.pop() {
                current = Some(n);
                break;
            } else {
                break;
            }
        }

        let mut result = current;
        let mut pending: Vec<Option<Box<TreapNode<K, P>>>> = Vec::new();

        while let Some(node) = visited.pop() {
            let mut n = node;
            while let Some(child) = result {
                if n.priority < child.priority {
                    if let Some(mut c) = pending.pop() {
                        if n.key < c.as_ref().unwrap().key {
                            n.right = Some(child);
                            c = Self::merge_into(c, None, n.left.take());
                            n.left = c;
                        } else {
                            n.left = Some(child);
                            c = Self::merge_into(c, None, n.right.take());
                            n.right = c;
                        }
                        result = c;
                    } else {
                        if n.key < child.key {
                            n.right = Some(child);
                        } else {
                            n.left = Some(child);
                        }
                        result = Some(n);
                    }
                } else {
                    pending.push(Some(child));
                    if n.key < child.key {
                        n.right = n.left.take();
                        result = n.left.take();
                        n.left = result.take();
                        result = n.right.take();
                        n.right = n.left.take();
                        result = Some(n);
                    } else {
                        n.left = n.right.take();
                        result = n.right.take();
                        n.right = result.take();
                        result = Some(n);
                    }
                }
            }
            if pending.is_empty() {
                result = Some(n);
            }
        }

        Self {
            root: result,
            len: self.len + other.len,
        }
    }

    fn inorder_rec(node: &Option<Box<TreapNode<K, P>>>, result: &mut Vec<&K>) {
        if let Some(n) = node {
            Self::inorder_rec(&n.left, result);
            result.push(&n.key);
            Self::inorder_rec(&n.right, result);
        }
    }

    /// Return keys in sorted order.
    pub fn inorder(&self) -> Vec<&K> {
        let mut result = Vec::new();
        Self::inorder_rec(&self.root, &mut result);
        result
    }

    fn height_rec(node: &Option<Box<TreapNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::height_rec(&n.left), Self::height_rec(&n.right)),
        }
    }

    /// Return the height of the tree.
    pub fn height(&self) -> usize {
        Self::height_rec(&self.root)
    }
}

impl<K, P> Treap<K, P> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn singleton(key: K, priority: P) -> Self {
        Self {
            root: Some(Box::new(TreapNode {
                key,
                priority,
                left: None,
                right: None,
            })),
            len: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn root(&self) -> Option<&TreapNode<K, P>> {
        self.root.as_deref()
    }
}

impl<K, P> Default for Treap<K, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone, P: Ord + Clone> Treap<K, P> {
    /// Insert with a random priority.
    pub fn insert_with_random_priority<R: Rng>(self, key: K, priority_bound: P, rng: &mut R) -> Self
    where
        P: rand::distributions::uniform::SampleUniform,
    {
        let priority = rng.gen_range(P::MIN..=priority_bound);
        self.insert(key, priority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_treap_is_empty() {
        let treap: Treap<i32, i32> = Treap::new();
        assert!(treap.is_empty());
        assert_eq!(treap.len(), 0);
    }

    #[test]
    fn singleton_has_one_element() {
        let treap = Treap::singleton(42, 7);
        assert_eq!(treap.len(), 1);
        assert!(!treap.is_empty());
        assert_eq!(treap.search(&42), Some(&7));
    }

    #[test]
    fn insert_increases_len() {
        let treap = Treap::<i32, i32>::new().insert(1, 10).insert(2, 5);
        assert_eq!(treap.len(), 2);
    }

    #[test]
    fn search_returns_priority() {
        let treap = Treap::new().insert(1, 10).insert(2, 5).insert(3, 15);
        assert_eq!(treap.search(&1), Some(&10));
        assert_eq!(treap.search(&2), Some(&5));
        assert_eq!(treap.search(&3), Some(&15));
    }

    #[test]
    fn search_missing_key_returns_none() {
        let treap = Treap::new().insert(1, 10).insert(2, 5);
        assert_eq!(treap.search(&99), None);
    }

    #[test]
    fn delete_removes_key() {
        let treap = Treap::new().insert(1, 10).insert(2, 5).insert(3, 15);
        let treap = treap.delete(&2);
        assert_eq!(treap.len(), 2);
        assert_eq!(treap.search(&2), None);
        assert_eq!(treap.search(&1), Some(&10));
        assert_eq!(treap.search(&3), Some(&15));
    }

    #[test]
    fn delete_decreases_len() {
        let treap = Treap::singleton(1, 10);
        let treap = treap.delete(&1);
        assert_eq!(treap.len(), 0);
        assert!(treap.is_empty());
    }

    #[test]
    fn find_min_returns_smallest_key() {
        let treap = Treap::new().insert(3, 10).insert(1, 5).insert(2, 15);
        assert_eq!(treap.find_min(), Some(&1));
    }

    #[test]
    fn find_max_returns_largest_key() {
        let treap = Treap::new().insert(3, 10).insert(1, 5).insert(2, 15);
        assert_eq!(treap.find_max(), Some(&2));
    }

    #[test]
    fn inorder_returns_sorted_keys() {
        let treap = Treap::new()
            .insert(3, 10)
            .insert(1, 5)
            .insert(2, 15)
            .insert(4, 3);
        let keys: Vec<&i32> = treap.inorder();
        assert_eq!(keys, vec![&1, &2, &3, &4]);
    }

    #[test]
    fn insert_maintains_bst_invariant() {
        let treap = Treap::new()
            .insert(5, 10)
            .insert(3, 5)
            .insert(7, 15)
            .insert(1, 3)
            .insert(4, 7);

        fn check_bst<K: Ord>(
            node: &Option<Box<TreapNode<K, i32>>>,
            min: Option<&K>,
            max: Option<&K>,
        ) {
            if let Some(n) = node {
                if let Some(min_val) = min {
                    assert!(min_val < &n.key);
                }
                if let Some(max_val) = max {
                    assert!(&n.key < max_val);
                }
                check_bst(&n.left, min, Some(&n.key));
                check_bst(&n.right, Some(&n.key), max);
            }
        }

        check_bst(&treap.root, None, None);
    }

    #[test]
    fn insert_maintains_heap_invariant() {
        let treap = Treap::new()
            .insert(5, 50)
            .insert(3, 30)
            .insert(7, 70)
            .insert(1, 10)
            .insert(4, 40);

        fn check_heap<K, P: Ord>(node: &Option<Box<TreapNode<K, P>>>) {
            if let Some(n) = node {
                if let Some(ref left) = n.left {
                    assert!(n.priority <= left.priority);
                    check_heap(&n.left);
                }
                if let Some(ref right) = n.right {
                    assert!(n.priority <= right.priority);
                    check_heap(&n.right);
                }
            }
        }

        check_heap(&treap.root);
    }

    #[test]
    fn multiple_insert_delete_keeps_invariants() {
        let mut treap = Treap::<i32, i32>::new();
        for i in (0..100).rev() {
            treap = treap.insert(i, i * 2);
        }
        assert_eq!(treap.len(), 100);

        for i in 0..50 {
            treap = treap.delete(&i);
        }
        assert_eq!(treap.len(), 50);

        fn check_bst<K: Ord>(
            node: &Option<Box<TreapNode<K, i32>>>,
            min: Option<&K>,
            max: Option<&K>,
        ) {
            if let Some(n) = node {
                if let Some(min_val) = min {
                    assert!(min_val < &n.key);
                }
                if let Some(max_val) = max {
                    assert!(&n.key < max_val);
                }
                check_bst(&n.left, min, Some(&n.key));
                check_bst(&n.right, Some(&n.key), max);
            }
        }

        check_bst(&treap.root, None, None);
    }

    #[test]
    fn serde_roundtrip() {
        let treap = Treap::new().insert(1, 10).insert(2, 5).insert(3, 15);
        let json = serde_json::to_string(&treap).unwrap();
        let back: Treap<i32, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(treap, back);
    }

    #[test]
    fn default_is_empty() {
        let treap: Treap<i32, i32> = Treap::default();
        assert!(treap.is_empty());
    }

    #[test]
    fn delete_nonexistent_key_leaves_treap_unchanged() {
        let treap = Treap::new().insert(1, 10).insert(2, 5);
        let treap = treap.delete(&99);
        assert_eq!(treap.len(), 2);
        assert_eq!(treap.search(&1), Some(&10));
        assert_eq!(treap.search(&2), Some(&5));
    }

    #[test]
    fn height_grows_with_elements() {
        let treap = Treap::new().insert(1, 10).insert(2, 5).insert(3, 15);
        assert!(treap.height() >= 1);
    }
}
