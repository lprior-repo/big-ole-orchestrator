//! Min-pairing-heap implementation for priority scheduling.
//!
//! A pairing heap is a simplified Fibonacci heap with better practical performance.
//! Maintains min-heap property: parent is always smaller than its children.

use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct PairingHeap<T: Ord + Clone> {
    root: Option<Box<Node<T>>>,
    len: usize,
}

#[derive(Debug, Clone)]
struct Node<T: Ord + Clone> {
    elem: T,
    children: Vec<Box<Node<T>>>,
}

impl<T: Ord + Clone> PairingHeap<T> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn push(&mut self, elem: T) {
        let node = Box::new(Node {
            elem,
            children: Vec::new(),
        });
        self.root = Some(match self.root.take() {
            Some(r) => link(node, r),
            None => node,
        });
        self.len += 1;
    }

    pub fn peek(&self) -> Option<T> {
        self.root.as_ref().map(|n| n.elem.clone())
    }

    pub fn pop(&mut self) -> Option<T> {
        let root = self.root.take()?;
        self.len = self.len.saturating_sub(1);
        self.root = merge_pairs(root.children);
        Some(root.elem)
    }

    pub fn merge(&mut self, other: &mut Self) {
        if other.root.is_none() {
            return;
        }
        self.root = Some(match (self.root.take(), other.root.take()) {
            (Some(a), Some(b)) => link(a, b),
            (a, b) => a.or(b).unwrap(),
        });
        self.len += other.len;
        other.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
}

impl<T: Ord + Clone> Default for PairingHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

fn link<T: Ord + Clone>(a: Box<Node<T>>, b: Box<Node<T>>) -> Box<Node<T>> {
    match a.elem.cmp(&b.elem) {
        Ordering::Less | Ordering::Equal => {
            let mut ch = a.children;
            ch.push(b);
            Box::new(Node {
                elem: a.elem,
                children: ch,
            })
        }
        Ordering::Greater => {
            let mut ch = b.children;
            ch.push(a);
            Box::new(Node {
                elem: b.elem,
                children: ch,
            })
        }
    }
}

fn merge_pairs<T: Ord + Clone>(nodes: Vec<Box<Node<T>>>) -> Option<Box<Node<T>>> {
    if nodes.is_empty() {
        return None;
    }
    if nodes.len() == 1 {
        return Some(nodes.into_iter().next().unwrap());
    }
    let mut it = nodes.into_iter();
    let mut merged = Vec::new();
    while let Some(f) = it.next() {
        merged.push(match it.next() {
            Some(s) => link(f, s),
            None => f,
        });
    }
    let mut acc = merged.pop().unwrap();
    while let Some(next) = merged.pop() {
        acc = link(acc, next);
    }
    Some(acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_heap_pop_is_none() {
        let mut h: PairingHeap<i32> = PairingHeap::new();
        assert!(h.pop().is_none());
        assert!(h.peek().is_none());
    }

    #[test]
    fn single_element() {
        let mut h: PairingHeap<i32> = PairingHeap::new();
        h.push(42);
        assert_eq!(h.pop(), Some(42));
        assert!(h.is_empty());
    }

    #[test]
    fn maintains_min_heap() {
        let mut h = PairingHeap::new();
        h.push(5);
        h.push(3);
        h.push(7);
        h.push(1);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(3));
        assert_eq!(h.pop(), Some(5));
        assert_eq!(h.pop(), Some(7));
    }

    #[test]
    fn peek_returns_min_without_removing() {
        let mut h = PairingHeap::new();
        h.push(5);
        h.push(3);
        h.push(7);
        h.push(1);
        assert_eq!(h.peek(), Some(1));
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn merge_combines_two_heaps() {
        let mut h1 = PairingHeap::new();
        h1.push(1);
        h1.push(5);

        let mut h2 = PairingHeap::new();
        h2.push(3);
        h2.push(7);

        h1.merge(&mut h2);

        assert_eq!(h1.pop(), Some(1));
        assert_eq!(h1.pop(), Some(3));
        assert_eq!(h1.pop(), Some(5));
        assert_eq!(h1.pop(), Some(7));
        assert!(h1.is_empty());
    }

    #[test]
    fn merge_empty_heap_is_noop() {
        let mut h1 = PairingHeap::new();
        h1.push(1);

        let mut h2 = PairingHeap::new();

        h1.merge(&mut h2);

        assert_eq!(h1.pop(), Some(1));
        assert!(h1.is_empty());
    }
}
