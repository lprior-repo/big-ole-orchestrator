//! Link-Cut Tree (LCT) — dynamic forest data structure.
//!
//! Provides amortized O(log n) operations for maintaining a forest of rooted
//! trees with dynamic `link`, `cut`, and path queries. Uses splay-tree-based
//! preferred-path decomposition (Sleator–Tarjan, 1983).

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
    nodes: Vec<Node<V, A>>,
}

impl<V: LctAggregate<A>, A: Monoid> Default for LinkCutTree<V, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: LctAggregate<A>, A: Monoid> LinkCutTree<V, A> {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn make_tree(&mut self, value: V) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(Node {
            ch: [None, None],
            parent: None,
            rev: false,
            agg: value.lct_aggregate(),
            value,
        });
        idx
    }

    // ── Splay internals ────────────────────────────────────────────

    fn push(&mut self, x: usize) {
        if self.nodes[x].rev {
            self.nodes[x].rev = false;
            self.nodes[x].ch.swap(0, 1);
            let ch = self.nodes[x].ch;
            for &c in ch.iter() {
                if let Some(c) = c {
                    self.nodes[c].rev ^= true;
                }
            }
        }
    }

    fn pull(&mut self, x: usize) {
        let mut agg = self.nodes[x].value.lct_aggregate();
        for &c in self.nodes[x].ch.iter() {
            if let Some(c) = c {
                agg = agg.combine(&self.nodes[c].agg.clone());
            }
        }
        self.nodes[x].agg = agg;
    }

    /// Is x a root of its preferred-path splay tree?
    /// (i.e., its parent does NOT point to x as a child)
    fn is_root(&self, x: usize) -> bool {
        let Some(p) = self.nodes[x].parent else {
            return true;
        };
        self.nodes[p].ch[0] != Some(x) && self.nodes[p].ch[1] != Some(x)
    }

    /// Which side is x on in its parent?
    #[allow(clippy::expect_used)]
    fn dir(&self, x: usize) -> usize {
        let p = self.nodes[x]
            .parent
            .expect("LCT node has no parent despite not being root");
        if self.nodes[p].ch[1] == Some(x) {
            1
        } else {
            0
        }
    }

    #[allow(clippy::expect_used)]
    fn rotate(&mut self, x: usize) {
        let p = self.nodes[x]
            .parent
            .expect("LCT node has no parent despite not being root");
        let g = self.nodes[p].parent;
        let d = self.dir(x);

        self.push(p);
        self.push(x);

        // x's opposite child becomes p's child on d-side
        self.nodes[p].ch[d] = self.nodes[x].ch[1 - d];
        if let Some(c) = self.nodes[p].ch[d] {
            self.nodes[c].parent = Some(p);
        }

        // x takes p's place
        self.nodes[x].ch[1 - d] = Some(p);
        self.nodes[p].parent = Some(x);
        self.nodes[x].parent = g;

        // Fix grandparent
        if let Some(g) = g {
            if self.nodes[g].ch[0] == Some(p) {
                self.nodes[g].ch[0] = Some(x);
            } else if self.nodes[g].ch[1] == Some(p) {
                self.nodes[g].ch[1] = Some(x);
            }
        }

        self.pull(p);
        self.pull(x);
    }

    #[allow(clippy::expect_used)]
    fn splay(&mut self, x: usize) {
        self.push(x);
        while !self.is_root(x) {
            let p = self.nodes[x]
                .parent
                .expect("LCT node has no parent in splay loop");
            if !self.is_root(p) {
                let _g = self.nodes[p]
                    .parent
                    .expect("LCT grandparent missing despite non-root parent");
                if self.dir(x) == self.dir(p) {
                    self.rotate(p); // zig-zig
                } else {
                    self.rotate(x); // zig-zag
                }
            }
            self.rotate(x);
        }
    }

    /// Make the preferred path from tree-root to x the right spine of x's splay tree.
    /// After expose(x), x has no right child in the splay sense, and the path from
    /// root to x is represented by left-descending from x.
    fn expose(&mut self, x: usize) {
        // Make x the rightmost of its preferred path
        self.splay(x);
        // Detach right child — the old preferred child is no longer preferred
        self.nodes[x].ch[1] = None;
        self.pull(x);

        // Walk up via path-parents, making each one x's right child
        let mut cur = x;
        while let Some(pp) = self.find_path_parent(cur) {
            self.splay(pp);
            // Detach pp's old right child
            self.nodes[pp].ch[1] = None;
            // Attach cur as pp's right child
            self.nodes[pp].ch[1] = Some(cur);
            self.nodes[cur].parent = Some(pp);
            self.pull(pp);
            cur = pp;
        }
        self.splay(x);
    }

    /// Find the path-parent of x (the node above x's preferred path in the
    /// represented tree). This is x's parent pointer when x is splay-root.
    fn find_path_parent(&self, x: usize) -> Option<usize> {
        let p = self.nodes[x].parent?;
        // If p is a splay parent (has x as child), then x isn't a splay root
        // and the path-parent is found by going up until we find a splay root.
        // But after splay(x), x IS a splay root, so its parent IS the path-parent.
        Some(p)
    }

    #[allow(dead_code)]
    fn evert(&mut self, x: usize) {
        self.expose(x);
        self.nodes[x].rev ^= true;
        self.push(x);
    }

    // ── Public API ─────────────────────────────────────────────────

    pub fn find_root(&mut self, node: usize) -> Result<usize, LctError> {
        if node >= self.nodes.len() {
            return Err(LctError::InvalidNode(node));
        }
        self.expose(node);
        let mut x = node;
        loop {
            self.push(x);
            if let Some(l) = self.nodes[x].ch[0] {
                x = l;
            } else {
                break;
            }
        }
        self.splay(x);
        Ok(x)
    }

    pub fn link(&mut self, child: usize, parent: usize) -> Result<(), LctError> {
        if child >= self.nodes.len() {
            return Err(LctError::InvalidNode(child));
        }
        if parent >= self.nodes.len() {
            return Err(LctError::InvalidNode(parent));
        }
        // child must be a root in its represented tree
        self.expose(child);
        if self.nodes[child].ch[0].is_some() {
            return Err(LctError::NotRoot { node: child });
        }
        // Simple path-parent link: set child's parent to parent
        self.nodes[child].parent = Some(parent);
        Ok(())
    }

    pub fn cut(&mut self, node: usize) -> Result<(), LctError> {
        if node >= self.nodes.len() {
            return Err(LctError::InvalidNode(node));
        }
        self.expose(node);
        match self.nodes[node].ch[0] {
            Some(l) => {
                self.push(node);
                self.nodes[l].parent = None;
                self.nodes[node].ch[0] = None;
                self.pull(node);
                Ok(())
            }
            None => Err(LctError::AlreadyRoot { node }),
        }
    }

    pub fn path_aggregate(&mut self, node: usize) -> Result<A, LctError> {
        if node >= self.nodes.len() {
            return Err(LctError::InvalidNode(node));
        }
        self.expose(node);
        Ok(self.nodes[node].agg.clone())
    }

    pub fn lca(&mut self, a: usize, b: usize) -> Result<usize, LctError> {
        if a >= self.nodes.len() {
            return Err(LctError::InvalidNode(a));
        }
        if b >= self.nodes.len() {
            return Err(LctError::InvalidNode(b));
        }
        if a == b {
            return Ok(a);
        }
        // Standard LCT LCA: expose(a), then expose(b).
        // After expose(b), if a has no parent, they're disconnected → return a.
        // Otherwise, the splay root of a's auxiliary tree has a path-parent
        // that is the LCA.
        self.expose(a);
        self.expose(b);
        // After both exposes, splay a to its root.
        self.splay(a);
        // If a has no parent at all, they're in different trees.
        if self.nodes[a].parent.is_none() {
            return Ok(a);
        }
        // a's splay tree is connected to b's via path-parent.
        // Walk from a up through splay parents to the splay root.
        // The splay root's path-parent IS the LCA.
        let mut x = a;
        while let Some(p) = self.nodes[x].parent {
            // Check if p is a splay parent (has x as child) vs path-parent
            if self.nodes[p].ch[0] != Some(x) && self.nodes[p].ch[1] != Some(x) {
                // p is a path-parent — this is the LCA
                return Ok(p);
            }
            x = p;
        }
        // x is a splay root with no path-parent — it IS the root of the tree.
        // This means a and b's LCA is the root.
        Ok(x)
    }

    pub fn connected(&mut self, a: usize, b: usize) -> Result<bool, LctError> {
        let ra = self.find_root(a)?;
        let rb = self.find_root(b)?;
        Ok(ra == rb)
    }

    pub fn get(&self, node: usize) -> Result<&V, LctError> {
        if node >= self.nodes.len() {
            return Err(LctError::InvalidNode(node));
        }
        Ok(&self.nodes[node].value)
    }

    pub fn set(&mut self, node: usize, value: V) -> Result<(), LctError> {
        if node >= self.nodes.len() {
            return Err(LctError::InvalidNode(node));
        }
        self.splay(node);
        self.nodes[node].value = value;
        self.pull(node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_tree_creates_singleton() {
        let mut lct: LinkCutTree<(), ()> = LinkCutTree::new();
        let n = lct.make_tree(());
        assert_eq!(lct.len(), 1);
        assert_eq!(lct.find_root(n).unwrap(), n);
    }

    #[test]
    fn link_two_nodes() {
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let a = lct.make_tree(0);
        let b = lct.make_tree(1);
        lct.link(b, a).unwrap();
        assert_eq!(lct.find_root(b).unwrap(), a);
    }

    #[test]
    fn cut_disconnects_child() {
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let a = lct.make_tree(0);
        let b = lct.make_tree(1);
        lct.link(b, a).unwrap();
        lct.cut(b).unwrap();
        assert_eq!(lct.find_root(b).unwrap(), b);
    }

    #[test]
    fn chain_find_root() {
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
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
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
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
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
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
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        lct.link(n1, n0).unwrap();
        lct.link(n2, n1).unwrap();
        assert_eq!(lct.lca(n0, n2).unwrap(), n0);
    }

    #[test]
    fn link_non_root_errors() {
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        let n1 = lct.make_tree(1);
        let n2 = lct.make_tree(2);
        lct.link(n1, n0).unwrap();
        assert!(matches!(lct.link(n1, n2), Err(LctError::NotRoot { .. })));
    }

    #[test]
    fn cut_root_errors() {
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        let n0 = lct.make_tree(0);
        assert!(matches!(lct.cut(n0), Err(LctError::AlreadyRoot { .. })));
    }

    #[test]
    fn reconnect_after_cut() {
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
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
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
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
        let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
        lct.make_tree(0);
        assert!(matches!(lct.find_root(99), Err(LctError::InvalidNode(99))));
    }
}
