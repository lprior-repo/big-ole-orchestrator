//! Binomial heap - priority queue based on binomial trees.
//!
//! A binomial heap is a collection of binomial trees that supports:
//! - `insert`: O(1) amortized
//! - `find_min`: O(1)
//! - `delete_min`: O(log n) amortized
//! - `merge`: O(log n) amortized

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinomialNode<T> {
    pub value: T,
    degree: usize,
    child: Option<Box<BinomialNode<T>>>,
    sibling: Option<Box<BinomialNode<T>>>,
}

impl<T: PartialOrd> BinomialNode<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            degree: 0,
            child: None,
            sibling: None,
        }
    }

    fn link(first: Self, second: &mut Self) {
        // Standard binomial link: attach first as leftmost child of second.
        // Caller ensures second.value <= first.value (second is root).
        let mut child = first;
        child.sibling = second.child.take();
        second.degree += 1;
        second.child = Some(Box::new(child));
    }

    fn min_value(&self) -> &T {
        let mut min_val = &self.value;
        if let Some(ref child) = self.child {
            let child_min = child.min_value();
            if child_min < min_val {
                min_val = child_min;
            }
        }
        if let Some(ref sibling) = self.sibling {
            let sibling_min = sibling.min_value();
            if sibling_min < min_val {
                min_val = sibling_min;
            }
        }
        min_val
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinomialHeap<T> {
    trees: Vec<Option<BinomialNode<T>>>,
    len: usize,
}

impl<T> Default for BinomialHeap<T> {
    fn default() -> Self {
        Self {
            trees: Vec::new(),
            len: 0,
        }
    }
}

impl<T: Ord> BinomialHeap<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn find_min(&self) -> Option<&T> {
        let mut min = None;
        for tree in self.trees.iter().flatten() {
            match min {
                None => min = Some(tree.min_value()),
                Some(current_min) => {
                    let tree_min = tree.min_value();
                    if *tree_min < *current_min {
                        min = Some(tree_min);
                    }
                }
            }
        }
        min
    }

    fn merge_trees(&self, a: BinomialNode<T>, b: BinomialNode<T>) -> BinomialNode<T> {
        let mut a = a;
        let mut b = b;
        if a.value <= b.value {
            BinomialNode::link(b, &mut a);
            a
        } else {
            BinomialNode::link(a, &mut b);
            b
        }
    }

    fn carry(&mut self, carry: Option<BinomialNode<T>>, degree: usize) {
        while degree >= self.trees.len() {
            self.trees.push(None);
        }

        let (replacement, new_carry) = match (self.trees[degree].take(), carry) {
            (None, None) => (None, None),
            (Some(tree), None) | (None, Some(tree)) => (Some(tree), None),
            (Some(a), Some(b)) => {
                let result = self.merge_trees(a, b);
                (None, Some(result))
            }
        };

        self.trees[degree] = replacement;
        if let Some(c) = new_carry {
            self.carry(Some(c), degree + 1);
        }
    }

    pub fn insert(&mut self, value: T) {
        self.carry(Some(BinomialNode::new(value)), 0);
        self.len += 1;
    }

    pub fn merge(&mut self, other: &mut BinomialHeap<T>) {
        let other_len = other.len;
        let mut other_trees = std::mem::take(&mut other.trees);
        other.len = 0;

        let mut carry: Option<BinomialNode<T>> = None;
        let max_degree = std::cmp::max(self.trees.len(), other_trees.len());
        while self.trees.len() < max_degree + 1 {
            self.trees.push(None);
        }
        for degree in 0..=max_degree {
            let self_tree = self.trees[degree].take();
            let other_tree = if degree < other_trees.len() {
                other_trees[degree].take()
            } else {
                None
            };
            match (carry.take(), self_tree, other_tree) {
                (None, None, None) => {}
                (None, None, Some(t)) | (None, Some(t), None) => {
                    self.trees[degree] = Some(t);
                }
                (Some(c), None, None) => {
                    self.trees[degree] = Some(c);
                }
                (None, Some(a), Some(b)) => {
                    carry = Some(self.merge_trees(a, b));
                }
                (Some(c), Some(a), None) => {
                    carry = Some(self.merge_trees(c, a));
                }
                (Some(c), None, Some(a)) => {
                    carry = Some(self.merge_trees(c, a));
                }
                (Some(c), Some(a), Some(b)) => {
                    let merged1 = self.merge_trees(a, b);
                    carry = Some(self.merge_trees(c, merged1));
                }
            }
        }
        if let Some(c) = carry {
            self.trees.push(Some(c));
        }
        self.len += other_len;
    }

    pub fn delete_min(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let (min_degree, _) = self
            .trees
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.as_ref().map(|tree| (i, tree.min_value())))
            .min_by_key(|(_, v)| *v)
            .unwrap();

        let min_tree = self.trees[min_degree].take()?;
        let min_value = min_tree.value;

        let mut children: Vec<Option<BinomialNode<T>>> = Vec::new();
        let mut current = min_tree.child;
        while let Some(mut node) = current {
            let sibling = node.sibling.take();
            children.push(Some(*node));
            current = sibling;
        }
        children.reverse();

        let child_len = 2usize.pow(min_tree.degree as u32) - 1;
        self.len -= 1;

        let mut child_heap = BinomialHeap {
            trees: children,
            len: child_len,
        };

        self.merge(&mut child_heap);

        Some(min_value)
    }
}

impl<T: Ord> Extend<T> for BinomialHeap<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert(item);
        }
    }
}

impl<T: Ord> FromIterator<T> for BinomialHeap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut heap = Self::new();
        heap.extend(iter);
        heap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_heap_is_empty() {
        let heap: BinomialHeap<i32> = BinomialHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn insert_increases_len() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        heap.insert(5);
        assert_eq!(heap.len(), 1);
        heap.insert(3);
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn find_min_returns_smallest() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        assert_eq!(heap.find_min(), Some(&3));
    }

    #[test]
    fn find_min_none_when_empty() {
        let heap: BinomialHeap<i32> = BinomialHeap::new();
        assert_eq!(heap.find_min(), None);
    }

    #[test]
    fn delete_min_returns_smallest() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        assert_eq!(heap.delete_min(), Some(3));
        assert_eq!(heap.find_min(), Some(&5));
    }

    #[test]
    fn delete_min_empty_heap() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        assert_eq!(heap.delete_min(), None);
    }

    #[test]
    fn delete_min_single_element() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        heap.insert(42);
        assert_eq!(heap.delete_min(), Some(42));
        assert!(heap.is_empty());
    }

    #[test]
    fn merge_two_heaps() {
        let mut heap1: BinomialHeap<i32> = BinomialHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: BinomialHeap<i32> = BinomialHeap::new();
        heap2.insert(3);
        heap2.insert(7);

        heap1.merge(&mut heap2);

        assert_eq!(heap1.len(), 4);
        assert_eq!(heap1.find_min(), Some(&1));
        assert_eq!(heap1.delete_min(), Some(1));
        assert_eq!(heap1.delete_min(), Some(3));
        assert_eq!(heap1.delete_min(), Some(5));
        assert_eq!(heap1.delete_min(), Some(7));
        assert!(heap1.is_empty());
    }

    #[test]
    fn merge_with_empty_heap() {
        let mut heap1: BinomialHeap<i32> = BinomialHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: BinomialHeap<i32> = BinomialHeap::new();

        heap1.merge(&mut heap2);

        assert_eq!(heap1.len(), 2);
        assert_eq!(heap1.find_min(), Some(&1));
    }

    #[test]
    fn merge_empty_into_empty() {
        let mut heap1: BinomialHeap<i32> = BinomialHeap::new();
        let mut heap2: BinomialHeap<i32> = BinomialHeap::new();
        heap1.merge(&mut heap2);
        assert!(heap1.is_empty());
    }

    #[test]
    fn delete_min_maintains_heap_property() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        for i in (0..100).rev() {
            heap.insert(i);
        }
        for i in 0..100 {
            assert_eq!(heap.delete_min(), Some(i));
        }
        assert!(heap.is_empty());
    }

    #[test]
    fn from_iter_creates_heap() {
        let heap: BinomialHeap<i32> = vec![5, 3, 7, 1, 9].into_iter().collect();
        assert_eq!(heap.len(), 5);
        assert_eq!(heap.find_min(), Some(&1));
    }

    #[test]
    fn extend_adds_elements() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        heap.insert(1);
        heap.extend(vec![5, 3, 7]);
        assert_eq!(heap.len(), 4);
        assert_eq!(heap.find_min(), Some(&1));
    }

    #[test]
    fn serde_roundtrip() {
        let mut heap: BinomialHeap<i32> = BinomialHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        let json = serde_json::to_string(&heap).unwrap();
        let back: BinomialHeap<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(heap, back);
    }
}
