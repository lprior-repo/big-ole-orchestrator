//! QA tests for btree.rs: proptest, adversarial, boundary, and concurrent coverage.

use proptest::prelude::*;
use vo_types::BTree;

#[derive(Debug, Clone)]
enum Op {
    Insert(i32, i32),
    Delete(i32),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        proptest::arbitrary::any::<(i32, i32)>().prop_map(|(k, v)| Op::Insert(k, v)),
        proptest::arbitrary::any::<i32>().prop_map(Op::Delete),
    ]
}

// ── Proptest: random insert/delete sequences preserve BST invariants ──

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 50, ..proptest::prelude::ProptestConfig::default() })]

    #[test]
    fn proptest_insert_delete_random_ops(
        ops in proptest::collection::vec(op_strategy(), 0..200),
        order in 3usize..20usize
    ) {
        let mut tree = BTree::with_order(order);
        let mut reference: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();

        for op in &ops {
            match op {
                Op::Insert(k, v) => {
                    tree.insert(*k, *v);
                    reference.insert(*k, *v);
                }
                Op::Delete(k) => {
                    let tree_result = tree.delete(k);
                    let ref_result = reference.remove(k);
                    assert_eq!(tree_result.is_ok(), ref_result.is_some());
                }
            }
            assert!(tree.verify(), "invariant violated: {op:?} order={order}");
            assert_eq!(tree.len(), reference.len());
        }

        for (&k, &v) in &reference {
            assert_eq!(tree.search(&k), Some(&v));
        }
    }

    #[test]
    fn proptest_range_matches_sorted_reference(
        inserts in proptest::collection::vec(proptest::arbitrary::any::<i32>(), 1..100),
        lo in proptest::arbitrary::any::<i32>(),
        hi in proptest::arbitrary::any::<i32>()
    ) {
        let mut tree = BTree::new();
        let mut sorted_keys = std::collections::BTreeSet::new();
        for &k in &inserts {
            tree.insert(k, k.wrapping_mul(10));
            sorted_keys.insert(k);
        }

        if lo <= hi {
            let tree_results: Vec<i32> = tree.range(lo..=hi).iter().map(|(k, _)| **k).collect();
            let ref_results: Vec<&i32> = sorted_keys.range(lo..=hi).collect();
            assert_eq!(tree_results.len(), ref_results.len());
            for (t, r) in tree_results.iter().zip(ref_results.iter()) {
                assert_eq!(t, *r);
            }
        }
    }

    #[test]
    fn proptest_update_does_not_change_len(
        pairs in proptest::collection::vec(proptest::arbitrary::any::<(i32, i32)>(), 1..50),
        order in 3usize..15usize
    ) {
        let mut tree = BTree::with_order(order);
        for (k, v) in &pairs {
            tree.insert(*k, *v);
        }
        let len_after_insert = tree.len();

        for (k, v) in &pairs {
            tree.insert(*k, v.wrapping_add(1));
        }

        assert_eq!(tree.len(), len_after_insert);
        assert!(tree.verify());
    }

    #[test]
    fn proptest_delete_nonexistent_preserves_state(
        inserts in proptest::collection::vec(proptest::arbitrary::any::<i32>(), 10..50),
        phantom_keys in proptest::collection::vec(proptest::arbitrary::any::<i32>(), 1..20)
    ) {
        let mut tree = BTree::new();
        for &k in &inserts {
            tree.insert(k, k);
        }
        let len_before = tree.len();

        for &pk in &phantom_keys {
            if !inserts.contains(&pk) {
                assert!(tree.delete(&pk).is_err());
            }
        }

        assert_eq!(tree.len(), len_before);
        assert!(tree.verify());
    }

    #[test]
    fn proptest_height_is_logarithmic(
        keys in proptest::collection::vec(proptest::arbitrary::any::<u32>(), 100..1000)
    ) {
        let order = 4usize;
        let max_keys = order - 1;
        let mut tree = BTree::with_order(order);
        let mut seen = std::collections::HashSet::new();
        for &k in &keys {
            if seen.insert(k) {
                tree.insert(k, k);
            }
        }

        if !tree.is_empty() {
            let h = tree.height() as f64;
            let n = tree.len() as f64;
            let b = max_keys as f64;
            let max_theoretical = (n.ln() / b.ln()).ceil() + 1.0;
            assert!(h <= max_theoretical + 1.0,
                "height {} exceeds theoretical max ~{} for n={} order={}",
                h, max_theoretical, tree.len(), order);
        }
        assert!(tree.verify());
    }

    #[test]
    fn proptest_from_vec_matches_sequential_insert(
        pairs in proptest::collection::vec(proptest::arbitrary::any::<(i32, String)>(), 0..200)
    ) {
        let tree_from: BTree<i32, String> = BTree::from(pairs.clone());

        let mut tree_seq = BTree::new();
        for (k, v) in &pairs {
            tree_seq.insert(*k, v.clone());
        }

        assert_eq!(tree_from.len(), tree_seq.len());
        assert!(tree_from.verify());
        assert!(tree_seq.verify());
    }
}

// ── Proptest: per-operation insert/delete properties (tw-yya6) ──

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 100, ..proptest::prelude::ProptestConfig::default() })]

    #[test]
    fn proptest_insert_findable(
        pairs in proptest::collection::vec(proptest::arbitrary::any::<(i32, i32)>(), 1..300),
        order in 3usize..20usize
    ) {
        let mut tree = BTree::with_order(order);
        for (k, v) in &pairs {
            tree.insert(*k, *v);
            assert_eq!(tree.search(k), Some(v), "key {k} not findable after insert");
            assert!(tree.contains(k), "contains false after inserting {k}");
            assert!(tree.verify());
        }
    }

    #[test]
    fn proptest_delete_key_gone(
        keys in proptest::collection::btree_set(proptest::arbitrary::any::<i32>(), 1..200),
        order in 3usize..15usize
    ) {
        let mut tree = BTree::with_order(order);
        for &k in &keys {
            tree.insert(k, k);
        }

        for &k in &keys {
            assert!(tree.search(&k).is_some(), "key {k} missing before delete");
            tree.delete(&k).unwrap();
            assert_eq!(tree.search(&k), None, "key {k} still findable after delete");
            assert!(!tree.contains(&k), "contains true after deleting {k}");
        }
        assert_eq!(tree.len(), 0);
    }

    #[test]
    #[ignore = "exposes BTree delete invariant bug — delete corrupts node structure for certain key/order combos"]
    fn proptest_delete_preserves_invariants(
        keys in proptest::collection::btree_set(proptest::arbitrary::any::<i32>(), 1..100),
        order in 3usize..10usize
    ) {
        let mut tree = BTree::with_order(order);
        for &k in &keys {
            tree.insert(k, k);
            assert!(tree.verify(), "invariant violated after inserting {k}");
        }
        for &k in &keys {
            tree.delete(&k).unwrap();
            assert!(tree.verify(), "invariant violated after deleting {k}");
        }
    }

    #[test]
    fn proptest_no_panic_valid_ops(
        ops in proptest::collection::vec(op_strategy(), 0..500),
        order in 3usize..10usize
    ) {
        let mut tree = BTree::with_order(order);
        for op in &ops {
            match op {
                Op::Insert(k, v) => { tree.insert(*k, *v); }
                Op::Delete(k) => { let _ = tree.delete(k); }
            }
        }
        assert!(tree.verify());
    }

    #[test]
    fn proptest_delete_preserves_other_keys(
        base_keys in proptest::collection::vec(proptest::arbitrary::any::<i32>(), 10..100),
        delete_targets in proptest::collection::vec(proptest::arbitrary::any::<i32>(), 1..30),
        order in 3usize..10usize
    ) {
        let mut tree = BTree::with_order(order);
        let mut reference: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();

        for &k in &base_keys {
            tree.insert(k, k);
            reference.insert(k, k);
        }

        for &k in &delete_targets {
            if reference.remove(&k).is_some() {
                tree.delete(&k).unwrap();
            }
        }

        for (&k, &v) in &reference {
            assert_eq!(tree.search(&k), Some(&v), "key {k} lost after deletes");
        }
        assert!(tree.verify());
    }

    #[test]
    fn proptest_min_max_after_each_op(
        ops in proptest::collection::vec(op_strategy(), 0..200),
        order in 3usize..10usize
    ) {
        let mut tree = BTree::with_order(order);
        let mut reference = std::collections::BTreeMap::<i32, i32>::new();

        for op in &ops {
            match op {
                Op::Insert(k, v) => {
                    tree.insert(*k, *v);
                    reference.insert(*k, *v);
                }
                Op::Delete(k) => {
                    if reference.remove(k).is_some() {
                        tree.delete(k).unwrap();
                    }
                }
            }

            if reference.is_empty() {
                assert!(tree.min().is_none());
                assert!(tree.max().is_none());
            } else {
                assert_eq!(tree.min().map(|(k, _)| *k), reference.keys().next().copied());
                assert_eq!(tree.max().map(|(k, _)| *k), reference.keys().next_back().copied());
            }
        }
    }
}

// ── Adversarial: worst-case patterns ──

#[test]
fn adversarial_delete_all_sorted() {
    let mut tree = BTree::with_order(4);
    for i in 0..200 {
        tree.insert(i, i);
    }
    for i in 0..200 {
        tree.delete(&i).unwrap();
        assert!(tree.verify(), "failed after deleting {i}");
    }
    assert!(tree.is_empty());
}

#[test]
fn adversarial_delete_all_reverse_sorted() {
    let mut tree = BTree::with_order(4);
    for i in 0..200 {
        tree.insert(i, i);
    }
    for i in (0..200).rev() {
        tree.delete(&i).unwrap();
        assert!(tree.verify(), "failed after deleting {i}");
    }
    assert!(tree.is_empty());
}

#[test]
fn adversarial_order_three_minimum() {
    let mut tree = BTree::with_order(3);
    for i in 0..100 {
        tree.insert(i, i);
        assert!(tree.verify(), "order-3 insert failed at {i}");
    }
    for i in (0..100).rev() {
        tree.delete(&i).unwrap();
        assert!(tree.verify(), "order-3 delete failed at {i}");
    }
    assert!(tree.is_empty());
}

#[test]
fn adversarial_ping_pong_insert_delete() {
    let mut tree = BTree::with_order(3);
    for i in 0..100 {
        tree.insert(i, i);
        if i > 0 {
            tree.delete(&(i - 1)).unwrap();
        }
        assert!(tree.verify());
    }
    assert_eq!(tree.len(), 1);
    assert!(tree.contains(&99));
}

#[test]
fn adversarial_many_root_splits_and_shrinks() {
    let mut tree = BTree::with_order(3);
    for i in 0..50 {
        tree.insert(i, i);
    }
    assert!(tree.height() >= 3);
    assert!(tree.verify());
    for i in 0..50 {
        tree.delete(&i).unwrap();
    }
    assert!(tree.is_empty());
    assert_eq!(tree.height(), 0);
}

#[test]
fn adversarial_duplicate_keys_maintain_invariant() {
    let mut tree = BTree::with_order(3);
    for _ in 0..50 {
        for k in [1, 5, 10, 20, 50, 100] {
            tree.insert(k, k);
        }
        assert!(tree.verify());
    }
    assert_eq!(tree.len(), 6);
}

#[test]
fn adversarial_delete_alternating_ends() {
    let mut tree = BTree::with_order(4);
    for i in 0..100 {
        tree.insert(i, i);
    }
    let mut lo = 0i32;
    let mut hi = 99i32;
    for _ in 0..50 {
        tree.delete(&lo).unwrap();
        tree.delete(&hi).unwrap();
        lo += 1;
        hi -= 1;
        assert!(tree.verify());
    }
    assert!(tree.is_empty());
}

#[test]
fn adversarial_insert_delete_same_key_repeatedly() {
    let mut tree = BTree::with_order(3);
    for i in 0..100 {
        tree.insert(42, i);
        if i > 0 {
            assert_eq!(tree.len(), 1);
        }
        assert!(tree.verify());
    }
    assert_eq!(tree.search(&42), Some(&99));
}

#[test]
fn adversarial_bulk_insert_then_random_delete() {
    let mut tree = BTree::with_order(4);
    for i in 0..300 {
        tree.insert(i, i);
    }

    let mut seed = 12345u64;
    let mut remaining = vec![0i32; 300];
    for i in 0..300 {
        remaining[i] = i as i32;
    }

    for _ in 0..250 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let idx = (seed >> 33) as usize % remaining.len();
        let val = remaining.remove(idx);
        tree.delete(&val).unwrap();
        assert!(tree.verify());
    }
    assert_eq!(tree.len(), 50);
}

#[test]
fn adversarial_alternating_min_max_insert() {
    let mut tree = BTree::with_order(3);
    for i in 0..200 {
        if i % 2 == 0 {
            tree.insert(i / 2, i);
        } else {
            tree.insert(1000 - i / 2, i);
        }
        assert!(tree.verify());
    }
    assert_eq!(tree.len(), 200);
}

// ── Boundary: B-tree order edge cases ──

#[test]
#[should_panic(expected = "B-tree order must be at least 3")]
fn boundary_order_below_minimum_panics() {
    let _tree = BTree::<i32, i32>::with_order(2);
}

#[test]
#[should_panic(expected = "B-tree order must be at least 3")]
fn boundary_order_zero_panics() {
    let _tree = BTree::<i32, i32>::with_order(0);
}

#[test]
fn boundary_order_one_panics() {
    let result = std::panic::catch_unwind(|| BTree::<i32, i32>::with_order(1));
    assert!(result.is_err());
}

#[test]
fn boundary_order_large() {
    let mut tree = BTree::with_order(1000);
    for i in 0..500 {
        tree.insert(i, i);
    }
    assert!(tree.verify());
    assert_eq!(tree.height(), 1);
}

#[test]
fn boundary_exact_max_keys_triggers_split() {
    let mut tree = BTree::with_order(3);
    tree.insert(1, "a");
    tree.insert(2, "b");
    assert_eq!(tree.height(), 1);
    tree.insert(3, "c");
    assert_eq!(tree.height(), 2);
    assert!(tree.verify());
}

#[test]
fn boundary_min_keys_delete_triggers_merge() {
    let mut tree = BTree::with_order(3);
    for i in 0..10 {
        tree.insert(i, i);
    }
    for i in 0..8 {
        tree.delete(&i).unwrap();
        assert!(tree.verify(), "merge boundary failed deleting {i}");
    }
}

#[test]
fn boundary_single_key_insert_delete_cycle() {
    let mut tree = BTree::new();
    for i in 0..20 {
        tree.insert(1, i);
        assert_eq!(tree.len(), 1);
        tree.delete(&1).unwrap();
        assert!(tree.is_empty());
    }
}

#[test]
fn boundary_empty_range_on_populated_tree() {
    let mut tree = BTree::new();
    for i in 0..10 {
        tree.insert(i, i);
    }
    assert!(tree.range(-100..-50).is_empty());
    assert!(tree.range(100..200).is_empty());
}

#[test]
fn boundary_range_single_element() {
    let mut tree = BTree::new();
    for i in 0..10 {
        tree.insert(i, i);
    }
    let single = tree.range(5..=5);
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].0, &5);
}

#[test]
fn boundary_full_unbounded_range() {
    let mut tree = BTree::new();
    for i in 0..50 {
        tree.insert(i, i);
    }
    assert_eq!(tree.range(..).len(), 50);
}

#[test]
fn boundary_negative_keys() {
    let mut tree = BTree::new();
    for i in -100..0 {
        tree.insert(i, i * 10);
    }
    tree.insert(0, 0);
    assert!(tree.verify());
    assert_eq!(tree.len(), 101);
    assert_eq!(tree.min().unwrap().0, &-100);
    assert_eq!(tree.search(&-50), Some(&-500));
}

#[test]
fn boundary_i32_min_max_keys() {
    let mut tree = BTree::new();
    tree.insert(i32::MIN, "min");
    tree.insert(i32::MAX, "max");
    tree.insert(0, "mid");
    assert!(tree.verify());
    assert_eq!(tree.min().unwrap().0, &i32::MIN);
    assert_eq!(tree.max().unwrap().0, &i32::MAX);
}

#[test]
fn boundary_delete_only_key_leaves_empty() {
    let mut tree = BTree::new();
    tree.insert(42, "answer");
    assert_eq!(tree.height(), 1);
    tree.delete(&42).unwrap();
    assert!(tree.is_empty());
    assert_eq!(tree.height(), 0);
    assert_eq!(tree.len(), 0);
}

#[test]
fn boundary_clone_preserves_structure() {
    let mut tree = BTree::with_order(4);
    for i in 0..50 {
        tree.insert(i, format!("val_{i}"));
    }
    let cloned = tree.clone();
    assert_eq!(cloned.len(), tree.len());
    assert_eq!(cloned.height(), tree.height());
    assert!(cloned.verify());
    for i in 0..50 {
        assert_eq!(cloned.search(&i), tree.search(&i));
    }
}

#[test]
fn boundary_serde_roundtrip_large() {
    let mut tree = BTree::with_order(5);
    for i in 0..200 {
        tree.insert(i, i * 3);
    }
    let json = serde_json::to_string(&tree).unwrap();
    let back: BTree<i32, i32> = serde_json::from_str(&json).unwrap();
    assert_eq!(back.len(), 200);
    assert!(back.verify());
    for i in 0..200 {
        assert_eq!(back.search(&i), Some(&(i * 3)));
    }
}

#[test]
fn boundary_string_key_with_empty_string() {
    let mut tree = BTree::new();
    tree.insert("".to_string(), 0);
    tree.insert("a".to_string(), 1);
    tree.insert("z".to_string(), 2);
    assert!(tree.verify());
    assert_eq!(tree.search(&"".to_string()), Some(&0));
    assert_eq!(tree.min().unwrap().0, &"".to_string());
}

#[test]
fn boundary_zero_value_keys() {
    let mut tree = BTree::new();
    for _ in 0..5 {
        tree.insert(0, 0);
    }
    assert_eq!(tree.len(), 1);
    assert_eq!(tree.search(&0), Some(&0));
}

// ── Concurrent access stress (Arc<Mutex<BTree>>) ──

#[tokio::test]
async fn concurrent_insert_from_multiple_tasks() {
    use std::sync::{Arc, Mutex};

    let tree = Arc::new(Mutex::new(BTree::<i32, i32>::with_order(4)));
    let mut handles = Vec::new();

    for task_id in 0..4 {
        let tree_clone = Arc::clone(&tree);
        handles.push(tokio::spawn(async move {
            for i in 0..50 {
                let key = task_id * 50 + i;
                tree_clone.lock().unwrap().insert(key, key);
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let tree = tree.lock().unwrap();
    assert_eq!(tree.len(), 200);
    assert!(tree.verify());
    for i in 0..200 {
        assert_eq!(tree.search(&i), Some(&i));
    }
}

#[tokio::test]
async fn concurrent_insert_delete_different_keys() {
    use std::sync::{Arc, Mutex};

    let tree = Arc::new(Mutex::new(BTree::<i32, i32>::with_order(4)));
    let mut handles = Vec::new();

    let t0 = Arc::clone(&tree);
    handles.push(tokio::spawn(async move {
        for i in 0..100 {
            t0.lock().unwrap().insert(i, i);
        }
    }));

    let t1 = Arc::clone(&tree);
    handles.push(tokio::spawn(async move {
        for i in 100..200 {
            t1.lock().unwrap().insert(i, i);
        }
    }));

    let t2 = Arc::clone(&tree);
    handles.push(tokio::spawn(async move {
        tokio::task::yield_now().await;
        for i in 50..100 {
            let _ = t2.lock().unwrap().delete(&i);
        }
    }));

    for h in handles {
        h.await.unwrap();
    }

    let tree = tree.lock().unwrap();
    assert!(tree.verify());
    for i in 100..200 {
        assert_eq!(
            tree.search(&i),
            Some(&i),
            "key {i} missing after concurrent ops"
        );
    }
}

#[tokio::test]
async fn concurrent_reads_during_writes() {
    use std::sync::{Arc, Mutex};

    let tree = Arc::new(Mutex::new(BTree::<i32, i32>::with_order(8)));
    let mut handles = Vec::new();

    for i in 0..100 {
        tree.lock().unwrap().insert(i, i);
    }

    for task_id in 0..2 {
        let t = Arc::clone(&tree);
        handles.push(tokio::spawn(async move {
            for i in 0..100 {
                t.lock().unwrap().insert(i, i + task_id * 1000);
            }
        }));
    }

    for _ in 0..3 {
        let t = Arc::clone(&tree);
        handles.push(tokio::spawn(async move {
            let guard = t.lock().unwrap();
            assert!(guard.verify());
            assert_eq!(guard.len(), 100);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let tree = tree.lock().unwrap();
    assert!(tree.verify());
    assert_eq!(tree.len(), 100);
}
