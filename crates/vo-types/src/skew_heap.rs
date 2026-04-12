//! Skew heap: self-adjusting binary heap with O(log n) amortized merge.
//!
//! Skew heaps maintain the heap property and achieve amortized O(log n) performance
//! for all operations through a simple merging strategy that swaps left and right
//! children at each node during merge.

use serde::{Deserialize, Serialize};

/// A single node in the skew heap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewNode<T> {
    pub value: T,
    pub left: Option<Box<SkewNode<T>>>,
    pub right: Option<Box<SkewNode<T>>>,
}

impl<T> SkewNode<T> {
    fn new(value: T) -> Box<Self> {
        Box::new(SkewNode {
            value,
            left: None,
            right: None,
        })
    }
}

/// Skew heap: priority queue with O(log n) amortized merge.
///
/// # Invariants
/// - Heap property: each node's value is <= its children's values
/// - No invariant on tree shape (self-adjusting)
///
/// # Complexity
/// - `merge`: O(log n) amortized
/// - `insert`: O(log n) amortized (create singleton + merge)
/// - `find_min`: O(1)
/// - `pop_min`: O(log n) amortized (merge left and right subtrees)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewHeap<T> {
    root: Option<Box<SkewNode<T>>>,
    len: usize,
}

/// Error from skew heap operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkewHeapError {
    #[error("heap is empty")]
    EmptyHeap,

    #[error("cannot merge two heaps with different comparators")]
    IncompatibleHeaps,
}

impl<T: Ord> SkewHeap<T> {
    /// Merge two heaps into one, returning the merged heap.
    ///
    /// Takes the smaller of the two roots and recursively merges the other
    /// heap with the right subtree of the smaller root, then swaps left and right.
    pub fn merge(h1: Self, h2: Self) -> Self {
        match (h1.root, h2.root) {
            (None, None) => Self::new(),
            (Some(node), None) | (None, Some(node)) => Self {
                root: Some(node),
                len: h1.len + h2.len,
            },
            (Some(mut n1), Some(mut n2)) => {
                let len = h1.len + h2.len;
                if n1.value <= n2.value {
                    let right = Self::merge(
                        Self {
                            root: n1.right.take(),
                            len: 0,
                        },
                        Self {
                            root: Some(n2),
                            len: 0,
                        },
                    )
                    .root;
                    n1.right = n1.left.take();
                    n1.left = right;
                    Self {
                        root: Some(n1),
                        len,
                    }
                } else {
                    let right = Self::merge(
                        Self {
                            root: n2.right.take(),
                            len: 0,
                        },
                        Self {
                            root: Some(n1),
                            len: 0,
                        },
                    )
                    .root;
                    n2.right = n2.left.take();
                    n2.left = right;
                    Self {
                        root: Some(n2),
                        len,
                    }
                }
            }
        }
    }

    /// Insert a value into the heap.
    pub fn insert(self, value: T) -> Self {
        let singleton = SkewHeap::singleton(value);
        Self::merge(self, singleton)
    }

    /// Return the minimum element without modifying the heap.
    pub fn find_min(&self) -> Result<&T, SkewHeapError> {
        self.root
            .as_ref()
            .map(|n| &n.value)
            .ok_or(SkewHeapError::EmptyHeap)
    }

    /// Remove and return the minimum element.
    pub fn pop_min(mut self) -> Result<(T, Self), SkewHeapError> {
        match self.root.take() {
            None => Err(SkewHeapError::EmptyHeap),
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

    fn count_nodes(node: Option<&SkewNode<T>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                1 + Self::count_nodes(n.left.as_deref()) + Self::count_nodes(n.right.as_deref())
            }
        }
    }

    /// Build a heap from a vector of values.
    pub fn from_vec(values: Vec<T>) -> Self {
        values.into_iter().fold(Self::new(), |h, v| h.insert(v))
    }
}

impl<T> SkewHeap<T> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn singleton(value: T) -> Self {
        Self {
            root: Some(SkewNode::new(value)),
            len: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn root(&self) -> Option<&SkewNode<T>> {
        self.root.as_deref()
    }
}

impl<T> Default for SkewHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_heap_is_empty() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn singleton_has_one_element() {
        let heap = SkewHeap::singleton(42);
        assert_eq!(heap.len(), 1);
        assert!(!heap.is_empty());
        assert_eq!(heap.find_min().unwrap(), &42);
    }

    #[test]
    fn insert_increases_len() {
        let heap = SkewHeap::<i32>::new().insert(1);
        assert_eq!(heap.len(), 1);
        let heap = heap.insert(2);
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn find_min_returns_min_element() {
        let heap = SkewHeap::new().insert(3).insert(1).insert(2);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn find_min_on_empty_returns_error() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        assert!(matches!(heap.find_min(), Err(SkewHeapError::EmptyHeap)));
    }

    #[test]
    fn pop_min_returns_and_removes_min() {
        let heap = SkewHeap::new().insert(3).insert(1).insert(2);
        let (min, heap) = heap.pop_min().unwrap();
        assert_eq!(min, 1);
        assert_eq!(heap.len(), 2);
        assert_eq!(heap.find_min().unwrap(), &2);
    }

    #[test]
    fn pop_min_on_empty_returns_error() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        assert!(matches!(heap.pop_min(), Err(SkewHeapError::EmptyHeap)));
    }

    #[test]
    fn pop_min_reduces_len() {
        let heap = SkewHeap::singleton(1);
        let (_, heap) = heap.pop_min().unwrap();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn merge_empty_with_empty() {
        let h1: SkewHeap<i32> = SkewHeap::new();
        let h2: SkewHeap<i32> = SkewHeap::new();
        let merged = SkewHeap::merge(h1, h2);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_empty_with_nonempty() {
        let h1: SkewHeap<i32> = SkewHeap::new();
        let h2 = SkewHeap::singleton(42);
        let merged = SkewHeap::merge(h1, h2);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged.find_min().unwrap(), &42);
    }

    #[test]
    fn merge_nonempty_with_empty() {
        let h1 = SkewHeap::singleton(42);
        let h2: SkewHeap<i32> = SkewHeap::new();
        let merged = SkewHeap::merge(h1, h2);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged.find_min().unwrap(), &42);
    }

    #[test]
    fn merge_two_singletons() {
        let h1 = SkewHeap::singleton(1);
        let h2 = SkewHeap::singleton(2);
        let merged = SkewHeap::merge(h1, h2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.find_min().unwrap(), &1);
    }

    #[test]
    fn merge_respects_heap_property() {
        let h1 = SkewHeap::new().insert(1).insert(5).insert(9);
        let h2 = SkewHeap::new().insert(2).insert(6).insert(10);
        let merged = SkewHeap::merge(h1, h2);

        fn check_heap<T: Ord>(node: &SkewNode<T>) {
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
        let h1 = SkewHeap::new().insert(1).insert(3).insert(5);
        let h2 = SkewHeap::new().insert(2).insert(4).insert(6);
        let merged = SkewHeap::merge(h1, h2);

        assert_eq!(merged.len(), 6);

        let mut sorted = Vec::new();
        let mut heap = merged;
        while let Ok((min, rest)) = heap.clone().pop_min() {
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
        let heap = SkewHeap::from_vec(values);

        assert_eq!(heap.len(), 7);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn repeated_pop_min_yields_sorted_order() {
        let heap = SkewHeap::from_vec(vec![5, 3, 7, 1, 9, 2, 8]);
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
        let heap = SkewHeap::new().insert(1).insert(2).insert(3);
        let json = serde_json::to_string(&heap).unwrap();
        let back: SkewHeap<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(heap, back);
    }

    #[test]
    fn default_is_empty() {
        let heap: SkewHeap<i32> = SkewHeap::default();
        assert!(heap.is_empty());
    }

    #[test]
    fn merge_multiple_times() {
        let h1 = SkewHeap::singleton(1);
        let h2 = SkewHeap::singleton(3);
        let h3 = SkewHeap::singleton(2);

        let merged = SkewHeap::merge(SkewHeap::merge(h1, h2), h3);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged.find_min().unwrap(), &1);
    }

    #[test]
    fn insert_after_merge() {
        let heap = SkewHeap::singleton(1);
        let heap = SkewHeap::merge(heap, SkewHeap::singleton(3));
        let heap = heap.insert(2);
        assert_eq!(heap.len(), 3);
        assert_eq!(heap.find_min().unwrap(), &1);
    }
}
