#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_is_empty() {
        let tree: BTree<i32, String> = BTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn default_is_empty() {
        let tree: BTree<i32, i32> = BTree::default();
        assert!(tree.is_empty());
    }

    #[test]
    fn insert_single_element() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.search(&1), Some(&"a".to_string()));
    }

    #[test]
    fn search_missing_key_returns_none() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        assert_eq!(tree.search(&99), None);
    }

    #[test]
    fn search_empty_tree_returns_none() {
        let tree: BTree<i32, String> = BTree::new();
        assert_eq!(tree.search(&1), None);
    }

    #[test]
    fn insert_updates_existing_key() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        tree.insert(1, "b".to_string());
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.search(&1), Some(&"b".to_string()));
    }

    #[test]
    fn insert_many_maintains_order() {
        let mut tree = BTree::new();
        for i in (0..50).rev() {
            tree.insert(i, format!("val_{i}"));
        }
        assert_eq!(tree.len(), 50);
        assert!(tree.verify());

        for i in 0..50 {
            assert_eq!(tree.search(&i), Some(&format!("val_{i}")));
        }
    }

    #[test]
    fn delete_existing_key() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        tree.insert(2, "b".to_string());
        tree.insert(3, "c".to_string());

        let removed = tree.delete(&2).unwrap();
        assert_eq!(removed, "b".to_string());
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.search(&2), None);
        assert!(tree.verify());
    }

    #[test]
    fn delete_missing_key_returns_error() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        assert!(matches!(tree.delete(&99), Err(BTreeError::KeyNotFound)));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn delete_from_empty_tree_returns_error() {
        let mut tree: BTree<i32, String> = BTree::new();
        assert!(matches!(tree.delete(&1), Err(BTreeError::KeyNotFound)));
    }

    #[test]
    fn delete_all_elements() {
        let mut tree = BTree::new();
        for i in 0..20 {
            tree.insert(i, i);
        }
        for i in 0..20 {
            tree.delete(&i).unwrap();
        }
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn contains_key() {
        let mut tree = BTree::new();
        tree.insert(42, "answer");
        assert!(tree.contains(&42));
        assert!(!tree.contains(&1));
    }

    #[test]
    fn min_returns_smallest() {
        let mut tree = BTree::new();
        tree.insert(5, "e");
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(4, "d");
        tree.insert(2, "b");

        let (k, v) = tree.min().unwrap();
        assert_eq!(k, &1);
        assert_eq!(v, &"a");
    }

    #[test]
    fn max_returns_largest() {
        let mut tree = BTree::new();
        tree.insert(5, "e");
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(4, "d");
        tree.insert(2, "b");

        let (k, v) = tree.max().unwrap();
        assert_eq!(k, &5);
        assert_eq!(v, &"e");
    }

    #[test]
    fn min_max_on_empty_returns_none() {
        let tree: BTree<i32, String> = BTree::new();
        assert!(tree.min().is_none());
        assert!(tree.max().is_none());
    }

    #[test]
    fn range_query() {
        let mut tree = BTree::new();
        for i in 0..20 {
            tree.insert(i, i * 10);
        }

        let results = tree.range(5..15);
        assert_eq!(results.len(), 10);
        for (k, v) in &results {
            assert!(**k >= 5 && **k < 15);
            assert_eq!(**v, **k * 10);
        }
    }

    #[test]
    fn range_query_empty_result() {
        let mut tree = BTree::new();
        tree.insert(1, 10);
        tree.insert(5, 50);

        let results = tree.range(2..4);
        assert!(results.is_empty());
    }

    #[test]
    fn range_query_inclusive() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        let results = tree.range(3..=7);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn range_query_unbounded_start() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        let results = tree.range(..5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn range_query_unbounded_end() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        let results = tree.range(5..);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn height_increases_with_size() {
        let mut tree = BTree::with_order(4);
        assert_eq!(tree.height(), 0);

        for i in 0..10 {
            tree.insert(i, i);
        }
        assert!(tree.height() >= 2);
        assert!(tree.verify());
    }

    #[test]
    fn height_of_single_element() {
        let mut tree = BTree::new();
        tree.insert(1, "a");
        assert_eq!(tree.height(), 1);
    }

    #[test]
    fn verify_empty_tree() {
        let tree: BTree<i32, String> = BTree::new();
        assert!(tree.verify());
    }

    #[test]
    fn verify_after_inserts() {
        let mut tree = BTree::with_order(4);
        for i in 0..100 {
            tree.insert(i, i);
            assert!(tree.verify(), "tree invalid after inserting {i}");
        }
    }

    #[test]
    fn verify_after_deletes() {
        let mut tree = BTree::with_order(4);
        for i in 0..100 {
            tree.insert(i, i);
        }
        for i in (0..100).rev() {
            tree.delete(&i).unwrap();
            assert!(tree.verify(), "tree invalid after deleting {i}");
        }
    }

    #[test]
    fn delete_triggers_node_merge() {
        let mut tree = BTree::with_order(4);
        for i in 0..20 {
            tree.insert(i, i);
        }
        for i in 0..15 {
            tree.delete(&i).unwrap();
        }
        assert_eq!(tree.len(), 5);
        assert!(tree.verify());
        for i in 15..20 {
            assert_eq!(tree.search(&i), Some(&i));
        }
    }

    #[test]
    fn delete_triggers_borrow_from_left() {
        let mut tree = BTree::with_order(4);
        for i in 0..10 {
            tree.insert(i, i);
        }
        tree.delete(&9).unwrap();
        tree.delete(&8).unwrap();
        tree.delete(&7).unwrap();
        assert!(tree.verify());
        assert_eq!(tree.len(), 7);
    }

    #[test]
    fn delete_triggers_borrow_from_right() {
        let mut tree = BTree::with_order(4);
        for i in 0..10 {
            tree.insert(i, i);
        }
        tree.delete(&0).unwrap();
        tree.delete(&1).unwrap();
        tree.delete(&2).unwrap();
        assert!(tree.verify());
        assert_eq!(tree.len(), 7);
    }

    #[test]
    fn from_vec_builds_correct_tree() {
        let pairs = vec![(3, "c"), (1, "a"), (2, "b")];
        let tree: BTree<i32, &str> = BTree::from(pairs);
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.search(&1), Some(&"a"));
        assert_eq!(tree.search(&2), Some(&"b"));
        assert_eq!(tree.search(&3), Some(&"c"));
        assert!(tree.verify());
    }

    #[test]
    fn root_split_creates_new_root() {
        let mut tree = BTree::with_order(3);
        tree.insert(1, "a");
        tree.insert(2, "b");
        assert_eq!(tree.height(), 1);

        tree.insert(3, "c");
        assert_eq!(tree.height(), 2);
        assert!(tree.verify());
    }

    #[test]
    fn sequential_insert_delete_cycle() {
        let mut tree = BTree::with_order(4);
        for round in 0..5 {
            for i in 0..50 {
                tree.insert(i, format!("r{round}_v{i}"));
            }
            assert!(tree.verify());
            for i in 0..50 {
                tree.delete(&i).unwrap();
            }
            assert!(tree.is_empty());
        }
    }

    #[test]
    fn string_keys() {
        let mut tree = BTree::new();
        tree.insert("banana".to_string(), 2);
        tree.insert("apple".to_string(), 1);
        tree.insert("cherry".to_string(), 3);

        assert_eq!(tree.min().unwrap().0, &"apple".to_string());
        assert_eq!(tree.max().unwrap().0, &"cherry".to_string());
        assert!(tree.verify());
    }

    #[test]
    fn serde_roundtrip() {
        let mut tree = BTree::new();
        for i in 0..20 {
            tree.insert(i, i * 10);
        }
        let json = serde_json::to_string(&tree).unwrap();
        let back: BTree<i32, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(tree.len(), back.len());
        for i in 0..20 {
            assert_eq!(back.search(&i), Some(&(i * 10)));
        }
        assert!(back.verify());
    }

    #[test]
    fn btree_node_leaf_is_leaf() {
        let node = BTreeNode::leaf(vec![1, 2], vec!["a", "b"]);
        assert!(node.is_leaf());
    }

    #[test]
    fn btree_node_search_index() {
        let node = BTreeNode::leaf(vec![1, 3, 5], vec!["a", "b", "c"]);
        assert_eq!(node.search_index(&0), 0);
        assert_eq!(node.search_index(&1), 0);
        assert_eq!(node.search_index(&2), 1);
        assert_eq!(node.search_index(&3), 1);
        assert_eq!(node.search_index(&4), 2);
        assert_eq!(node.search_index(&5), 2);
        assert_eq!(node.search_index(&6), 3);
    }

    #[test]
    fn with_order_custom() {
        let tree = BTree::<i32, i32>::with_order(5);
        assert_eq!(tree.max_keys(), 4);
        assert_eq!(tree.min_keys(), 2);
    }

    #[test]
    fn large_scale_insert_and_search() {
        let mut tree = BTree::with_order(32);
        let n = 1000;
        for i in 0..n {
            tree.insert(i, i * 2);
        }
        assert_eq!(tree.len(), n);
        assert!(tree.verify());
        for i in 0..n {
            assert_eq!(tree.search(&i), Some(&(i * 2)));
        }
    }

    #[test]
    fn large_scale_delete_and_verify() {
        let mut tree = BTree::with_order(32);
        let n = 500;
        for i in 0..n {
            tree.insert(i, i);
        }
        for i in (0..n).step_by(2) {
            tree.delete(&i).unwrap();
        }
        assert_eq!(tree.len(), n / 2);
        assert!(tree.verify());
        for i in (0..n).step_by(2) {
            assert!(tree.search(&i).is_none());
        }
        for i in (1..n).step_by(2) {
            assert_eq!(tree.search(&i), Some(&i));
        }
    }

    #[test]
    fn delete_root_key_when_root_is_leaf() {
        let mut tree = BTree::new();
        tree.insert(1, "a");
        tree.delete(&1).unwrap();
        assert!(tree.is_empty());
        assert_eq!(tree.height(), 0);
    }

    #[test]
    fn interleaved_insert_delete() {
        let mut tree = BTree::with_order(4);
        tree.insert(10, 10);
        tree.insert(20, 20);
        tree.insert(5, 5);
        tree.delete(&10).unwrap();
        tree.insert(15, 15);
        tree.delete(&5).unwrap();
        tree.insert(25, 25);
        tree.delete(&20).unwrap();

        assert!(tree.verify());
        assert_eq!(tree.len(), 2);
        assert!(tree.contains(&15));
        assert!(tree.contains(&25));
    }

    #[test]
    fn range_after_deletes() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }
        tree.delete(&3).unwrap();
        tree.delete(&7).unwrap();

        let results = tree.range(2..=8);
        let keys: Vec<&i32> = results.iter().map(|(k, _)| k).copied().collect();
        assert_eq!(keys, vec![&2, &4, &5, &6, &8]);
    }

    #[test]
    fn insert_delete_interleaved_stress() {
        let mut tree = BTree::with_order(4);
        let mut values: Vec<i32> = Vec::new();

        for i in 0..200 {
            tree.insert(i, i);
            values.push(i);
            assert!(tree.verify());
        }

        let mut rng_seed = 42u64;
        for _ in 0..100 {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let idx = (rng_seed >> 33) as usize % values.len();
            let val = values.remove(idx);
            tree.delete(&val).unwrap();
            assert!(tree.verify());
        }

        assert_eq!(tree.len(), 100);
        for &v in &values {
            assert_eq!(tree.search(&v), Some(&v));
        }
    }

    // ── Adversarial: worst-case patterns ──

    #[test]
    fn adversarial_sorted_insertion_500() {
        let mut tree = BTree::with_order(4);
        for i in 0..500 {
            tree.insert(i, i);
            assert!(tree.verify(), "failed after sorted insert {i}");
        }
        assert_eq!(tree.len(), 500);
        for i in 0..500 {
            assert_eq!(tree.search(&i), Some(&i));
        }
    }

    #[test]
    fn adversarial_reverse_sorted_insertion() {
        let mut tree = BTree::with_order(4);
        for i in (0..500).rev() {
            tree.insert(i, i);
            assert!(tree.verify(), "failed after reverse sorted insert {i}");
        }
        assert_eq!(tree.len(), 500);
    }
}
