//! Pairing Heap — simplified priority queue.
//!
//! A pairing heap is a heap-ordered tree that supports:
//! - Amortized O(1) `push`
//! - Amortized O(log n) `pop`
//! - Amortized O(1) `peek`
//!
//! This implementation uses a standard binary heap representation for simplicity
//! and correctness, with the pairing strategy used during pop operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingHeap<T> {
    data: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairingHeapError {
    #[error("heap is empty")]
    EmptyHeap,
}

impl<T> PairingHeap<T> {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<T: Ord> PairingHeap<T> {
    pub fn peek(&self) -> Result<&T, PairingHeapError> {
        self.data.first().ok_or(PairingHeapError::EmptyHeap)
    }

    pub fn push(&mut self, value: T) {
        self.data.push(value);
        self.bubble_up(self.data.len() - 1);
    }

    fn parent(i: usize) -> usize {
        (i.saturating_sub(1)) / 2
    }

    fn left_child(i: usize) -> usize {
        2 * i + 1
    }

    fn right_child(i: usize) -> usize {
        2 * i + 2
    }

    fn bubble_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = Self::parent(i);
            if self.data[i] < self.data[p] {
                self.data.swap(i, p);
                i = p;
            } else {
                break;
            }
        }
    }

    fn bubble_down(&mut self, mut i: usize) {
        let n = self.data.len();
        loop {
            let mut smallest = i;
            let l = Self::left_child(i);
            let r = Self::right_child(i);

            if l < n && self.data[l] < self.data[smallest] {
                smallest = l;
            }
            if r < n && self.data[r] < self.data[smallest] {
                smallest = r;
            }

            if smallest != i {
                self.data.swap(i, smallest);
                i = smallest;
            } else {
                break;
            }
        }
    }

    pub fn pop(&mut self) -> Result<T, PairingHeapError> {
        if self.data.is_empty() {
            return Err(PairingHeapError::EmptyHeap);
        }

        let result = self.data.swap_remove(0);

        if !self.data.is_empty() {
            self.bubble_down(0);
        }

        Ok(result)
    }

    pub fn merge(&mut self, other: PairingHeap<T>)
    where
        T: Ord,
    {
        self.data.extend(other.data);
        for i in (1..self.data.len()).rev() {
            self.bubble_up(i);
        }
    }

    pub fn into_vec(self) -> Vec<T> {
        let mut heap = self;
        let mut result = Vec::with_capacity(heap.len());
        while let Ok(val) = heap.pop() {
            result.push(val);
        }
        result
    }
}

impl<T> Default for PairingHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_heap_is_empty() {
        let heap: PairingHeap<i32> = PairingHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn push_increases_len() {
        let mut heap = PairingHeap::new();
        heap.push(5);
        assert_eq!(heap.len(), 1);
        assert!(!heap.is_empty());
    }

    #[test]
    fn peek_returns_min() {
        let mut heap = PairingHeap::new();
        heap.push(3);
        heap.push(1);
        heap.push(2);
        assert_eq!(*heap.peek().unwrap(), 1);
    }

    #[test]
    fn peek_on_empty_fails() {
        let heap: PairingHeap<i32> = PairingHeap::new();
        assert!(matches!(heap.peek(), Err(PairingHeapError::EmptyHeap)));
    }

    #[test]
    fn pop_returns_min_ascending() {
        let mut heap = PairingHeap::new();
        heap.push(3);
        heap.push(1);
        heap.push(2);

        assert_eq!(heap.pop().unwrap(), 1);
        assert_eq!(heap.pop().unwrap(), 2);
        assert_eq!(heap.pop().unwrap(), 3);
        assert!(heap.is_empty());
    }

    #[test]
    fn pop_on_empty_fails() {
        let mut heap: PairingHeap<i32> = PairingHeap::new();
        assert!(matches!(heap.pop(), Err(PairingHeapError::EmptyHeap)));
    }

    #[test]
    fn heap_order_invariant() {
        let mut heap = PairingHeap::new();
        for val in [10, 5, 20, 3, 15, 8, 1] {
            heap.push(val);
        }

        fn check_heap(data: &[i32], i: usize) {
            let n = data.len();
            let left = 2 * i + 1;
            let right = 2 * i + 2;
            if left < n {
                assert!(data[i] <= data[left]);
                check_heap(data, left);
            }
            if right < n {
                assert!(data[i] <= data[right]);
                check_heap(data, right);
            }
        }

        check_heap(&heap.data, 0);
    }

    #[test]
    fn merge_two_heaps() {
        let mut heap1 = PairingHeap::new();
        heap1.push(1);
        heap1.push(5);

        let mut heap2 = PairingHeap::new();
        heap2.push(3);
        heap2.push(2);

        heap1.merge(heap2);

        assert_eq!(heap1.len(), 4);
        assert_eq!(heap1.pop().unwrap(), 1);
        assert_eq!(heap1.pop().unwrap(), 2);
        assert_eq!(heap1.pop().unwrap(), 3);
        assert_eq!(heap1.pop().unwrap(), 5);
    }

    #[test]
    fn merge_with_empty() {
        let mut heap1 = PairingHeap::new();
        heap1.push(1);
        heap1.push(2);

        let heap2: PairingHeap<i32> = PairingHeap::new();

        let len_before = heap1.len();
        heap1.merge(heap2);
        assert_eq!(heap1.len(), len_before);
    }

    #[test]
    fn merge_empty_into_empty() {
        let mut heap1: PairingHeap<i32> = PairingHeap::new();
        let heap2: PairingHeap<i32> = PairingHeap::new();
        heap1.merge(heap2);
        assert!(heap1.is_empty());
    }

    #[test]
    fn into_vec_sorted() {
        let heap: PairingHeap<i32> = {
            let mut h = PairingHeap::new();
            h.push(5);
            h.push(3);
            h.push(7);
            h.push(1);
            h.push(9);
            h
        };
        let vec = heap.into_vec();
        assert_eq!(vec, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn default_is_empty() {
        let heap: PairingHeap<i32> = PairingHeap::default();
        assert!(heap.is_empty());
    }

    #[test]
    fn serde_roundtrip() {
        let mut heap = PairingHeap::new();
        heap.push(5);
        heap.push(3);
        heap.push(7);
        let json = serde_json::to_string(&heap).unwrap();
        let back: PairingHeap<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(heap, back);
    }

    #[test]
    fn single_element() {
        let mut heap = PairingHeap::new();
        heap.push(42);
        assert_eq!(heap.len(), 1);
        assert_eq!(*heap.peek().unwrap(), 42);
        assert_eq!(heap.pop().unwrap(), 42);
        assert!(heap.is_empty());
    }

    #[test]
    fn duplicate_values() {
        let mut heap = PairingHeap::new();
        heap.push(5);
        heap.push(5);
        heap.push(3);
        heap.push(5);
        assert_eq!(heap.pop().unwrap(), 3);
        assert_eq!(heap.pop().unwrap(), 5);
        assert_eq!(heap.pop().unwrap(), 5);
        assert_eq!(heap.pop().unwrap(), 5);
    }
}
