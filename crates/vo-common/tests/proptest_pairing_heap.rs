//! Proptest suite for PairingHeap — imports from library.

use proptest::prelude::*;
<<<<<<< HEAD
use vo_common::structures::PairingHeap;
=======

#[derive(Debug, Clone)]
struct PairingHeap<T: Ord + Clone> {
    root: Option<Box<Node<T>>>,
    len: usize,
}

#[derive(Debug, Clone)]
struct Node<T: Ord + Clone> {
    elem: T,
    children: Vec<Box<Node<T>>>,
}

impl<T: Ord + Clone> PairingHeap<T> {
    fn new() -> Self {
        Self { root: None, len: 0 }
    }
    fn push(&mut self, elem: T) {
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
    fn peek(&self) -> Option<T> {
        self.root.as_ref().map(|n| n.elem.clone())
    }
    fn pop(&mut self) -> Option<T> {
        let root = self.root.take()?;
        self.len = self.len.saturating_sub(1);
        self.root = merge_pairs(root.children);
        Some(root.elem)
    }
    fn merge(&mut self, other: &mut Self) {
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
    fn len(&self) -> usize {
        self.len
    }
}

fn link<T: Ord + Clone>(a: Box<Node<T>>, b: Box<Node<T>>) -> Box<Node<T>> {
    if a.elem <= b.elem {
        let mut ch = a.children;
        ch.push(b);
        Box::new(Node {
            elem: a.elem,
            children: ch,
        })
    } else {
        let mut ch = b.children;
        ch.push(a);
        Box::new(Node {
            elem: b.elem,
            children: ch,
        })
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
>>>>>>> 7e356012 (style: apply consistent rustfmt formatting)

proptest! {
    #[test]
    fn peek_returns_minimum(vals in proptest::collection::vec(proptest::num::i32::ANY, 0..50)) {
        let mut heap = PairingHeap::new();
        for &v in &vals { heap.push(v); }
        prop_assert_eq!(heap.peek(), vals.iter().min().copied());
    }

    #[test]
    fn pop_yields_sorted_order(vals in proptest::collection::vec(proptest::num::i32::ANY, 0..50)) {
        let mut heap = PairingHeap::new();
        for &v in &vals { heap.push(v); }
        let mut popped: Vec<i32> = Vec::new();
        while let Some(v) = heap.pop() { popped.push(v); }
        let mut expected = vals; expected.sort();
        prop_assert_eq!(popped, expected);
    }

    #[test]
    fn merge_preserves_all_elements(
        a in proptest::collection::vec(proptest::num::i32::ANY, 0..30),
        b in proptest::collection::vec(proptest::num::i32::ANY, 0..30)
    ) {
        let mut ha = PairingHeap::new(); for &v in &a { ha.push(v); }
        let mut hb = PairingHeap::new(); for &v in &b { hb.push(v); }
        ha.merge(&mut hb);
        let mut popped: Vec<i32> = Vec::new();
        while let Some(v) = ha.pop() { popped.push(v); }
        let mut expected = a; expected.extend(&b); expected.sort();
        prop_assert_eq!(popped, expected);
    }

    #[test]
    fn no_priority_inversion(a in proptest::num::i32::ANY, b in proptest::num::i32::ANY) {
        let mut h = PairingHeap::new();
        h.push(a); h.push(b);
        let first = h.pop().unwrap();
        let second = h.pop().unwrap();
        prop_assert!(first <= second, "priority inversion: {} before {}", first, second);
    }

    #[test]
    fn len_tracks_insertions(vals in proptest::collection::vec(proptest::num::i32::ANY, 0..50)) {
        let mut heap = PairingHeap::new();
        prop_assert_eq!(heap.len(), 0);
        for (i, &v) in vals.iter().enumerate() {
            heap.push(v);
            prop_assert_eq!(heap.len(), i + 1);
        }
        for expected_len in (0..vals.len()).rev() {
            heap.pop().unwrap();
            prop_assert_eq!(heap.len(), expected_len);
        }
    }
}

#[test]
fn empty_heap_pop_is_none() {
    let mut h: PairingHeap<i32> = PairingHeap::new();
    assert!(h.pop().is_none());
    assert!(h.peek().is_none());
}
