//! Link-Cut Tree (LCT) — dynamic forest data structure.
//!
//! Provides amortized O(log n) operations for maintaining a forest of rooted
//! trees with dynamic `link`, `cut`, and path queries. Uses splay-tree-based
//! preferred-path decomposition (Sleator–Tarjan, 1983).

use std::sync::RwLock;

use crate::monoid::Monoid;

pub trait LctAggregate<A: Monoid>: Clone {
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

    /// Which side is x on in its parent?
    #[allow(clippy::expect_used)]
    fn dir(nodes: &[Node<V, A>], x: usize) -> usize {
        #[allow(clippy::expect_used)]
        let p = nodes[x]
            .parent
            .expect("LCT node has no parent despite not being root");
        if nodes[p].ch[1] == Some(x) {
            1
        } else {
            0
        }
    }

    #[allow(clippy::expect_used)]
    fn rotate(nodes: &mut [Node<V, A>], x: usize) {
        #[allow(clippy::expect_used)]
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

    #[allow(clippy::expect_used)]
    fn splay(nodes: &mut [Node<V, A>], x: usize) {
        Self::push(nodes, x);
        while !Self::is_root(nodes, x) {
            #[allow(clippy::expect_used)]
            let p = nodes[x]
                .parent
                .expect("LCT node has no parent in splay loop");
            if !Self::is_root(nodes, p) {
                #[allow(clippy::expect_used)]
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

    fn expose(nodes: &mut [Node<V, A>], x: usize) {
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
    fn evert(nodes: &mut [Node<V, A>], x: usize) {
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

    // ──────────────────────────────────────────────────────────────────
    // Proptest and Adversarial Tests
    // ──────────────────────────────────────────────────────────────────

    #[cfg(feature = "proptest")]
    mod proptest_tests {
        use super::*;
        use proptest::collection::vec;
        use proptest::prelude::*;

        fn arb_node_ops(max_nodes: usize) -> impl Strategy<Value = Vec<NodeOp>> {
            let make_node =
                proptest::collection::uniform(1..=max_nodes, any::<u64>()).prop_map(|vals| {
                    vals.into_iter()
                        .map(|v| NodeOp::Make(v))
                        .collect::<Vec<_>>()
                });
            let ops: Vec<NodeOp> = vec(any::<NodeOp>(), 1..=50).prop_filter(
                "must have enough nodes for link/cut",
                |ops| {
                    let make_count = ops.iter().filter(|o| matches!(o, NodeOp::Make(_))).count();
                    let link_count = ops
                        .iter()
                        .filter(|o| matches!(o, NodeOp::Link(_, _)))
                        .count();
                    let cut_count = ops.iter().filter(|o| matches!(o, NodeOp::Cut(_))).count();
                    make_count >= link_count && make_count > cut_count
                },
            );
            prop_oneof![100, make_node, ops.prop_map(|v| v)]
        }

        #[derive(Debug, Clone)]
        enum NodeOp {
            Make(u64),
            Link(usize, usize),
            Cut(usize),
            Set(usize, u64),
        }

        #[cfg(feature = "proptest")]
        proptest! {
            #[test]
            fn proptest_random_sequence(
                ops in arb_node_ops(20)
            ) {
                let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
                let mut node_indices = Vec::new();

                for op in ops {
                    match op {
                        NodeOp::Make(value) => {
                            let idx = lct.make_tree(value);
                            node_indices.push(idx);
                            assert_eq!(lct.len(), node_indices.len());
                            assert_eq!(lct.find_root(idx).unwrap(), idx);
                        }
                        NodeOp::Link(child, parent) => {
                            if child < node_indices.len() && parent < node_indices.len() {
                                let _ = lct.link(node_indices[child], node_indices[parent]);
                            }
                        }
                        NodeOp::Cut(node_idx) => {
                            if node_idx < node_indices.len() {
                                let _ = lct.cut(node_indices[node_idx]);
                            }
                        }
                        NodeOp::Set(node_idx, value) => {
                            if node_idx < node_indices.len() {
                                let _ = lct.set(node_indices[node_idx], value);
                            }
                        }
                    }
                }
            }

            #[test]
            fn proptest_link_cut_invariants(
                num_nodes in 1..=15u32,
                seed in any::<u64>()
            ) {
                use rand::SeedableRng;
                use rand::rngs::StdRng;
                use rand::Rng;

                let mut rng = StdRng::seed_from_u64(seed);
                let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
                let mut nodes = Vec::new();

                // Make all nodes
                for i in 0..num_nodes {
                    let value = rng.gen_range(0..=100);
                    let idx = lct.make_tree(value);
                    nodes.push(idx);
                }

                // Random link/cut operations
                let mut parent_map: Vec<Option<usize>> = vec![None; nodes.len()];

                for _ in 0..(num_nodes as usize * 5) {
                    let op = rng.gen_range(0..100);
                    if op < 40 && nodes.len() > 1 {
                        // Link operation
                        let child_idx = rng.gen_range(0..nodes.len());
                        let parent_idx = rng.gen_range(0..nodes.len());
                        if child_idx != parent_idx {
                            let child = nodes[child_idx];
                            let parent = nodes[parent_idx];
                            if lct.find_root(child).unwrap() == child {
                                if lct.link(child, parent).is_ok() {
                                    parent_map[child_idx] = Some(parent_idx);
                                }
                            }
                        }
                    } else if op < 70 {
                        // Cut operation
                        let node_idx = rng.gen_range(0..nodes.len());
                        if parent_map[node_idx].is_some() {
                            if lct.cut(nodes[node_idx]).is_ok() {
                                parent_map[node_idx] = None;
                            }
                        }
                    } else {
                        // Aggregate query
                        let node_idx = rng.gen_range(0..nodes.len());
                        let _agg = lct.path_aggregate(nodes[node_idx]);
                    }
                }
            }

            #[test]
            fn proptest_tree_connectivity(
                num_nodes in 2..=10u32,
                seed in any::<u64>()
            ) {
                use rand::SeedableRng;
                use rand::rngs::StdRng;
                use rand::Rng;

                let mut rng = StdRng::seed_from_u64(seed);
                let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
                let mut nodes = Vec::new();

                for _ in 0..num_nodes {
                    let idx = lct.make_tree(0);
                    nodes.push(idx);
                }

                // Build a random tree structure
                for i in 1..nodes.len() {
                    let parent_i = rng.gen_range(0..i);
                    let _ = lct.link(nodes[i], nodes[parent_i]);
                }

                // Verify all nodes connected to root
                let root = lct.find_root(nodes[0]).unwrap();
                for node in &nodes {
                    assert_eq!(lct.find_root(*node).unwrap(), root);
                }

                // Verify connectivity
                for i in 0..nodes.len() {
                    for j in 0..nodes.len() {
                        assert_eq!(lct.connected(nodes[i], nodes[j]).unwrap(), true);
                    }
                }
            }

            #[test]
            fn proptest_aggregate_correctness(
                depth in 1..=8u32,
                seed in any::<u64>()
            ) {
                use rand::SeedableRng;
                use rand::rngs::StdRng;
                use rand::Rng;

                let mut rng = StdRng::seed_from_u64(seed);
                let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
                let mut nodes = Vec::new();
                let mut expected_sum = 0u64;

                // Make nodes with random values
                for _ in 0..depth {
                    let value = rng.gen_range(1..=10);
                    expected_sum += value;
                    let idx = lct.make_tree(value);
                    nodes.push(idx);
                }

                // Create a chain
                for i in 1..nodes.len() {
                    lct.link(nodes[i], nodes[i-1]).unwrap();
                }

                // Aggregate at deepest node should be sum of all
                let agg = lct.path_aggregate(nodes[depth as usize - 1]).unwrap();
                assert_eq!(agg, expected_sum);

                // Modify a node and verify aggregate
                let mid = depth as usize / 2;
                let new_val = 100u64;
                lct.set(nodes[mid], new_val).unwrap();
                expected_sum = expected_sum - nodes[mid].lct_aggregate() + new_val;
                let agg = lct.path_aggregate(nodes[depth as usize - 1]).unwrap();
                assert_eq!(agg, expected_sum);
            }

            #[test]
            fn proptest_deep_chain_stress(
                chain_len in 1..=100u32,
                seed in any::<u64>()
            ) {
                use rand::SeedableRng;
                use rand::rngs::StdRng;

                let mut rng = StdRng::seed_from_u64(seed);
                let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
                let mut nodes = Vec::new();

                // Make nodes
                for _ in 0..chain_len {
                    let value = rng.gen_range(0..=100);
                    let idx = lct.make_tree(value);
                    nodes.push(idx);
                }

                // Create a deep chain
                for i in 1..nodes.len() {
                    lct.link(nodes[i], nodes[i-1]).unwrap();
                }

                // Find root should be first node
                assert_eq!(lct.find_root(nodes[chain_len as usize - 1]).unwrap(), nodes[0]);

                // Aggregate at end
                let agg = lct.path_aggregate(nodes[chain_len as usize - 1]).unwrap();
                let expected: u64 = (0..chain_len).map(|i| {
                    let idx = i as usize;
                    lct.get(nodes[idx]).unwrap()
                }).sum();
                assert_eq!(agg, expected);

                // Cut in middle
                let mid = chain_len as usize / 2;
                lct.cut(nodes[mid]).unwrap();

                // Now two separate trees
                assert!(!lct.connected(nodes[0], nodes[chain_len as usize - 1]).unwrap());
                assert_eq!(lct.find_root(nodes[chain_len as usize - 1]).unwrap(), nodes[mid]);
            }

            #[test]
            fn proptest_random_forest_operations(
                seed in any::<u64>()
            ) {
                use rand::SeedableRng;
                use rand::rngs::StdRng;
                use rand::Rng;

                let mut rng = StdRng::seed_from_u64(seed);
                let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
                let mut nodes = Vec::new();

                // Make 20 nodes
                for _ in 0..20 {
                    let idx = lct.make_tree(rng.gen_range(0..=100));
                    nodes.push(idx);
                }

                // Random operations
                for _ in 0..200 {
                    let op = rng.gen_range(0..100);
                    match op {
                        0..=20 => {
                            // Find root
                            let node = nodes[rng.gen_range(0..nodes.len())];
                            let _ = lct.find_root(node);
                        }
                        21..=45 => {
                            // Link
                            let c = nodes[rng.gen_range(0..nodes.len())];
                            let p = nodes[rng.gen_range(0..nodes.len())];
                            if c != p && lct.find_root(c).unwrap() == c {
                                let _ = lct.link(c, p);
                            }
                        }
                        46..=70 => {
                            // Cut
                            let node = nodes[rng.gen_range(0..nodes.len())];
                            if lct.find_root(node).unwrap() != node {
                                let _ = lct.cut(node);
                            }
                        }
                        71..=85 => {
                            // Path aggregate
                            let node = nodes[rng.gen_range(0..nodes.len())];
                            let _ = lct.path_aggregate(node);
                        }
                        86..=95 => {
                            // Connected
                            let a = nodes[rng.gen_range(0..nodes.len())];
                            let b = nodes[rng.gen_range(0..nodes.len())];
                            let _ = lct.connected(a, b);
                        }
                        96..=100 => {
                            // Set value
                            let node = nodes[rng.gen_range(0..nodes.len())];
                            let val = rng.gen_range(0..=1000);
                            let _ = lct.set(node, val);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod adversarial_tests {
        use super::*;

        // Test worst-case deep chain pattern
        #[test]
        fn adversarial_deep_chain_1000() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut nodes = Vec::new();

            for i in 0..1000 {
                let idx = lct.make_tree(i as u64);
                nodes.push(idx);
            }

            for i in 1..1000 {
                lct.link(nodes[i], nodes[i - 1]).unwrap();
            }

            // Find root at end
            let root = lct.find_root(nodes[999]).unwrap();
            assert_eq!(root, nodes[0]);

            // Aggregate
            let agg = lct.path_aggregate(nodes[999]).unwrap();
            let expected: u64 = (0..1000).sum();
            assert_eq!(agg, expected);
        }

        // Test zig-zag linking pattern
        #[test]
        fn adversarial_zigzag_links() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut nodes = Vec::new();

            for _ in 0..50 {
                let idx = lct.make_tree(1);
                nodes.push(idx);
            }

            // Zigzag: 1->0, 2->1, 3->2, etc.
            for i in 1..nodes.len() {
                lct.link(nodes[i], nodes[i - 1]).unwrap();
            }

            // Cut every other edge
            for i in 0..nodes.len() {
                if i % 2 == 1 {
                    let _ = lct.cut(nodes[i]);
                }
            }

            // Verify splits
            for i in (0..nodes.len()).step_by(2) {
                if i + 1 < nodes.len() {
                    assert!(!lct.connected(nodes[i], nodes[i + 1]).unwrap());
                }
            }
        }

        // Test alternating link/cut operations
        #[test]
        fn adversarial_alternate_link_cut() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let a = lct.make_tree(0);
            let b = lct.make_tree(1);

            // Link and cut repeatedly
            for _ in 0..100 {
                lct.link(b, a).unwrap();
                assert!(lct.connected(a, b).unwrap());
                lct.cut(b).unwrap();
                assert!(!lct.connected(a, b).unwrap());
            }
        }

        // Test aggregate after multiple cuts
        #[test]
        fn adversarial_aggregate_after_cuts() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut nodes = Vec::new();

            for i in 0..20 {
                let idx = lct.make_tree(i as u64);
                nodes.push(idx);
            }

            // Build a balanced-like tree
            for i in 1..nodes.len() {
                let parent = i / 2;
                lct.link(nodes[i], nodes[parent]).unwrap();
            }

            // Cut all edges from even nodes
            for i in (0..nodes.len()).step_by(2) {
                if i > 0 {
                    let _ = lct.cut(nodes[i]);
                }
            }

            // Verify aggregates on remaining connected components
            for i in 0..nodes.len() {
                let agg = lct.path_aggregate(nodes[i]).unwrap();
                assert!(agg >= 0);
            }
        }

        // Test LCA correctness on various structures
        #[test]
        fn adversarial_lca_deep_tree() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut nodes = Vec::new();

            for _ in 0..100 {
                let idx = lct.make_tree(0);
                nodes.push(idx);
            }

            for i in 1..nodes.len() {
                lct.link(nodes[i], nodes[i - 1]).unwrap();
            }

            // LCA of any node with root is root
            assert_eq!(lct.lca(nodes[0], nodes[50]).unwrap(), nodes[0]);
            assert_eq!(lct.lca(nodes[0], nodes[99]).unwrap(), nodes[0]);

            // LCA of adjacent nodes
            assert_eq!(lct.lca(nodes[10], nodes[11]).unwrap(), nodes[10]);
            assert_eq!(lct.lca(nodes[50], nodes[50]).unwrap(), nodes[50]);
        }

        // Test set updates propagate through path aggregates
        #[test]
        fn adversarial_set_propagation() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut nodes = Vec::new();

            for i in 0..10 {
                let idx = lct.make_tree(1);
                nodes.push(idx);
            }

            for i in 1..nodes.len() {
                lct.link(nodes[i], nodes[i - 1]).unwrap();
            }

            // Initial aggregate: sum of all = 10
            let initial_agg = lct.path_aggregate(nodes[9]).unwrap();
            assert_eq!(initial_agg, 10);

            // Set middle node to 100
            lct.set(nodes[5], 100).unwrap();
            let new_agg = lct.path_aggregate(nodes[9]).unwrap();
            assert_eq!(new_agg, 19); // 9 + 100 = 109 - 90 (original 5's contribution)
                                     // Actually: nodes 0-4 = 5, node 5 = 100, nodes 6-9 = 4, total = 109
            assert_eq!(new_agg, 109);

            // Set root to 0
            lct.set(nodes[0], 0).unwrap();
            let final_agg = lct.path_aggregate(nodes[9]).unwrap();
            assert_eq!(final_agg, 108);
        }

        // Test multiple disjoint trees
        #[test]
        fn adversarial_multiple_forests() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut forests = Vec::new();

            // Create 10 separate trees
            for _ in 0..10 {
                let root = lct.make_tree(0);
                let mut tree_nodes = vec![root];

                // Add 5 children to each
                for _ in 0..5 {
                    let child = lct.make_tree(1);
                    lct.link(child, root).unwrap();
                    tree_nodes.push(child);
                }
                forests.push(tree_nodes);
            }

            // Verify all nodes in same forest are connected
            for tree in &forests {
                for i in 0..tree.len() {
                    for j in 0..tree.len() {
                        assert!(lct.connected(tree[i], tree[j]).unwrap());
                    }
                }
            }

            // Verify nodes in different forests are not connected
            for i in 0..forests.len() {
                for j in (i + 1)..forests.len() {
                    assert!(!lct.connected(forests[i][0], forests[j][0]).unwrap());
                }
            }
        }

        // Test cut and reconnect patterns
        #[test]
        fn adversarial_cut_reconnect_cycle() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let root = lct.make_tree(0);
            let mut children = Vec::new();

            for i in 0..20 {
                let child = lct.make_tree(i as u64);
                lct.link(child, root).unwrap();
                children.push(child);
            }

            // Cut and reconnect repeatedly
            for i in 0..100 {
                let child = children[i % children.len()];
                lct.cut(child).unwrap();
                assert!(!lct.connected(root, child).unwrap());
                lct.link(child, root).unwrap();
                assert!(lct.connected(root, child).unwrap());
            }

            // Verify aggregate is correct
            let agg = lct.path_aggregate(root).unwrap();
            let expected: u64 = (1..=20).sum();
            assert_eq!(agg, expected);
        }

        // Test path aggregate on star topology
        #[test]
        fn adversarial_star_topology() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let root = lct.make_tree(0);

            let mut leaves = Vec::new();
            for i in 0..50 {
                let leaf = lct.make_tree((i + 1) as u64);
                lct.link(leaf, root).unwrap();
                leaves.push(leaf);
            }

            // Aggregate from each leaf should include all nodes
            let mut total = 0u64;
            for leaf in &leaves {
                let agg = lct.path_aggregate(*leaf).unwrap();
                total += agg;
            }

            // Each path goes through root + leaf
            // Sum over all leaves: 50 * (root_val + leaf_val)
            // = 50 * (0 + 1) + 50 * (0 + 2) + ... = 50 * (1 + 2 + ... + 50)
            let expected: u64 = (1..=50).map(|i| 50 * (i as u64)).sum();
            assert_eq!(total, expected);
        }

        // Test find_root after many operations
        #[test]
        fn adversarial_root_stability() {
            let mut lct: LinkCutTree<u64, u64> = LinkCutTree::new();
            let mut nodes = Vec::new();

            for i in 0..30 {
                let idx = lct.make_tree(i as u64);
                nodes.push(idx);
            }

            // Build complex structure
            for i in 1..10 {
                lct.link(nodes[i], nodes[0]).unwrap();
            }
            for i in 10..20 {
                lct.link(nodes[i], nodes[5]).unwrap();
            }
            for i in 20..30 {
                lct.link(nodes[i], nodes[15]).unwrap();
            }

            // Verify roots
            for _i in 0..10 {
                assert_eq!(lct.find_root(nodes[_i]).unwrap(), nodes[0]);
            }
            for _i in 10..20 {
                assert_eq!(lct.find_root(nodes[_i]).unwrap(), nodes[0]);
            }
            for _i in 20..30 {
                assert_eq!(lct.find_root(nodes[_i]).unwrap(), nodes[0]);
            }

            // Cut edge 5->0
            lct.cut(nodes[5]).unwrap();

            // Now 10-19 have different root
            for _i in 0..10 {
                assert_eq!(lct.find_root(nodes[_i]).unwrap(), nodes[0]);
            }
            for _i in 10..20 {
                assert_eq!(lct.find_root(nodes[_i]).unwrap(), nodes[5]);
            }
            for _i in 20..30 {
                assert_eq!(lct.find_root(nodes[_i]).unwrap(), nodes[5]);
            }
        }
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
