use super::RedBlackTree;

#[test]
fn insert_and_get() {
    let mut t = RedBlackTree::new();
    t.insert(1, "a");
    t.insert(2, "b");
    t.insert(3, "c");
    assert_eq!(t.get(&1), Some(&"a"));
    assert_eq!(t.get(&4), None);
}

#[test]
fn update_existing() {
    let mut t = RedBlackTree::new();
    t.insert(1, "a");
    t.insert(1, "b");
    assert_eq!(t.get(&1), Some(&"b"));
    assert_eq!(t.len(), 1);
}

#[test]
fn remove() {
    let mut t = RedBlackTree::new();
    t.insert(1, "a");
    t.insert(2, "b");
    t.insert(3, "c");
    assert!(t.remove(&2));
    assert_eq!(t.get(&2), None);
    assert_eq!(t.len(), 2);
}

#[test]
fn empty() {
    let t: RedBlackTree<i32, &str> = RedBlackTree::new();
    assert!(t.is_empty());
    assert_eq!(t.len(), 0);
}

#[test]
fn sorted_iter() {
    let mut t = RedBlackTree::new();
    t.insert(3, "c");
    t.insert(1, "a");
    t.insert(2, "b");
    assert_eq!(t.keys().collect::<Vec<_>>(), vec![&1, &2, &3]);
}

#[test]
fn from_iter() {
    let t: RedBlackTree<i32, &str> = vec![(3, "c"), (1, "a"), (2, "b")].into_iter().collect();
    assert_eq!(t.len(), 3);
    assert_eq!(t.get(&1), Some(&"a"));
}

#[test]
fn min_max() {
    let mut t = RedBlackTree::new();
    t.insert(3, "c");
    t.insert(1, "a");
    t.insert(2, "b");
    assert_eq!(t.minimum(), Some((&1, &"a")));
    assert_eq!(t.maximum(), Some((&3, &"c")));
}

#[test]
fn clear() {
    let mut t = RedBlackTree::new();
    t.insert(1, "a");
    t.clear();
    assert!(t.is_empty());
}

#[test]
fn contains() {
    let mut t = RedBlackTree::new();
    t.insert(1, "a");
    assert!(t.contains(&1));
    assert!(!t.contains(&2));
}

#[test]
fn range() {
    let mut t = RedBlackTree::new();
    for i in 1..=10 {
        t.insert(i, i);
    }
    let keys: Vec<_> = t.range(Some(&3), Some(&7)).map(|(k, _)| *k).collect();
    assert_eq!(keys, vec![3, 4, 5, 6]);
}

#[test]
fn bulk_ops() {
    let mut t = RedBlackTree::new();
    for i in 1..=200 {
        t.insert(i, i);
    }
    assert_eq!(t.len(), 200);
    for i in 1..=100 {
        assert!(t.remove(&i));
    }
    assert_eq!(t.len(), 100);
}

#[test]
fn delete_all() {
    let mut t = RedBlackTree::new();
    for i in 1..=50 {
        t.insert(i, i);
    }
    for i in 1..=50 {
        t.remove(&i);
    }
    assert!(t.is_empty());
}

#[test]
fn remove_missing() {
    let mut t = RedBlackTree::new();
    t.insert(1, "a");
    assert!(!t.remove(&99));
}

#[test]
fn reverse_order() {
    let mut t = RedBlackTree::new();
    for i in (1..=100).rev() {
        t.insert(i, i);
    }
    let keys: Vec<_> = t.keys().copied().collect();
    let expected: Vec<_> = (1..=100).collect();
    assert_eq!(keys, expected);
}
