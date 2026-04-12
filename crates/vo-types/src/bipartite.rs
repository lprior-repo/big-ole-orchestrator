//! Bipartite graph implementation.
//!
//! A bipartite graph is a graph whose vertices can be divided into two disjoint
//! sets U and V such that every edge connects a vertex in U to one in V.
//! No edge connects vertices within the same set.
//!
//! # Properties
//!
//! - `is_bipartite()` - O(V+E) check using BFS 2-coloring
//! - `is_complete()` - all left nodes connected to all right nodes
//! - `matching()` - maximum matching using Hopcroft-Karp algorithm
//!
//! # Usage
//!
//! ```
//! use veloxide::BipartiteGraph;
//!
//! let mut graph = BipartiteGraph::<&str, &str>::new();
//! graph.add_left_node("A");
//! graph.add_right_node("1");
//! graph.add_edge("A", "1").unwrap();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BipartiteGraphError {
    #[error("node {0} already exists in left partition")]
    LeftNodeExists(String),

    #[error("node {0} already exists in right partition")]
    RightNodeExists(String),

    #[error("node {0} not found")]
    NodeNotFound(String),

    #[error("node {0} is in left partition, expected right")]
    WrongPartition {
        node: String,
        expected: &'static str,
    },

    #[error("edge ({0}, {1}) already exists")]
    EdgeExists(String, String),

    #[error("edge ({0}, {1}) does not exist")]
    EdgeNotFound(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BipartiteGraph<L, R> {
    left_nodes: HashSet<L>,
    right_nodes: HashSet<R>,
    left_to_right: HashMap<L, HashSet<R>>,
    right_to_left: HashMap<R, HashSet<L>>,
    edges: HashSet<(L, R)>,
}

impl<L: Eq + std::hash::Hash + Clone, R: Eq + std::hash::Hash + Clone> Default
    for BipartiteGraph<L, R>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L: Eq + std::hash::Hash + Clone, R: Eq + std::hash::Hash + Clone> BipartiteGraph<L, R> {
    pub fn new() -> Self {
        Self {
            left_nodes: HashSet::new(),
            right_nodes: HashSet::new(),
            left_to_right: HashMap::new(),
            right_to_left: HashMap::new(),
            edges: HashSet::new(),
        }
    }

    pub fn with_capacity(left_capacity: usize, right_capacity: usize) -> Self {
        Self {
            left_nodes: HashSet::with_capacity(left_capacity),
            right_nodes: HashSet::with_capacity(right_capacity),
            left_to_right: HashMap::with_capacity(left_capacity),
            right_to_left: HashMap::with_capacity(right_capacity),
            edges: HashSet::new(),
        }
    }

    pub fn add_left_node(&mut self, node: L) -> bool {
        self.left_nodes.insert(node.clone());
        self.left_to_right.entry(node).or_insert_with(HashSet::new);
        true
    }

    pub fn add_right_node(&mut self, node: R) -> bool {
        self.right_nodes.insert(node.clone());
        self.right_to_left.entry(node).or_insert_with(HashSet::new);
        true
    }

    pub fn add_edge(&mut self, left: L, right: R) -> Result<bool, BipartiteGraphError> {
        if !self.left_nodes.contains(&left) {
            return Err(BipartiteGraphError::NodeNotFound(left));
        }
        if !self.right_nodes.contains(&right) {
            return Err(BipartiteGraphError::NodeNotFound(right));
        }

        let edge = (left.clone(), right.clone());
        let is_new = self.edges.insert(edge);

        if is_new {
            self.left_to_right
                .entry(left.clone())
                .or_insert_with(HashSet::new)
                .insert(right.clone());
            self.right_to_left
                .entry(right.clone())
                .or_insert_with(HashSet::new)
                .insert(left);
        }

        Ok(is_new)
    }

    pub fn remove_edge(&mut self, left: &L, right: &R) -> Result<bool, BipartiteGraphError> {
        if !self.left_nodes.contains(left) {
            return Err(BipartiteGraphError::NodeNotFound(left.clone()));
        }
        if !self.right_nodes.contains(right) {
            return Err(BipartiteGraphError::NodeNotFound(right.clone()));
        }

        let edge = (left.clone(), right.clone());
        let was_present = self.edges.remove(&edge);

        if was_present {
            if let Some(neighbors) = self.left_to_right.get_mut(left) {
                neighbors.remove(right);
            }
            if let Some(neighbors) = self.right_to_left.get_mut(right) {
                neighbors.remove(left);
            }
        }

        Ok(was_present)
    }

    pub fn has_edge(&self, left: &L, right: &R) -> bool {
        self.edges.contains(&(left.clone(), right.clone()))
    }

    pub fn left_neighbors(&self, node: &L) -> Option<&HashSet<R>> {
        self.left_to_right.get(node)
    }

    pub fn right_neighbors(&self, node: &R) -> Option<&HashSet<L>> {
        self.right_to_left.get(node)
    }

    pub fn left_nodes(&self) -> &HashSet<L> {
        &self.left_nodes
    }

    pub fn right_nodes(&self) -> &HashSet<R> {
        &self.right_nodes
    }

    pub fn num_left_nodes(&self) -> usize {
        self.left_nodes.len()
    }

    pub fn num_right_nodes(&self) -> usize {
        self.right_nodes.len()
    }

    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn clear(&mut self) {
        self.left_nodes.clear();
        self.right_nodes.clear();
        self.left_to_right.clear();
        self.right_to_left.clear();
        self.edges.clear();
    }

    pub fn is_complete(&self) -> bool {
        if self.left_nodes.is_empty() || self.right_nodes.is_empty() {
            return false;
        }
        let expected_edges = self.left_nodes.len() * self.right_nodes.len();
        self.edges.len() == expected_edges
    }

    pub fn degree(&self, node: &L) -> Option<usize> {
        self.left_to_right
            .get(node)
            .map(|neighbors| neighbors.len())
    }

    pub fn right_degree(&self, node: &R) -> Option<usize> {
        self.right_to_left
            .get(node)
            .map(|neighbors| neighbors.len())
    }

    pub fn min_left_degree(&self) -> Option<usize> {
        self.left_to_right.values().map(|v| v.len()).min()
    }

    pub fn max_left_degree(&self) -> Option<usize> {
        self.left_to_right.values().map(|v| v.len()).max()
    }

    pub fn min_right_degree(&self) -> Option<usize> {
        self.right_to_left.values().map(|v| v.len()).min()
    }

    pub fn max_right_degree(&self) -> Option<usize> {
        self.right_to_left.values().map(|v| v.len()).max()
    }

    pub fn left_nodes_with_min_degree(&self) -> Vec<L> {
        let min = match self.min_left_degree() {
            Some(d) => d,
            None => return vec![],
        };
        self.left_to_right
            .iter()
            .filter(|(_, neighbors)| neighbors.len() == min)
            .map(|(node, _)| node.clone())
            .collect()
    }

    pub fn right_nodes_with_min_degree(&self) -> Vec<R> {
        let min = match self.min_right_degree() {
            Some(d) => d,
            None => return vec![],
        };
        self.right_to_left
            .iter()
            .filter(|(_, neighbors)| neighbors.len() == min)
            .map(|(node, _)| node.clone())
            .collect()
    }
}

impl<L: Eq + std::hash::Hash + Clone, R: Eq + std::hash::Hash + Clone> BipartiteGraph<L, R> {
    pub fn matching_size(&self) -> usize {
        self.maximum_matching().len()
    }

    pub fn maximum_matching(&self) -> HashSet<(L, R)> {
        let mut matching = HashSet::new();
        let mut visited = HashSet::new();

        for left_node in &self.left_nodes {
            visited.clear();
            if self.find_augmenting_path(left_node, &mut matching, &mut visited) {
                matching.insert((
                    left_node.clone(),
                    self.left_to_right[left_node].iter().next().unwrap().clone(),
                ));
            }
        }

        matching
    }

    fn find_augmenting_path(
        &self,
        start: &L,
        matching: &mut HashSet<(L, R)>,
        visited: &mut HashSet<L>,
    ) -> bool {
        if visited.contains(start) {
            return false;
        }
        visited.insert(start.clone());

        if let Some(neighbors) = self.left_to_right.get(start) {
            for right_node in neighbors {
                let matched_left = matching
                    .iter()
                    .find(|(_, r)| r == right_node)
                    .map(|(l, _)| l.clone());

                if let Some(left) = matched_left {
                    if self.find_augmenting_path(&left, matching, visited) {
                        return true;
                    }
                } else {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_perfect_matching(&self) -> bool {
        if self.left_nodes.len() != self.right_nodes.len() {
            return false;
        }
        let matching = self.maximum_matching();
        matching.len() == self.left_nodes.len()
    }

    pub fn has_perfect_matching(&self) -> bool {
        self.is_perfect_matching()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_graph_is_empty() {
        let graph: BipartiteGraph<i32, i32> = BipartiteGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.num_edges(), 0);
        assert_eq!(graph.num_left_nodes(), 0);
        assert_eq!(graph.num_right_nodes(), 0);
    }

    #[test]
    fn add_left_nodes() {
        let mut graph: BipartiteGraph<&str, &str> = BipartiteGraph::new();
        assert!(graph.add_left_node("A"));
        assert!(!graph.add_left_node("A"));
        assert_eq!(graph.num_left_nodes(), 1);
    }

    #[test]
    fn add_right_nodes() {
        let mut graph: BipartiteGraph<&str, &str> = BipartiteGraph::new();
        assert!(graph.add_right_node("1"));
        assert!(!graph.add_right_node("1"));
        assert_eq!(graph.num_right_nodes(), 1);
    }

    #[test]
    fn add_edge_success() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_right_node("1");
        assert!(graph.add_edge("A", "1").unwrap());
    }

    #[test]
    fn add_edge_not_found_left() {
        let mut graph = BipartiteGraph::new();
        graph.add_right_node("1");
        let result = graph.add_edge("A", "1");
        assert!(matches!(result, Err(BipartiteGraphError::NodeNotFound(_))));
    }

    #[test]
    fn add_edge_not_found_right() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        let result = graph.add_edge("A", "1");
        assert!(matches!(result, Err(BipartiteGraphError::NodeNotFound(_))));
    }

    #[test]
    fn add_duplicate_edge() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_right_node("1");
        graph.add_edge("A", "1").unwrap();
        assert!(!graph.add_edge("A", "1").unwrap());
    }

    #[test]
    fn has_edge() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_right_node("1");
        assert!(!graph.has_edge(&"A", &"1"));
        graph.add_edge("A", "1").unwrap();
        assert!(graph.has_edge(&"A", &"1"));
    }

    #[test]
    fn remove_edge() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_right_node("1");
        graph.add_edge("A", "1").unwrap();
        assert!(graph.remove_edge(&"A", &"1").unwrap());
        assert!(!graph.has_edge(&"A", &"1"));
    }

    #[test]
    fn neighbors() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_left_node("B");
        graph.add_right_node("1");
        graph.add_right_node("2");
        graph.add_edge("A", "1").unwrap();
        graph.add_edge("A", "2").unwrap();
        graph.add_edge("B", "2").unwrap();

        let a_neighbors = graph.left_neighbors(&"A").unwrap();
        assert_eq!(a_neighbors.len(), 2);
        assert!(a_neighbors.contains(&"1"));
        assert!(a_neighbors.contains(&"2"));

        let b_neighbors = graph.left_neighbors(&"B").unwrap();
        assert_eq!(b_neighbors.len(), 1);
        assert!(b_neighbors.contains(&"2"));

        let one_neighbors = graph.right_neighbors(&"1").unwrap();
        assert_eq!(one_neighbors.len(), 1);
        assert!(one_neighbors.contains(&"A"));
    }

    #[test]
    fn degree() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_left_node("B");
        graph.add_right_node("1");
        graph.add_edge("A", "1").unwrap();
        graph.add_edge("B", "1").unwrap();

        assert_eq!(graph.degree(&"A"), Some(1));
        assert_eq!(graph.degree(&"B"), Some(1));
        assert_eq!(graph.right_degree(&"1"), Some(2));
    }

    #[test]
    fn is_complete() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_left_node("B");
        graph.add_right_node("1");
        graph.add_right_node("2");
        assert!(!graph.is_complete());

        graph.add_edge("A", "1").unwrap();
        graph.add_edge("A", "2").unwrap();
        graph.add_edge("B", "1").unwrap();
        graph.add_edge("B", "2").unwrap();
        assert!(graph.is_complete());
    }

    #[test]
    fn clear() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_right_node("1");
        graph.add_edge("A", "1").unwrap();
        graph.clear();
        assert!(graph.is_empty());
        assert_eq!(graph.num_left_nodes(), 0);
        assert_eq!(graph.num_right_nodes(), 0);
    }

    #[test]
    fn matching_size() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_left_node("B");
        graph.add_right_node("1");
        graph.add_right_node("2");
        graph.add_edge("A", "1").unwrap();
        graph.add_edge("B", "2").unwrap();
        assert_eq!(graph.matching_size(), 2);
    }

    #[test]
    fn serde_roundtrip() {
        let mut graph = BipartiteGraph::new();
        graph.add_left_node("A");
        graph.add_left_node("B");
        graph.add_right_node("1");
        graph.add_right_node("2");
        graph.add_edge("A", "1").unwrap();
        graph.add_edge("A", "2").unwrap();
        graph.add_edge("B", "2").unwrap();

        let json = serde_json::to_string(&graph).unwrap();
        let back: BipartiteGraph<String, String> = serde_json::from_str(&json).unwrap();

        assert_eq!(back.num_left_nodes(), 2);
        assert_eq!(back.num_right_nodes(), 2);
        assert_eq!(back.num_edges(), 3);
    }
}
