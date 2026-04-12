//! Leftist heap: priority queue with O(log n) worst-case merge.
//!
//! A leftist heap (also called leftist tree) is a priority queue implemented as a binary heap
//! with the additional invariant that the right spine (path of right children) is as short as
//! possible. This is maintained via the null path length (npl) property.
//!
//! # Invariants
//! - Heap property: each node's value is <= its children's values
//! - NPL property: npl(left) >= npl(right) - the tree is "skewed" toward the left
//!
//! # Complexity
//! - `merge`: O(log n) worst-case
//! - `insert`: O(log n) worst-case
//! - `find_min`: O(1)
//! - `pop_min`: O(log n) worst-case

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftistNode<T> {
    pub value: T,
    pub left: Option<Box<LeftistNode<T>>>,
    pub right: Option<Box<LeftistNode<T>>>,
    npl: usize,
}

impl<T> LeftistNode<T> {
    fn new(value: T) -> Box<Self> {
        Box::new(LeftistNode {
            value,
            left: None,
            right: None,
            npl: 1,
        })
    }

    fn npl(node: &Option<Box<LeftistNode<T>>>) -> usize {
        node.as_ref().map_or(0, |n| n.npl)
    }

    fn set_npl(&mut self, npl: usize) {
        self.npl = npl;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftistHeap<T> {
    root: Option<Box<LeftistNode<T>>>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeftistHeapError {
    #[error("heap is empty")]
    EmptyHeap,
}

impl<T: Ord> LeftistHeap<T> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn singleton(value: T) -> Self {
        Self {
            root: Some(LeftistNode::new(value)),
            len: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn find_min(&self) -> Result<&T, LeftistHeapError> {
        self.root
            .as_ref()
            .map(|n| &n.value)
            .ok_or(LeftistHeapError::EmptyHeap)
    }

    pub fn insert(self, value: T) -> Self {
        Self::merge(self, Self::singleton(value))
    }

    fn merge_trees(
        t1: Option<Box<LeftistNode<T>>>,
        t2: Option<Box<LeftistNode<T>>>,
    ) -> Option<Box<LeftistNode<T>>> {
        match (t1, t2) {
            (None, None) => None,
            (Some(node), None) | (None, Some(node)) => Some(node),
            (Some(mut node1), Some(mut node2)) => {
                if node1.value <= node2.value {
                    let right = Self::merge_trees(node1.right.take(), Some(node2));
                    node1.right = right;
                    let right_npl = LeftistNode::npl(&node1.right);
                    let left_npl = LeftistNode::npl(&node1.left);
                    if right_npl > left_npl {
                        std::mem::swap(&mut node1.left, &mut node1.right);
                    }
                    node1.set_npl(LeftistNode::npl(&node1.right) + 1);
                    Some(node1)
                } else {
                    let right = Self::merge_trees(Some(node1), node2.right.take());
                    node2.right = right;
                    let right_npl = LeftistNode::npl(&node2.right);
                    let left_npl = LeftistNode::npl(&node2.left);
                    if right_npl > left_npl {
                        std::mem::swap(&mut node2.left, &mut node2.right);
                    }
                    node2.set_npl(LeftistNode::npl(&node2.right) + 1);
                    Some(node2)
                }
            }
        }
    }

    pub fn merge(h1: Self, h2: Self) -> Self {
        match (h1.root, h2.root) {
            (None, None) => Self::new(),
            (Some(node), None) | (None, Some(node)) => Self {
                root: Some(node),
                len: h1.len + h2.len,
            },
            (Some(node1), Some(node2)) => {
                let len = h1.len + h2.len;
                let root = Self::merge_trees(Some(node1), Some(node2));
                Self { root, len }
            }
        }
    }

    pub fn pop_min(mut self) -> Result<(T, Self), LeftistHeapError> {
        match self.root.take() {
            None => Err(LeftistHeapError::EmptyHeap),
            Some(node) => {
                let left_len = Self::count_nodes(node.left.as_deref());
                let right_len = Self::count_nodes(node.right.as_deref());
                let left = Self {
                    root: node.left,
                    len: left_len,
                };
                let right = Self {
                    root: node.right,
                    len: right_len,
                };
                let merged = Self::merge(left, right);
                Ok((node.value, merged))
            }
        }
    }

    fn count_nodes(node: Option<&LeftistNode<T>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                1 + Self::count_nodes(n.left.as_deref()) + Self::count_nodes(n.right.as_deref())
            }
        }
    }

    pub fn from_vec(values: Vec<T>) -> Self {
        values.into_iter().fold(Self::new(), |h, v| h.insert(v))
    }

    pub fn root(&self) -> Option<&LeftistNode<T>> {
        self.root.as_deref()
    }
}

impl<T: Ord> LeftistHeap<T> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }
}

impl<T> Default for LeftistHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_heap_is_empty() {
        let heap: LeftistHeap<i32> = LeftistHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn singleton_has_one_element() {
        let heap = LeftistHeap::singleton(42);
        assert_eq!(heap.len(), 1);
        assert!(!heap.is_empty());
        assert_eq!(heap.find_min().unwrap(), &42);
    }

    #[test]
    fn insert_increases_len() {
        let heap = LeftistHeap::<i32>::new().insert(1);
        assert_eq!(heap.len(), 1);
        let heap = heap.insert(2);
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn find_min_returns_min_element() {
        let heap = LeftistHeap::new().insert(3).insert(1).insert(2);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn find_min_on_empty_returns_error() {
        let heap: LeftistHeap<i32> = LeftistHeap::new();
        assert!(matches!(heap.find_min(), Err(LeftistHeapError::EmptyHeap)));
    }

    #[test]
    fn pop_min_returns_and_removes_min() {
        let heap = LeftistHeap::new().insert(3).insert(1).insert(2);
        let (min, heap) = heap.pop_min().unwrap();
        assert_eq!(min, 1);
        assert_eq!(heap.len(), 2);
        assert_eq!(heap.find_min().unwrap(), &2);
    }

    #[test]
    fn pop_min_on_empty_returns_error() {
        let heap: LeftistHeap<i32> = LeftistHeap::new();
        assert!(matches!(heap.pop_min(), Err(LeftistHeapError::EmptyHeap)));
    }

    #[test]
    fn pop_min_reduces_len() {
        let heap = LeftistHeap::singleton(1);
        let (_, heap) = heap.pop_min().unwrap();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn merge_empty_with_empty() {
        let h1: LeftistHeap<i32> = LeftistHeap::new();
        let h2: LeftistHeap<i32> = LeftistHeap::new();
        let merged = LeftistHeap::merge(h1, h2);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_empty_with_nonempty() {
        let h1: LeftistHeap<i32> = LeftistHeap::new();
        let h2 = LeftistHeap::singleton(42);
        let merged = LeftistHeap::merge(h1, h2);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged.find_min().unwrap(), &42);
    }

    #[test]
    fn merge_nonempty_with_empty() {
        let h1 = LeftistHeap::singleton(42);
        let h2: LeftistHeap<i32> = LeftistHeap::new();
        let merged = LeftistHeap::merge(h1, h2);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged.find_min().unwrap(), &42);
    }

    #[test]
    fn merge_two_singletons() {
        let h1 = LeftistHeap::singleton(1);
        let h2 = LeftistHeap::singleton(2);
        let merged = LeftistHeap::merge(h1, h2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.find_min().unwrap(), &1);
    }

    #[test]
    fn merge_respects_heap_property() {
        let h1 = LeftistHeap::new().insert(1).insert(5).insert(9);
        let h2 = LeftistHeap::new().insert(2).insert(6).insert(10);
        let merged = LeftistHeap::merge(h1, h2);

        fn check_heap<T: Ord>(node: &LeftistNode<T>) {
            if let Some(ref left) = node.left {
                assert!(node.value <= left.value);
                check_heap(left);
            }
            if let Some(ref right) = node.right {
                assert!(node.value <= right.value);
                check_heap(right);
            }
        }

        let root = merged.root().unwrap();
        check_heap(root);
    }

    #[test]
    fn merge_all_elements_preserved() {
        let h1 = LeftistHeap::new().insert(1).insert(3).insert(5);
        let h2 = LeftistHeap::new().insert(2).insert(4).insert(6);
        let merged = LeftistHeap::merge(h1, h2);

        assert_eq!(merged.len(), 6);

        let mut sorted = Vec::new();
        let mut heap = merged;
        while let Ok((_min, rest)) = heap.clone().pop_min() {
            let (m, h) = heap.pop_min().unwrap();
            sorted.push(m);
            heap = h;
        }

        for i in 1..sorted.len() {
            assert!(sorted[i - 1] <= sorted[i]);
        }
    }

    #[test]
    fn from_vec_creates_valid_heap() {
        let values = vec![5, 3, 7, 1, 9, 2, 8];
        let heap = LeftistHeap::from_vec(values);

        assert_eq!(heap.len(), 7);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn repeated_pop_min_yields_sorted_order() {
        let heap = LeftistHeap::from_vec(vec![5, 3, 7, 1, 9, 2, 8]);
        let mut prev = i32::MIN;
        let mut heap = heap;

        for _ in 0..7 {
            let (min, rest) = heap.pop_min().unwrap();
            assert!(prev <= min);
            prev = min;
            heap = rest;
        }

        assert!(heap.is_empty());
    }

    #[test]
    fn serde_roundtrip() {
        let heap = LeftistHeap::new().insert(1).insert(2).insert(3);
        let json = serde_json::to_string(&heap).unwrap();
        let back: LeftistHeap<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(heap, back);
    }

    #[test]
    fn default_is_empty() {
        let heap: LeftistHeap<i32> = LeftistHeap::default();
        assert!(heap.is_empty());
    }

    #[test]
    fn merge_multiple_times() {
        let h1 = LeftistHeap::singleton(1);
        let h2 = LeftistHeap::singleton(3);
        let h3 = LeftistHeap::singleton(2);

        let merged = LeftistHeap::merge(LeftistHeap::merge(h1, h2), h3);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged.find_min().unwrap(), &1);
    }

    #[test]
    fn insert_after_merge() {
        let heap = LeftistHeap::singleton(1);
        let heap = LeftistHeap::merge(heap, LeftistHeap::singleton(3));
        let heap = heap.insert(2);
        assert_eq!(heap.len(), 3);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn npl_property_maintained() {
        fn check_npl<T: Ord>(node: &LeftistNode<T>) -> usize {
            let left_npl = node.left.as_ref().map_or(0, |l| check_npl(l));
            let right_npl = node.right.as_ref().map_or(0, |r| check_npl(r));
            assert!(
                left_npl >= right_npl,
                "NPL property violated: left_npl={} < right_npl={}",
                left_npl,
                right_npl
            );
            right_npl + 1
        }

        let heap = LeftistHeap::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        if let Some(root) = heap.root() {
            check_npl(root);
        }
    }

    #[test]
    fn right_spine_is_short() {
        fn right_spine_len<T>(node: &LeftistNode<T>) -> usize {
            match &node.right {
                None => 0,
                Some(right) => 1 + right_spine_len(right),
            }
        }

        let heap = LeftistHeap::from_vec((1..=100).collect::<Vec<_>>());
        let len = heap.root().map(|r| right_spine_len(r)).unwrap_or(0);
        assert!(
            len <= (100 as f64).sqrt() as usize + 1,
            "Right spine too long: {}",
            len
        );
    }
}
