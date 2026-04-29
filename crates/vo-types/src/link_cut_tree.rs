//! Link-Cut Tree (LCT) — dynamic forest data structure.
//!
//! Provides amortized O(log n) operations for maintaining a forest of rooted
//! trees with dynamic `link`, `cut`, and path queries. Uses splay-tree-based
//! preferred-path decomposition (Sleator–Tarjan, 1983).

use std::sync::RwLock;

// ── Monoid ─────────────────────────────────────────────────────────

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

pub trait LctAggregate<A: Monoid> {
    fn lct_aggregate(&self) -> A;
}

impl LctAggregate<()> for () {
    fn lct_aggregate(&self) {}
}

impl LctAggregate<u64> for u64 {
    fn lct_aggregate(&self) -> u64 {
        *self
    }
}

// ── Errors ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LctError {
    #[error("invalid node index: {0}")]
    InvalidNode(usize),
    #[error("node {node} is not a root")]
    NotRoot { node: usize },
    #[error("node {node} is already a root")]
    AlreadyRoot { node: usize },
}

// ── Internal Node ──────────────────────────────────────────────────

struct Node<V, A> {
    ch: [Option<usize>; 2], // [left, right] in splay tree
    parent: Option<usize>,
    rev: bool,
    value: V,
    agg: A,
}

// ── LinkCutTree ────────────────────────────────────────────────────

pub struct LinkCutTree<V, A: Monoid> {
    nodes: RwLock<Vec<Node<V, A>>>,
}

unsafe impl<V: Send, A: Send + Monoid> Send for LinkCutTree<V, A> {}
unsafe impl<V: Send + Sync, A: Send + Sync + Monoid> Sync for LinkCutTree<V, A> {}

impl<V: LctAggregate<A>, A: Monoid> Default for LinkCutTree<V, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: LctAggregate<A>, A: Monoid> LinkCutTree<V, A> {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(Vec::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.read().unwrap().len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.read().unwrap().is_empty()
    }

    pub fn make_tree(&self, value: V) -> usize {
        let idx = {
            let nodes = self.nodes.read().unwrap();
            nodes.len()
        };
        let mut nodes = self.nodes.write().unwrap();
        nodes.push(Node {
            ch: [None, None],
            parent: None,
            rev: false,
            agg: value.lct_aggregate(),
            value,
        });
        idx
    }

    // ── Splay internals ────────────────────────────────────────────

    fn push(nodes: &mut [Node<V, A>], x: usize) {
        if nodes[x].rev {
            nodes[x].rev = false;
            nodes[x].ch.swap(0, 1);
            let ch = nodes[x].ch;
            for &c in ch.iter() {
                if let Some(c) = c {
                    nodes[c].rev ^= true;
                }
            }
        }
    }

    fn pull(nodes: &mut [Node<V, A>], x: usize) {
        let mut agg = nodes[x].value.lct_aggregate();
        for &c in nodes[x].ch.iter() {
            if let Some(c) = c {
                agg = agg.combine(&nodes[c].agg.clone());
            }
        }
        nodes[x].agg = agg;
    }

    fn is_root(nodes: &[Node<V, A>], x: usize) -> bool {
        let Some(p) = nodes[x].parent else {
            return true;
        };
        nodes[p].ch[0] != Some(x) && nodes[p].ch[1] != Some(x)
    }

    fn dir(nodes: &[Node<V, A>], x: usize) -> usize {
        let p = nodes[x]
            .parent
            .expect("LCT node has no parent despite not being root");
        if nodes[p].ch[1] == Some(x) {
            1
        } else {
            0
        }
    }

    fn rotate(nodes: &mut Vec<Node<V, A>>, x: usize) {
        let p = nodes[x]
            .parent
            .expect("LCT node has no parent despite not being root");
        let g = nodes[p].parent;
        let d = Self::dir(nodes, x);

        Self::push(nodes, p);
        Self::push(nodes, x);

        nodes[p].ch[d] = nodes[x].ch[1 - d];
        if let Some(c) = nodes[p].ch[d] {
            nodes[c].parent = Some(p);
        }

        nodes[x].ch[1 - d] = Some(p);
        nodes[p].parent = Some(x);
        nodes[x].parent = g;

        if let Some(g) = g {
            if nodes[g].ch[0] == Some(p) {
                nodes[g].ch[0] = Some(x);
            } else if nodes[g].ch[1] == Some(p) {
                nodes[g].ch[1] = Some(x);
            }
        }

        Self::pull(nodes, p);
        Self::pull(nodes, x);
    }

    fn splay(nodes: &mut Vec<Node<V, A>>, x: usize) {
        Self::push(nodes, x);
        while !Self::is_root(nodes, x) {
            let p = nodes[x]
                .parent
                .expect("LCT node has no parent in splay loop");
            if !Self::is_root(nodes, p) {
                let _g = nodes[p]
                    .parent
                    .expect("LCT grandparent missing despite non-root parent");
                if Self::dir(nodes, x) == Self::dir(nodes, p) {
                    Self::rotate(nodes, p);
                } else {
                    Self::rotate(nodes, x);
                }
            }
            Self::rotate(nodes, x);
        }
    }

    fn expose(nodes: &mut Vec<Node<V, A>>, x: usize) {
        Self::splay(nodes, x);
        nodes[x].ch[1] = None;
        Self::pull(nodes, x);

        let mut cur = x;
        while let Some(pp) = nodes[cur].parent {
            Self::splay(nodes, pp);
            nodes[pp].ch[1] = None;
            nodes[pp].ch[1] = Some(cur);
            nodes[cur].parent = Some(pp);
            Self::pull(nodes, pp);
            cur = pp;
        }
        Self::splay(nodes, x);
    }

    fn find_path_parent(nodes: &[Node<V, A>], x: usize) -> Option<usize> {
        let p = nodes[x].parent?;
        Some(p)
    }

    #[allow(dead_code)]
    fn evert(nodes: &mut Vec<Node<V, A>>, x: usize) {
        Self::expose(nodes, x);
        nodes[x].rev ^= true;
        Self::push(nodes, x);
    }

    // ── Public API ─────────────────────────────────────────────────

    pub fn find_root(&self, node: usize) -> Result<usize, LctError> {
        if node >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(node));
        }
        let mut nodes = self.nodes.write().unwrap();
        Self::expose(&mut nodes, node);
        let mut x = node;
        loop {
            Self::push(&mut nodes, x);
            if let Some(l) = nodes[x].ch[0] {
                x = l;
            } else {
                break;
            }
        }
        Self::splay(&mut nodes, x);
        Ok(x)
    }

    pub fn link(&self, child: usize, parent: usize) -> Result<(), LctError> {
        if child >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(child));
        }
        if parent >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(parent));
        }
        let mut nodes = self.nodes.write().unwrap();
        Self::expose(&mut nodes, child);
        if nodes[child].ch[0].is_some() {
            return Err(LctError::NotRoot { node: child });
        }
        nodes[child].parent = Some(parent);
        Ok(())
    }

    pub fn cut(&self, node: usize) -> Result<(), LctError> {
        if node >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(node));
        }
        let mut nodes = self.nodes.write().unwrap();
        Self::expose(&mut nodes, node);
        match nodes[node].ch[0] {
            Some(l) => {
                Self::push(&mut nodes, node);
                nodes[l].parent = None;
                nodes[node].ch[0] = None;
                Self::pull(&mut nodes, node);
                Ok(())
            }
            None => Err(LctError::AlreadyRoot { node }),
        }
    }

    pub fn path_aggregate(&self, node: usize) -> Result<A, LctError> {
        if node >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(node));
        }
        let mut nodes = self.nodes.write().unwrap();
        Self::expose(&mut nodes, node);
        Ok(nodes[node].agg.clone())
    }

    pub fn lca(&self, a: usize, b: usize) -> Result<usize, LctError> {
        if a >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(a));
        }
        if b >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(b));
        }
        if a == b {
            return Ok(a);
        }
        let mut nodes = self.nodes.write().unwrap();
        Self::expose(&mut nodes, a);
        Self::expose(&mut nodes, b);
        Self::splay(&mut nodes, a);
        if nodes[a].parent.is_none() {
            return Ok(a);
        }
        let mut x = a;
        while let Some(p) = nodes[x].parent {
            if nodes[p].ch[0] != Some(x) && nodes[p].ch[1] != Some(x) {
                return Ok(p);
            }
            x = p;
        }
        Ok(x)
    }

    pub fn connected(&self, a: usize, b: usize) -> Result<bool, LctError> {
        let ra = self.find_root(a)?;
        let rb = self.find_root(b)?;
        Ok(ra == rb)
    }

    pub fn get(&self, node: usize) -> Result<V, LctError>
    where
        V: Clone,
    {
        if node >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(node));
        }
        Ok(self.nodes.read().unwrap()[node].value.clone())
    }

    pub fn set(&self, node: usize, value: V) -> Result<(), LctError> {
        if node >= self.nodes.read().unwrap().len() {
            return Err(LctError::InvalidNode(node));
        }
        let mut nodes = self.nodes.write().unwrap();
        Self::splay(&mut nodes, node);
        nodes[node].value = value;
        Self::pull(&mut nodes, node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_tree_creates_singleton() {
        let lct: LinkCutTree<(), ()> = LinkCutTree::new();
        let n = lct.make_tree(());
        assert_eq!(lct.len(), 1);
        assert_eq!(lct.find_root(n).unwrap(), n);
    }

    #[test]
    fn link_two_nodes() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let a = lct.make_tree(0);
        let b = lct.make_tree(1);
        lct.link(b, a).unwrap();
        assert_eq!(lct.find_root(b).unwrap(), a);
    }

    #[test]
    fn cut_disconnects_child() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let a = lct.make_tree(0);
        let b = lct.make_tree(1);
        lct.link(b, a).unwrap();
        lct.cut(b).unwrap();
        assert_eq!(lct.find_root(b).unwrap(), b);
    }

    #[test]
    fn chain_find_root() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        let n3 = lct.make_tree(3);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        lct.link(n3, n2).unwrap();
        assert_eq!(lct.find_root(n3).unwrap(), n0);
    }

    #[test]
    fn cut_in_middle_splits_tree() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        let n3 = lct.make_tree(3);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        lct.link(n3, n2).unwrap();
        lct.cut(n2).unwrap();
        assert_eq!(lct.find_root(n3).unwrap(), n2);
        assert_eq!(lct.find_root(n1).unwrap(), n0);
        assert!(!lct.connected(n0, n3).unwrap());
    }

    #[test]
    fn path_aggregate_sum() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        let n3 = lct.make_tree(3);
        let n4 = lct.make_tree(4);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        lct.link(n3, n2).unwrap();
        lct.link(n4, n3).unwrap();
        assert_eq!(lct.path_aggregate(n4).unwrap(), 10);
    }

    #[test]
    fn lca_returns_ancestor() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        assert_eq!(lct.lca(n0, n2).unwrap(), n0);
    }

    #[test]
    fn link_non_root_errors() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        lct.link(n1, n0).unwrap();
        assert!(matches!(lct.link(n1, n2), Err(LctError::NotRoot { .. })));
    }

    #[test]
    fn cut_root_errors() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        assert!(matches!(lct.cut(n0), Err(LctError::AlreadyRoot { .. })));
    }

    #[test]
    fn reconnect_after_cut() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        lct.cut(n1).unwrap();
        assert!(!lct.connected(n0, n2).unwrap());
        lct.link(n1, n0).unwrap();
        assert!(lct.connected(n0, n2).unwrap());
    }

    #[test]
    fn set_updates_aggregate() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        lct.set(n1, 100).unwrap();
        assert_eq!(lct.path_aggregate(n2).unwrap(), 102);
    }

    #[test]
    fn invalid_node_errors() {
        let lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        lct.make_tree(0);
        assert!(matches!(lct.find_root(99), Err(LctError::InvalidNode(99))));
    }

    #[test]
    fn concurrent_find_root() {
        use std::sync::Arc;
        use std::thread;

        let lct: Arc<LinkCutTree<u64, u64>> = Arc::new(LinkCutTree::new());
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        let n3 = lct.make_tree(3);
        let n4 = lct.make_tree(4);
        let n5 = lct.make_tree(5);
        let n6 = lct.make_tree(6);
        let n7 = lct.make_tree(7);

        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        lct.link(n3, n2).unwrap();
        lct.link(n4, n3).unwrap();
        lct.link(n5, n4).unwrap();
        lct.link(n6, n5).unwrap();
        lct.link(n7, n6).unwrap();

        let lct_clone = Arc::clone(&lct);
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let lct = Arc::clone(&lct_clone);
                thread::spawn(move || {
                    let node = [n0, n1, n2, n3, n4, n5, n6, n7][i];
                    for _ in 0..1000 {
                        let root = lct.find_root(node).unwrap();
                        assert_eq!(root, n0);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
