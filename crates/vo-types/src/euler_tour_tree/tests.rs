use super::traits::{EttAggregate, EttError, Monoid};
use super::tree::EulerTourTree;

#[test]
fn make_tree_creates_singleton() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    let n = ett.make_tree(());
    assert_eq!(ett.len(), 1);
    assert_eq!(ett.find_root(n).unwrap(), n);
}

#[test]
fn link_two_nodes() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    assert_eq!(ett.find_root(b).unwrap(), a);
}

#[test]
fn cut_disconnects_child() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    ett.cut(b).unwrap();
    assert_eq!(ett.find_root(b).unwrap(), b);
}

#[test]
fn chain_find_root() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    let n3 = ett.make_tree(3);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.link(n3, n2).unwrap();
    assert_eq!(ett.find_root(n3).unwrap(), n0);
}

#[test]
fn cut_in_middle_splits_tree() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    let n3 = ett.make_tree(3);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.link(n3, n2).unwrap();
    ett.cut(n2).unwrap();
    assert_eq!(ett.find_root(n3).unwrap(), n2);
    assert_eq!(ett.find_root(n1).unwrap(), n0);
    assert!(!ett.connected(n0, n3).unwrap());
}

#[test]
fn subtree_aggregate() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    assert_eq!(ett.subtree_aggregate(n0).unwrap(), 3);
    assert_eq!(ett.subtree_aggregate(n1).unwrap(), 3);
    assert_eq!(ett.subtree_aggregate(n2).unwrap(), 2);
}

#[test]
fn link_non_connected_succeeds() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    let result = ett.link(b, a);
    assert!(result.is_ok());
}

#[test]
fn link_already_connected_errors() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    let result = ett.link(a, b);
    assert!(matches!(result, Err(EttError::AlreadyConnected { .. })));
}

#[test]
fn connected_after_link() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    assert!(!ett.connected(a, b).unwrap());
    ett.link(b, a).unwrap();
    assert!(ett.connected(a, b).unwrap());
}

#[test]
fn connected_after_cut() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    assert!(ett.connected(a, b).unwrap());
    ett.cut(b).unwrap();
    assert!(!ett.connected(a, b).unwrap());
}

#[test]
fn invalid_node_errors() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    ett.make_tree(());
    assert!(matches!(ett.find_root(99), Err(EttError::InvalidNode(99))));
    assert!(matches!(ett.link(99, 0), Err(EttError::InvalidNode(99))));
    assert!(matches!(ett.cut(99), Err(EttError::InvalidNode(99))));
    assert!(matches!(
        ett.connected(99, 0),
        Err(EttError::InvalidNode(99))
    ));
}

#[test]
fn re_link_after_cut() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    ett.cut(b).unwrap();
    ett.link(b, a).unwrap();
    assert!(ett.connected(a, b).unwrap());
}

#[test]
fn multiple_trees() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let t1 = ett.make_tree(1);
    let t2 = ett.make_tree(2);
    let t3 = ett.make_tree(3);
    ett.link(t2, t1).unwrap();
    let t4 = ett.make_tree(4);
    let t5 = ett.make_tree(5);
    ett.link(t5, t4).unwrap();
    assert!(ett.connected(t1, t2).unwrap());
    assert!(ett.connected(t4, t5).unwrap());
    assert!(!ett.connected(t1, t4).unwrap());
    ett.link(t3, t1).unwrap();
    assert!(ett.connected(t1, t3).unwrap());
}

#[test]
fn link_to_non_root_succeeds() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    let n3 = ett.make_tree(3);
    let result = ett.link(n3, n1);
    assert!(result.is_ok());
    assert!(ett.connected(n0, n3).unwrap());
    assert!(ett.connected(n2, n3).unwrap());
}

#[test]
fn cut_root_node_fails() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    ett.link(n1, n0).unwrap();
    let result = ett.cut(n0);
    assert!(matches!(result, Err(EttError::NotConnected { .. })));
}

#[test]
fn find_root_on_singleton_is_self() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    let n = ett.make_tree(());
    assert_eq!(ett.find_root(n).unwrap(), n);
}

#[test]
fn empty_tree_len_is_zero() {
    let ett: EulerTourTree<(), ()> = EulerTourTree::new();
    assert_eq!(ett.len(), 0);
    assert!(ett.is_empty());
}

#[test]
fn non_empty_tree_is_not_empty() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    ett.make_tree(());
    assert!(!ett.is_empty());
}

#[test]
fn subtree_aggregate_singleton() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n = ett.make_tree(42);
    assert_eq!(ett.subtree_aggregate(n).unwrap(), 42);
}

#[test]
fn subtree_aggregate_with_values() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(10);
    let n1 = ett.make_tree(20);
    let n2 = ett.make_tree(30);
    let n3 = ett.make_tree(40);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.link(n3, n2).unwrap();
    assert_eq!(ett.subtree_aggregate(n0).unwrap(), 100);
    assert_eq!(ett.subtree_aggregate(n1).unwrap(), 90);
    assert_eq!(ett.subtree_aggregate(n2).unwrap(), 70);
    assert_eq!(ett.subtree_aggregate(n3).unwrap(), 40);
}

#[test]
fn get_returns_value() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n = ett.make_tree(99);
    assert_eq!(*ett.get(n).unwrap(), 99);
}

#[test]
fn get_invalid_node_fails() {
    let ett: EulerTourTree<(), ()> = EulerTourTree::new();
    assert!(matches!(ett.get(0), Err(EttError::InvalidNode(0))));
}

#[test]
fn set_updates_value_and_aggregate() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(10);
    let n1 = ett.make_tree(20);
    ett.link(n1, n0).unwrap();
    ett.set(n0, 100).unwrap();
    assert_eq!(*ett.get(n0).unwrap(), 100);
    assert_eq!(ett.subtree_aggregate(n0).unwrap(), 120);
}

#[test]
fn set_invalid_node_fails() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    ett.make_tree(());
    assert!(matches!(ett.set(99, ()), Err(EttError::InvalidNode(99))));
}

#[test]
fn link_circular_detection() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    let result = ett.link(n0, n2);
    assert!(matches!(result, Err(EttError::AlreadyConnected { .. })));
}

#[test]
fn already_connected_via_different_path() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    let n3 = ett.make_tree(3);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.link(n3, n2).unwrap();
    let result = ett.link(n0, n3);
    assert!(matches!(result, Err(EttError::AlreadyConnected { .. })));
}

#[test]
fn cut_and_reconnect_different_parent() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.cut(n2).unwrap();
    ett.cut(n1).unwrap();
    ett.link(n1, n2).unwrap();
    assert!(!ett.connected(n0, n1).unwrap());
    assert!(ett.connected(n1, n2).unwrap());
    assert!(!ett.connected(n0, n2).unwrap());
}

#[test]
fn forest_isolation() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let tree1_root = ett.make_tree(1);
    let tree1_child = ett.make_tree(2);
    ett.link(tree1_child, tree1_root).unwrap();
    let tree2_root = ett.make_tree(3);
    let tree2_child = ett.make_tree(4);
    ett.link(tree2_child, tree2_root).unwrap();
    assert!(!ett.connected(tree1_root, tree2_root).unwrap());
    assert!(!ett.connected(tree1_child, tree2_root).unwrap());
    assert!(!ett.connected(tree1_root, tree2_child).unwrap());
    assert!(ett.connected(tree1_root, tree1_child).unwrap());
    assert!(ett.connected(tree2_root, tree2_child).unwrap());
}

#[test]
fn connectivity_reflexive() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n = ett.make_tree(0);
    assert!(ett.connected(n, n).unwrap());
}

#[test]
fn connectivity_symmetric_after_link() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    assert!(ett.connected(a, b).unwrap());
    assert!(ett.connected(b, a).unwrap());
}

#[test]
fn connectivity_symmetric_after_cut() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let a = ett.make_tree(0);
    let b = ett.make_tree(1);
    ett.link(b, a).unwrap();
    ett.cut(b).unwrap();
    assert!(!ett.connected(a, b).unwrap());
    assert!(!ett.connected(b, a).unwrap());
}

#[test]
fn connectivity_transitive() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(0);
    let n1 = ett.make_tree(1);
    let n2 = ett.make_tree(2);
    let n3 = ett.make_tree(3);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.link(n3, n2).unwrap();
    assert!(ett.connected(n0, n1).unwrap());
    assert!(ett.connected(n1, n2).unwrap());
    assert!(ett.connected(n2, n3).unwrap());
    assert!(ett.connected(n0, n3).unwrap());
}

#[test]
fn invalid_node_find_root() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    assert!(matches!(ett.find_root(0), Err(EttError::InvalidNode(0))));
}

#[test]
fn invalid_node_subtree_aggregate() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    assert!(matches!(
        ett.subtree_aggregate(0),
        Err(EttError::InvalidNode(0))
    ));
}

#[test]
fn cut_all_children_from_root() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let root = ett.make_tree(0);
    let c1 = ett.make_tree(1);
    let c2 = ett.make_tree(2);
    let c3 = ett.make_tree(3);
    ett.link(c1, root).unwrap();
    ett.link(c2, root).unwrap();
    ett.link(c3, root).unwrap();
    assert!(ett.connected(root, c1).unwrap());
    assert!(ett.connected(root, c2).unwrap());
    assert!(ett.connected(root, c3).unwrap());
    ett.cut(c1).unwrap();
    ett.cut(c2).unwrap();
    ett.cut(c3).unwrap();
    assert_eq!(ett.find_root(root).unwrap(), root);
    assert_eq!(ett.find_root(c1).unwrap(), c1);
    assert_eq!(ett.find_root(c2).unwrap(), c2);
    assert_eq!(ett.find_root(c3).unwrap(), c3);
}

#[test]
fn make_tree_increments_len() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    assert_eq!(ett.len(), 0);
    ett.make_tree(1);
    assert_eq!(ett.len(), 1);
    ett.make_tree(2);
    assert_eq!(ett.len(), 2);
    ett.make_tree(3);
    assert_eq!(ett.len(), 3);
}

#[test]
fn unit_monoid_aggregate() {
    let mut ett: EulerTourTree<(), ()> = EulerTourTree::new();
    let n0 = ett.make_tree(());
    let n1 = ett.make_tree(());
    let n2 = ett.make_tree(());
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    assert!(matches!(ett.subtree_aggregate(n0), Ok(())));
    assert!(matches!(ett.subtree_aggregate(n1), Ok(())));
    assert!(matches!(ett.subtree_aggregate(n2), Ok(())));
}

#[test]
fn large_value_aggregate() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let root = ett.make_tree(u64::MAX / 2);
    let child = ett.make_tree(u64::MAX / 2);
    ett.link(child, root).unwrap();
    let result = ett.subtree_aggregate(root).unwrap();
    assert_eq!(result, u64::MAX - 1);
}

#[test]
fn cut_nonexistent_parent_fails() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    ett.make_tree(0);
    let result = ett.cut(0);
    assert!(matches!(result, Err(EttError::NotConnected { .. })));
}

#[test]
fn subtree_aggregate_after_multiple_cuts() {
    let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
    let n0 = ett.make_tree(1);
    let n1 = ett.make_tree(2);
    let n2 = ett.make_tree(3);
    let n3 = ett.make_tree(4);
    ett.link(n1, n0).unwrap();
    ett.link(n2, n1).unwrap();
    ett.link(n3, n2).unwrap();
    ett.cut(n2).unwrap();
    assert_eq!(ett.subtree_aggregate(n0).unwrap(), 3);
    assert_eq!(ett.subtree_aggregate(n2).unwrap(), 7);
}
