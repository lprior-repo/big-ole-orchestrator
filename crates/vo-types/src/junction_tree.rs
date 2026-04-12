//! Junction tree (Join tree): tree decomposition data structure.
//!
//! A junction tree represents a tree decomposition of a graph, where each node
//! (clique) contains a set of vertices. It maintains the Running Intersection
//! Property (RIP): if a vertex appears in two cliques, it appears in all cliques
//! on the unique path between them.
//!
//! This structure is primarily used for:
//! - Exact inference in probabilistic graphical models (junction tree algorithm)
//! - Processing queries on graphs with bounded treewidth
//! - Constraint satisfaction algorithms

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JunctionTree {
    nodes: Vec<Clique>,
    adjacency: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clique {
    pub id: usize,
    pub vertices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JunctionTreeError {
    #[error("clique not found: {0}")]
    CliqueNotFound(usize),

    #[error("vertex not found in any clique: {0}")]
    VertexNotFound(usize),

    #[error("empty tree cannot be built from zero cliques")]
    EmptyInput,

    #[error("cliques must form a tree (exactly n-1 edges for n cliques)")]
    NotATree { edges: usize, cliques: usize },

    #[error("running intersection property violated: vertex {vertex} appears in disconnected cliques {c1} and {c2}")]
    RipViolation { vertex: usize, c1: usize, c2: usize },

    #[error("clique id mismatch: expected {expected}, got {actual}")]
    CliqueIdMismatch { expected: usize, actual: usize },

    #[error("duplicate clique id: {0}")]
    DuplicateCliqueId(usize),
}

impl JunctionTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            adjacency: Vec::new(),
        }
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(n),
            adjacency: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn add_clique(&mut self, clique: Clique) -> usize {
        let id = self.nodes.len();
        let mut c = clique;
        c.id = id;
        self.nodes.push(c);
        self.adjacency.push(Vec::new());
        id
    }

    pub fn add_edge(&mut self, a: usize, b: usize) -> Result<(), JunctionTreeError> {
        if a >= self.nodes.len() {
            return Err(JunctionTreeError::CliqueNotFound(a));
        }
        if b >= self.nodes.len() {
            return Err(JunctionTreeError::CliqueNotFound(b));
        }
        if a == b {
            return Ok(());
        }
        if !self.adjacency[a].contains(&b) {
            self.adjacency[a].push(b);
        }
        if !self.adjacency[b].contains(&a) {
            self.adjacency[b].push(a);
        }
        Ok(())
    }

    pub fn get_clique(&self, id: usize) -> Result<&Clique, JunctionTreeError> {
        self.nodes
            .get(id)
            .ok_or(JunctionTreeError::CliqueNotFound(id))
    }

    pub fn get_cliques(&self) -> &[Clique] {
        &self.nodes
    }

    pub fn neighbors(&self, id: usize) -> Result<&[usize], JunctionTreeError> {
        self.adjacency
            .get(id)
            .ok_or(JunctionTreeError::CliqueNotFound(id))
            .map(Vec::as_slice)
    }

    pub fn contains_vertex(&self, vertex: usize) -> bool {
        self.nodes.iter().any(|c| c.vertices.contains(&vertex))
    }

    pub fn find_cliques_with_vertex(&self, vertex: usize) -> Vec<usize> {
        self.nodes
            .iter()
            .filter(|c| c.vertices.contains(&vertex))
            .map(|c| c.id)
            .collect()
    }

    pub fn separator(&self, a: usize, b: usize) -> Result<Vec<usize>, JunctionTreeError> {
        let clique_a = self.get_clique(a)?;
        let clique_b = self.get_clique(b)?;
        Ok(clique_a
            .vertices
            .iter()
            .filter(|v| clique_b.vertices.contains(v))
            .copied()
            .collect())
    }

    pub fn path_between(&self, a: usize, b: usize) -> Result<Vec<usize>, JunctionTreeError> {
        if a >= self.nodes.len() {
            return Err(JunctionTreeError::CliqueNotFound(a));
        }
        if b >= self.nodes.len() {
            return Err(JunctionTreeError::CliqueNotFound(b));
        }
        if a == b {
            return Ok(vec![a]);
        }

        let mut visited = vec![false; self.nodes.len()];
        let mut parent = vec![None; self.nodes.len()];
        let mut queue = vec![a];
        visited[a] = true;

        while let Some(current) = queue.pop() {
            if current == b {
                let mut path = vec![b];
                let mut node = b;
                while let Some(p) = parent[node] {
                    path.push(p);
                    node = p;
                }
                path.reverse();
                return Ok(path);
            }

            for &neighbor in &self.adjacency[current] {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    parent[neighbor] = Some(current);
                    queue.push(neighbor);
                }
            }
        }

        Ok(vec![])
    }

    pub fn verify_rip(&self) -> Result<(), JunctionTreeError> {
        let vertex_to_cliques: HashMap<usize, Vec<usize>> = self
            .nodes
            .iter()
            .flat_map(|c| c.vertices.iter().map(|&v| (v, c.id)).collect::<Vec<_>>())
            .fold(HashMap::new(), |mut acc, (v, c)| {
                acc.entry(v).or_default().push(c);
                acc
            });

        for (vertex, clique_ids) in vertex_to_cliques {
            if clique_ids.len() <= 1 {
                continue;
            }

            for i in 0..clique_ids.len() {
                for j in (i + 1)..clique_ids.len() {
                    let c1 = clique_ids[i];
                    let c2 = clique_ids[j];
                    let path = self.path_between(c1, c2)?;

                    if path.is_empty() {
                        return Err(JunctionTreeError::RipViolation { vertex, c1, c2 });
                    }

                    for &clique_id in &path {
                        let clique = &self.nodes[clique_id];
                        if !clique.vertices.contains(&vertex) {
                            return Err(JunctionTreeError::RipViolation { vertex, c1, c2 });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn is_tree(&self) -> bool {
        if self.nodes.is_empty() {
            return true;
        }
        let edge_count: usize = self.adjacency.iter().map(|v| v.len()).sum::<usize>() / 2;
        edge_count == self.nodes.len() - 1
    }

    pub fn build_from_cliques_and_edges(
        cliques: Vec<Vec<usize>>,
        edges: Vec<(usize, usize)>,
    ) -> Result<Self, JunctionTreeError> {
        if cliques.is_empty() {
            return Err(JunctionTreeError::EmptyInput);
        }

        let mut tree = Self::with_capacity(cliques.len());

        for (id, vertices) in cliques.into_iter().enumerate() {
            let clique = Clique { id, vertices };
            tree.add_clique(clique);
        }

        for (a, b) in edges {
            tree.add_edge(a, b)?;
        }

        if !tree.is_tree() {
            let edge_count: usize = tree.adjacency.iter().map(|v| v.len()).sum::<usize>() / 2;
            return Err(JunctionTreeError::NotATree {
                edges: edge_count,
                cliques: tree.nodes.len(),
            });
        }

        tree.verify_rip()?;

        Ok(tree)
    }

    pub fn moralized_graph(&self) -> (Vec<HashSet<usize>>, Vec<(usize, usize)>) {
        let n = self.nodes.len();
        let mut edges: HashSet<(usize, usize)> = HashSet::new();
        let mut clique_vertices: Vec<HashSet<usize>> = self
            .nodes
            .iter()
            .map(|c| c.vertices.iter().copied().collect())
            .collect();

        for i in 0..n {
            for j in (i + 1)..n {
                let sep = self.separator(i, j).unwrap_or_default();
                if !sep.is_empty() {
                    edges.insert((i, j));
                }
            }
        }

        let connections: Vec<(usize, usize)> = edges.into_iter().collect();
        (clique_vertices, connections)
    }

    pub fn induced_width(&self) -> usize {
        self.nodes
            .iter()
            .map(|c| c.vertices.len())
            .max()
            .unwrap_or(0)
            .saturating_sub(1)
    }
}

impl Default for JunctionTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_is_empty() {
        let tree: JunctionTree = JunctionTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn add_cliques_increments_len() {
        let mut tree = JunctionTree::new();
        assert_eq!(
            tree.add_clique(Clique {
                id: 0,
                vertices: vec![1, 2]
            }),
            0
        );
        assert_eq!(
            tree.add_clique(Clique {
                id: 0,
                vertices: vec![2, 3]
            }),
            1
        );
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn add_edge_connects_cliques() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3],
        });
        tree.add_edge(0, 1).unwrap();

        assert_eq!(tree.neighbors(0).unwrap(), &[1]);
        assert_eq!(tree.neighbors(1).unwrap(), &[0]);
    }

    #[test]
    fn edge_to_nonexistent_clique_errors() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        assert!(matches!(
            tree.add_edge(0, 99),
            Err(JunctionTreeError::CliqueNotFound(99))
        ));
    }

    #[test]
    fn get_clique_returns_clique() {
        let mut tree = JunctionTree::new();
        let id = tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        let clique = tree.get_clique(id).unwrap();
        assert_eq!(clique.vertices, vec![1, 2, 3]);
    }

    #[test]
    fn get_nonexistent_clique_errors() {
        let tree: JunctionTree = JunctionTree::new();
        assert!(matches!(
            tree.get_clique(0),
            Err(JunctionTreeError::CliqueNotFound(0))
        ));
    }

    #[test]
    fn find_cliques_with_vertex() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![4, 5],
        });
        tree.add_edge(0, 1).unwrap();
        tree.add_edge(1, 2).unwrap();

        let with_2 = tree.find_cliques_with_vertex(2);
        assert_eq!(with_2, vec![0, 1]);

        let with_5 = tree.find_cliques_with_vertex(5);
        assert_eq!(with_5, vec![2]);
    }

    #[test]
    fn separator_returns_common_vertices() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3, 4],
        });
        tree.add_edge(0, 1).unwrap();

        let sep = tree.separator(0, 1).unwrap();
        assert!(sep.contains(&2));
        assert!(sep.contains(&3));
        assert_eq!(sep.len(), 2);
    }

    #[test]
    fn path_between_connected_cliques() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 4],
        });
        tree.add_edge(0, 1).unwrap();
        tree.add_edge(1, 2).unwrap();

        let path = tree.path_between(0, 2).unwrap();
        assert_eq!(path, vec![0, 1, 2]);
    }

    #[test]
    fn path_between_same_clique() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });

        let path = tree.path_between(0, 0).unwrap();
        assert_eq!(path, vec![0]);
    }

    #[test]
    fn is_tree_returns_true_for_valid_tree() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 4],
        });
        tree.add_edge(0, 1).unwrap();
        tree.add_edge(1, 2).unwrap();

        assert!(tree.is_tree());
    }

    #[test]
    fn is_tree_returns_false_for_forest() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 4],
        });

        assert!(!tree.is_tree());
    }

    #[test]
    fn is_tree_returns_false_for_cycle() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 1],
        });
        tree.add_edge(0, 1).unwrap();
        tree.add_edge(1, 2).unwrap();
        tree.add_edge(2, 0).unwrap();

        assert!(!tree.is_tree());
    }

    #[test]
    fn verify_rip_accepts_valid_tree() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3, 4],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 4, 5],
        });
        tree.add_edge(0, 1).unwrap();
        tree.add_edge(1, 2).unwrap();

        assert!(tree.verify_rip().is_ok());
    }

    #[test]
    fn induced_width_calculated_correctly() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3, 4],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 4, 5],
        });
        tree.add_edge(0, 1).unwrap();
        tree.add_edge(1, 2).unwrap();

        assert_eq!(tree.induced_width(), 2);
    }

    #[test]
    fn build_from_cliques_and_edges() {
        let cliques = vec![vec![1, 2, 3], vec![2, 3, 4], vec![3, 4, 5]];
        let edges = vec![(0, 1), (1, 2)];

        let tree = JunctionTree::build_from_cliques_and_edges(cliques, edges).unwrap();
        assert_eq!(tree.len(), 3);
        assert!(tree.is_tree());
    }

    #[test]
    fn build_from_empty_cliques_errors() {
        let result = JunctionTree::build_from_cliques_and_edges(vec![], vec![]);
        assert!(matches!(result, Err(JunctionTreeError::EmptyInput)));
    }

    #[test]
    fn build_from_non_tree_errors() {
        let cliques = vec![vec![1, 2], vec![2, 3], vec![3, 1]];
        let edges = vec![(0, 1), (1, 2), (2, 0)];

        let result = JunctionTree::build_from_cliques_and_edges(cliques, edges);
        assert!(matches!(result, Err(JunctionTreeError::NotATree { .. })));
    }

    #[test]
    fn moralized_graph_contains_cliques_and_edges() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3, 4],
        });
        tree.add_edge(0, 1).unwrap();

        let (cliques, edges) = tree.moralized_graph();
        assert_eq!(cliques.len(), 2);
        assert!(!edges.is_empty());
    }

    #[test]
    fn serde_roundtrip() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![2, 3, 4],
        });
        tree.add_edge(0, 1).unwrap();

        let json = serde_json::to_string(&tree).unwrap();
        let back: JunctionTree = serde_json::from_str(&json).unwrap();

        assert_eq!(back.len(), 2);
        assert!(back.verify_rip().is_ok());
    }

    #[test]
    fn contains_vertex() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2, 3],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![4, 5, 6],
        });

        assert!(tree.contains_vertex(1));
        assert!(tree.contains_vertex(5));
        assert!(!tree.contains_vertex(99));
    }

    #[test]
    fn path_between_disconnected_cliques() {
        let mut tree = JunctionTree::new();
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![1, 2],
        });
        tree.add_clique(Clique {
            id: 0,
            vertices: vec![3, 4],
        });

        let path = tree.path_between(0, 1).unwrap();
        assert!(path.is_empty());
    }
}
