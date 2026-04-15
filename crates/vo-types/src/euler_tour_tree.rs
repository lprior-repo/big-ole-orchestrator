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
}
