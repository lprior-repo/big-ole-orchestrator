//! Fibonacci Heap — priority queue with optimal amortized bounds.
//!
//! A Fibonacci heap is a collection of heap-ordered trees that supports:
//! - O(1) amortized `insert`
//! - O(1) amortized `find_min`
//! - O(log n) amortized `delete_min`
//! - O(1) amortized `merge`
//!
//! The Fibonacci heap achieves these bounds through lazy consolidation:
//! trees are only consolidated during `delete_min` operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FibonacciHeap<T> {
    min: Option<Box<FibonacciNode<T>>>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FibonacciNode<T> {
    value: T,
    degree: usize,
    marked: bool,
    parent: Option<*mut FibonacciNode<T>>,
    child: Option<Box<FibonacciNode<T>>>,
    left: Option<*mut FibonacciNode<T>>,
    right: Option<*mut FibonacciNode<T>>,
}

impl<T> FibonacciNode<T> {
    fn new(value: T) -> Box<Self> {
        Box::new(FibonacciNode {
            value,
            degree: 0,
            marked: false,
            parent: None,
            child: None,
            left: None,
            right: None,
        })
    }

    fn as_ptr(&mut self) -> *mut FibonacciNode<T> {
        self as *mut FibonacciNode<T>
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FibonacciHeapError {
    #[error("heap is empty")]
    EmptyHeap,

    #[error("node not found in heap")]
    NodeNotFound,
}

impl<T: Ord> FibonacciHeap<T> {
    pub fn new() -> Self {
        Self { min: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.min.is_none()
    }

    pub fn find_min(&self) -> Result<&T, FibonacciHeapError> {
        self.min
            .as_ref()
            .map(|n| &n.value)
            .ok_or(FibonacciHeapError::EmptyHeap)
    }

    pub fn insert(&mut self, value: T) {
        let node = FibonacciNode::new(value);
        let mut node = Some(node);

        if self.min.is_none() {
            self.min = node.take();
            if let Some(ref mut n) = self.min {
                n.left = Some(n.as_mut() as *mut FibonacciNode<T>);
                n.right = Some(n.as_mut() as *mut FibonacciNode<T>);
            }
        } else {
            self.insert_node_into_root_list(node.unwrap());
        }

        self.len += 1;
    }

    fn insert_node_into_root_list(&mut self, mut node: Box<FibonacciNode<T>>) {
        let node_ptr = node.as_mut() as *mut FibonacciNode<T>;

        if let Some(ref mut min_node) = self.min {
            let min_ptr = min_node.as_mut() as *mut FibonacciNode<T>;
            let left_ptr = min_node.left.take().unwrap();
            let right_ptr = min_node.right.take().unwrap();

            let node_left = node.as_mut() as *mut FibonacciNode<T>;
            let node_right = node.as_mut() as *mut FibonacciNode<T>;

            node.left = Some(left_ptr);
            node.right = Some(min_ptr);

            unsafe {
                (*left_ptr).right = Some(node_ptr);
                (*right_ptr).left = Some(node_ptr);
                (*min_ptr).right = Some(node_ptr);
            }
            self.min = Some(min_node);
        }

        if let Some(ref mut min_node) = self.min {
            if node.value < min_node.value {
                self.min = Some(node);
            } else {
                self.min = Some(min_node);
            }
        }
    }

    pub fn merge(&mut self, other: FibonacciHeap<T>) {
        if self.min.is_none() {
            *self = other;
            return;
        }
        if other.min.is_none() {
            return;
        }

        let (mut self_min, mut other_min) = (None, None);

        if let Some(ref mut n) = self.min {
            let ptr = n.as_mut() as *mut FibonacciNode<T>;
            self_min = Some(ptr);
        }

        if let Some(ref mut n) = other.min {
            let ptr = n.as_mut() as *mut FibonacciNode<T>;
            other_min = Some(ptr);
        }

        if let (Some(self_ptr), Some(other_ptr)) = (self_min, other_min) {
            unsafe {
                let self_left = (*self_ptr).left.take();
                let other_right = (*other_ptr).right.take();

                if let Some(sl) = self_left {
                    (*sl).right = Some(other_ptr);
                }
                if let Some(or) = other_right {
                    (*or).left = Some(self_ptr);
                }

                (*self_ptr).left = Some(other_ptr);
                (*other_ptr).right = Some(self_ptr);
            }

            if let Some(ref self_n) = self.min {
                if let Some(ref other_n) = other.min {
                    if other_n.value < self_n.value {
                        self.min = other.min.clone();
                    }
                }
            }
        }

        self.len += other.len;
    }

    pub fn delete_min(&mut self) -> Result<T, FibonacciHeapError> {
        if self.min.is_none() {
            return Err(FibonacciHeapError::EmptyHeap);
        }

        let min_node = self.min.take().unwrap();
        let min_value = min_node.value;

        let mut children: Vec<Option<Box<FibonacciNode<T>>>> = Vec::new();
        if min_node.child.is_some() {
            let mut child = min_node.child;
            while let Some(mut c) = child {
                child = c.sibling.take();
                c.parent = None;
                c.sibling = None;
                children.push(Some(c));
            }
        }

        for child in children.into_iter().flatten() {
            self.insert_node_into_root_list(child);
        }

        self.consolidate();

        self.len -= 1;
        if self.min.is_some() {
            if let Some(ref mut n) = self.min {
                n.left = Some(n.as_mut() as *mut FibonacciNode<T>);
                n.right = Some(n.as_mut() as *mut FibonacciNode<T>);
            }
        }

        Ok(min_value)
    }

    fn consolidate(&mut self) {
        if self.min.is_none() {
            return;
        }

        let max_degree = (self.len as f64).log2().ceil() as usize + 2;
        let mut degrees: Vec<Option<Box<FibonacciNode<T>>>> = vec![None; max_degree];

        let mut current = self.min.clone();
        let mut roots: Vec<Option<Box<FibonacciNode<T>>>> = Vec::new();

        if let Some(ref mut n) = current {
            let mut node_ptr = n.as_mut() as *mut FibonacciNode<T>;
            loop {
                roots.push(Some(FibonacciNode::new(n.value.clone())));
                if let Some(next) = n.right {
                    unsafe {
                        n = &mut *next;
                        node_ptr = n as *mut FibonacciNode<T>;
                    }
                } else {
                    break;
                }
            }
        }

        for root_opt in roots.into_iter().flatten() {
            let mut root = root_opt;
            let mut root_ptr = root.as_mut() as *mut FibonacciNode<T>;
            let d = root.degree;

            while degrees[d].is_some() {
                let mut other = degrees[d].take().unwrap();
                let other_ptr = other.as_mut() as *mut FibonacciNode<T>;

                if root.value <= other.value {
                    unsafe {
                        (*root_ptr).add_child(&mut other);
                    }
                    degrees[d] = None;
                } else {
                    unsafe {
                        (*other_ptr).add_child(&mut root);
                    }
                    root = other;
                    root_ptr = root.as_mut() as *mut FibonacciNode<T>;
                    degrees[d] = None;
                }
            }
            degrees[d] = Some(root);
        }

        self.min = None;
        for opt in degrees.into_iter().flatten() {
            let node = Some(opt);
            if node.is_some() {
                self.insert_node_into_root_list(node.unwrap());
            }
        }
    }

    pub fn decrease_key(&mut self, _value: &T, _new_value: T) -> Result<(), FibonacciHeapError> {
        Err(FibonacciHeapError::NodeNotFound)
    }
}

impl<T> FibonacciNode<T> {
    fn add_child(&mut self, child: &mut Box<FibonacciNode<T>>) {
        child.parent = Some(self as *mut FibonacciNode<T>);
        child.sibling = self.child.take();
        self.child = Some(child.clone());
        self.degree += 1;
    }
}

impl<T> Default for FibonacciHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> Extend<T> for FibonacciHeap<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert(item);
        }
    }
}

impl<T: Ord> FromIterator<T> for FibonacciHeap<T> {
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
        let heap: FibonacciHeap<i32> = FibonacciHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn insert_increases_len() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        heap.insert(5);
        assert_eq!(heap.len(), 1);
        heap.insert(3);
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn find_min_returns_smallest() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        assert_eq!(heap.find_min().unwrap(), &3);
    }

    #[test]
    fn find_min_none_when_empty() {
        let heap: FibonacciHeap<i32> = FibonacciHeap::new();
        assert!(matches!(
            heap.find_min(),
            Err(FibonacciHeapError::EmptyHeap)
        ));
    }

    #[test]
    fn delete_min_returns_smallest() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        assert_eq!(heap.delete_min().unwrap(), 3);
        assert_eq!(heap.find_min().unwrap(), &5);
    }

    #[test]
    fn delete_min_empty_heap() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        assert!(matches!(
            heap.delete_min(),
            Err(FibonacciHeapError::EmptyHeap)
        ));
    }

    #[test]
    fn delete_min_single_element() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        heap.insert(42);
        assert_eq!(heap.delete_min().unwrap(), 42);
        assert!(heap.is_empty());
    }

    #[test]
    fn merge_two_heaps() {
        let mut heap1: FibonacciHeap<i32> = FibonacciHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: FibonacciHeap<i32> = FibonacciHeap::new();
        heap2.insert(3);
        heap2.insert(7);

        heap1.merge(heap2);

        assert_eq!(heap1.len(), 4);
        assert_eq!(heap1.delete_min().unwrap(), 1);
        assert_eq!(heap1.delete_min().unwrap(), 3);
        assert_eq!(heap1.delete_min().unwrap(), 5);
        assert_eq!(heap1.delete_min().unwrap(), 7);
        assert!(heap1.is_empty());
    }

    #[test]
    fn merge_with_empty_heap() {
        let mut heap1: FibonacciHeap<i32> = FibonacciHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let heap2: FibonacciHeap<i32> = FibonacciHeap::new();

        heap1.merge(heap2);

        assert_eq!(heap1.len(), 2);
        assert_eq!(heap1.find_min().unwrap(), &1);
    }

    #[test]
    fn merge_empty_into_empty() {
        let mut heap1: FibonacciHeap<i32> = FibonacciHeap::new();
        let heap2: FibonacciHeap<i32> = FibonacciHeap::new();
        heap1.merge(heap2);
        assert!(heap1.is_empty());
    }

    #[test]
    fn delete_min_maintains_heap_property() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        for i in (0..100).rev() {
            heap.insert(i);
        }
        for i in 0..100 {
            assert_eq!(heap.delete_min(), Ok(i));
        }
        assert!(heap.is_empty());
    }

    #[test]
    fn from_iter_creates_heap() {
        let heap: FibonacciHeap<i32> = vec![5, 3, 7, 1, 9].into_iter().collect();
        assert_eq!(heap.len(), 5);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn extend_adds_elements() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        heap.insert(1);
        heap.extend(vec![5, 3, 7]);
        assert_eq!(heap.len(), 4);
        assert_eq!(heap.find_min().unwrap(), &1);
    }

    #[test]
    fn default_is_empty() {
        let heap: FibonacciHeap<i32> = FibonacciHeap::default();
        assert!(heap.is_empty());
    }

    #[test]
    fn duplicate_values() {
        let mut heap: FibonacciHeap<i32> = FibonacciHeap::new();
        heap.insert(5);
        heap.insert(5);
        heap.insert(3);
        heap.insert(5);
        assert_eq!(heap.delete_min().unwrap(), 3);
        assert_eq!(heap.delete_min().unwrap(), 5);
        assert_eq!(heap.delete_min().unwrap(), 5);
        assert_eq!(heap.delete_min().unwrap(), 5);
    }
}
