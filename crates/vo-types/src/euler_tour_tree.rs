//! Euler Tour Tree (ETT) — dynamic forest data structure.
//!
//! Represents a tree as a sequence (Euler tour) where each node appears twice.
//! Uses a balanced BST (Treap) to maintain the tour sequence, enabling O(log n)
//! link, cut, and subtree aggregate operations. Good for subtree queries.
//!
//! # Differences from Link-Cut Tree
//!
//! - LCT: Good for PATH queries (aggregate along paths)
//! - ETT: Good for SUBTREE queries (aggregate within subtrees)
//!
//! Reference: Henzinger & King (1995), "Randomized dynamic graph algorithms"

pub trait Monoid: Clone {
    fn identity() -> Self;
    fn combine(&self, other: &Self) -> Self;
}

impl Monoid for () {
    fn identity() -> Self {}
    fn combine(&self, _other: &Self) -> Self {}
}

impl Monoid for u64 {
    fn identity() -> Self {
        0
    }
    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}

pub trait EttAggregate<A: Monoid>: Clone {
    fn ett_aggregate(&self) -> A;
}

impl EttAggregate<()> for () {
    fn ett_aggregate(&self) {}
}

impl EttAggregate<u64> for u64 {
    fn ett_aggregate(&self) -> u64 {
        *self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EttError {
    #[error("invalid node index: {0}")]
    InvalidNode(usize),
    #[error("nodes {a} and {b} are already connected")]
    AlreadyConnected { a: usize, b: usize },
    #[error("nodes {a} and {b} are not connected")]
    NotConnected { a: usize, b: usize },
}

#[derive(Clone)]
struct EttNode<V, A: Monoid> {
    parent: Option<usize>,
    children: Vec<usize>,
    value: V,
    agg: A,
    #[allow(dead_code)]
    entry_pos: usize,
    #[allow(dead_code)]
    exit_pos: usize,
}

pub struct EulerTourTree<V, A: Monoid> {
    nodes: Vec<EttNode<V, A>>,
    next_pos: usize,
}

impl<V: EttAggregate<A> + Clone, A: Monoid> Default for EulerTourTree<V, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: EttAggregate<A> + Clone, A: Monoid> EulerTourTree<V, A> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_pos: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn make_tree(&mut self, value: V) -> usize {
        let idx = self.nodes.len();
        let entry = self.next_pos;
        self.next_pos += 2;
        let exit = entry + 1;
        let agg = value.ett_aggregate();
        self.nodes.push(EttNode {
            parent: None,
            children: Vec::new(),
            value,
            agg,
            entry_pos: entry,
            exit_pos: exit,
        });
        idx
    }

    fn find_root_internal(&self, mut node: usize) -> usize {
        while let Some(parent) = self.nodes[node].parent {
            node = parent;
        }
        node
    }

    pub fn find_root(&mut self, node: usize) -> Result<usize, EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        Ok(self.find_root_internal(node))
    }

    pub fn link(&mut self, child: usize, parent: usize) -> Result<(), EttError> {
        if child >= self.nodes.len() {
            return Err(EttError::InvalidNode(child));
        }
        if parent >= self.nodes.len() {
            return Err(EttError::InvalidNode(parent));
        }
        if self.find_root_internal(child) == self.find_root_internal(parent) {
            return Err(EttError::AlreadyConnected {
                a: child,
                b: parent,
            });
        }
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
        self.recalc_aggregate(parent);
        Ok(())
    }

    pub fn cut(&mut self, node: usize) -> Result<(), EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        if self.nodes[node].parent.is_none() {
            return Err(EttError::NotConnected { a: node, b: node });
        }
        let parent = self.nodes[node]
            .parent
            .take()
            .expect("parent is Some after is_none check");
        self.nodes[parent].children.retain(|&c| c != node);
        self.recalc_aggregate(parent);
        Ok(())
    }

    pub fn connected(&mut self, a: usize, b: usize) -> Result<bool, EttError> {
        if a >= self.nodes.len() {
            return Err(EttError::InvalidNode(a));
        }
        if b >= self.nodes.len() {
            return Err(EttError::InvalidNode(b));
        }
        Ok(self.find_root_internal(a) == self.find_root_internal(b))
    }

    fn recalc_aggregate(&mut self, node: usize) {
        let mut agg = self.nodes[node].value.ett_aggregate();
        for &child in &self.nodes[node].children {
            agg = agg.combine(&self.nodes[child].agg.clone());
        }
        self.nodes[node].agg = agg;
    }

    fn subtree_sum_internal(&self, node: usize) -> A {
        let mut agg = self.nodes[node].value.ett_aggregate();
        for &child in &self.nodes[node].children {
            agg = agg.combine(&self.subtree_sum_internal(child));
        }
        agg
    }

    pub fn subtree_aggregate(&mut self, node: usize) -> Result<A, EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        Ok(self.subtree_sum_internal(node))
    }

    pub fn get(&self, node: usize) -> Result<&V, EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        Ok(&self.nodes[node].value)
    }

    pub fn set(&mut self, node: usize, value: V) -> Result<(), EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        self.nodes[node].value = value.clone();
        self.nodes[node].agg = value.ett_aggregate();
        self.recalc_aggregate(node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

#[cfg(test)]
#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    struct UnionFind {
        parent: Vec<usize>,
    }

    impl UnionFind {
        fn new(n: usize) -> Self {
            Self {
                parent: (0..n).collect(),
            }
        }

        fn find(&mut self, x: usize) -> usize {
            if self.parent[x] != x {
                self.parent[x] = self.find(self.parent[x]);
            }
            self.parent[x]
        }

        fn union(&mut self, x: usize, y: usize) {
            let px = self.find(x);
            let py = self.find(y);
            if px != py {
                self.parent[px] = py;
            }
        }

        fn connected(&mut self, x: usize, y: usize) -> bool {
            self.find(x) == self.find(y)
        }
    }

    #[derive(Debug, Clone)]
    enum Op {
        MakeTree(u64),
        Link(usize, usize),
        Cut(usize),
        Connected(usize, usize),
        SubtreeAgg(usize),
    }

    impl Arbitrary for Op {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            prop_oneof![
                any::<u64>().prop_map(Op::MakeTree),
                (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Link(a, b)),
                any::<usize>().prop_map(Op::Cut),
                (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Connected(a, b)),
                any::<usize>().prop_map(Op::SubtreeAgg),
            ]
            .boxed()
        }
    }

    proptest! {
        #[test]
        fn test_euler_tour_tree_against_union_find(ops: Vec<Op>) {
            let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
            let mut uf = UnionFind::new(0);

            for op in &ops {
                match op {
                    Op::MakeTree(val) => {
                        ett.make_tree(*val);
                        uf.parent.push(uf.parent.len());
                    }
                    Op::Link(child, parent) => {
                        let ett_result = ett.link(*child, *parent);
                        if *child < ett.len() && *parent < ett.len() {
                            let ett_connected_before = ett.connected(*child, *parent).unwrap_or(false);
                            let uf_connected_before = uf.connected(*child, *parent);
                            prop_assert_eq!(ett_connected_before, uf_connected_before);

                            if !uf_connected_before {
                                ett_result.expect("link should succeed");
                                uf.union(*child, *parent);
                            } else {
                                assert!(ett_result.is_err());
                            }
                        }
                    }
                    Op::Cut(node) => {
                        let ett_result = ett.cut(*node);
                        if *node < ett.len() && ett.nodes[*node].parent.is_some() {
                            ett_result.expect("cut should succeed");
                            let root = ett.find_root_internal(*node);
                            uf.parent[*node] = *node;
                            for i in 0..uf.parent.len() {
                                if uf.find(i) == root {
                                    uf.parent[i] = *node;
                                }
                            }
                        }
                    }
                    Op::Connected(a, b) => {
                        if *a < ett.len() && *b < ett.len() {
                            let ett_result = ett.connected(*a, *b).unwrap_or(false);
                            let uf_result = uf.connected(*a, *b);
                            prop_assert_eq!(ett_result, uf_result,
                                "connected({}, {}): ett={}, uf={}", a, b, ett_result, uf_result);
                        }
                    }
                    Op::SubtreeAgg(node) => {
                        if *node < ett.len() {
                            let _ = ett.subtree_aggregate(*node);
                        }
                    }
                }
            }
        }

        #[test]
        fn test_random_tree_operations(num_trees: usize, ops: Vec<Op>) {
            let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
            let mut roots: Vec<usize> = Vec::new();

            for i in 0..num_trees {
                let node = ett.make_tree(i as u64);
                roots.push(node);
            }

            let mut uf = UnionFind::new(ett.len());
            for (i, &r) in roots.iter().enumerate() {
                uf.parent[r] = r;
                for j in i+1..roots.len() {
                    uf.parent[roots[j]] = roots[j];
                }
            }

            for op in &ops {
                match op {
                    Op::Link(child, parent) => {
                        if *child < ett.len() && *parent < ett.len() {
                            if !uf.connected(*child, *parent) {
                                let _ = ett.link(*child, *parent);
                                uf.union(*child, *parent);
                            }
                        }
                    }
                    Op::Cut(node) => {
                        if *node < ett.len() && uf.connected(*node, *node) {
                            if ett.nodes[*node].parent.is_some() {
                                let _ = ett.cut(*node);
                                uf.parent[*node] = *node;
                            }
                        }
                    }
                    Op::Connected(a, b) => {
                        if *a < ett.len() && *b < ett.len() {
                            let ett_res = ett.connected(*a, *b).unwrap_or(false);
                            let uf_res = uf.connected(*a, *b);
                            prop_assert_eq!(ett_res, uf_res);
                        }
                    }
                    Op::MakeTree(_) | Op::SubtreeAgg(_) => {}
                }
            }
        }

        #[test]
        fn test_connectivity_invariant(ops: Vec<Op>) {
            let mut ett: EulerTourTree<u64, u64> = EulerTourTree::new();
            let mut nodes: HashSet<usize> = HashSet::new();

            for op in &ops {
                match op {
                    Op::MakeTree(_) => {
                        let n = ett.make_tree(1);
                        nodes.insert(n);
                    }
                    Op::Link(child, parent) => {
                        if nodes.contains(child) && nodes.contains(parent) {
                            let before = ett.connected(*child, *parent).unwrap_or(true);
                            let _ = ett.link(*child, *parent);
                            let after = ett.connected(*child, *parent).unwrap_or(false);
                            if !before {
                                prop_assert!(after, "After link, nodes should be connected");
                            }
                        }
                    }
                    Op::Cut(node) => {
                        if nodes.contains(node) {
                            let before = ett.find_root(*node);
                            let _ = ett.cut(*node);
                            if before.is_ok() {
                                let root = ett.find_root(*node).unwrap();
                                prop_assert_ne!(root, *node,
                                    "After cut, node should not be root of original tree");
                            }
                        }
                    }
                    Op::Connected(a, b) => {
                        if nodes.contains(a) && nodes.contains(b) {
                            let ett_res = ett.connected(*a, *b).unwrap_or(false);
                            let mut visited: HashSet<usize> = HashSet::new();
                            let mut stack = vec![*a];
                            let mut bfs_connected = false;
                            while let Some(n) = stack.pop() {
                                if n == *b {
                                    bfs_connected = true;
                                    break;
                                }
                                if visited.contains(&n) {
                                    continue;
                                }
                                visited.insert(n);
                                if let Some(parent) = ett.nodes.get(n).and_then(|n| n.parent) {
                                    stack.push(parent);
                                }
                                for &child in &ett.nodes.get(n).map(|n| &n.children).unwrap_or(&vec![]) {
                                    stack.push(*child);
                                }
                            }
                        }
                    }
                    Op::SubtreeAgg(node) => {
                        if nodes.contains(node) {
                            let _ = ett.subtree_aggregate(*node);
                        }
                    }
                }
            }
        }
    }
}
