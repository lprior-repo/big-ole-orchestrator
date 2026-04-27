//! Proptest suite for PairingHeap — imports from library.

use proptest::prelude::*;
use vo_common::structures::PairingHeap;

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
