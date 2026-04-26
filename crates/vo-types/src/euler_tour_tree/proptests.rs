use super::tree::EulerTourTree;
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
                            for &child in ett.nodes.get(n).map(|n| &n.children).unwrap_or(&vec![]) {
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
