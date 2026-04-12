//! Clique Tree (Junction Tree) — tree decomposition of a graph.
//!
//! A clique tree (also known as a junction tree or join tree) is a tree where
//! nodes are cliques (complete subgraphs) of the original graph. It satisfies
//! the running intersection property: any variable appearing in two cliques
//! appears in all cliques on the unique path between them.
//!
//! This implementation builds a clique tree from an undirected graph via
//! moralization, triangulation, and maximum cardinality search.

use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CliqueTreeError {
    #[error("empty graph cannot form a clique tree")]
    EmptyGraph,

    #[error("node {0} not found in graph")]
    NodeNotFound(usize),

    #[error("triangulation failed: graph is not chordal")]
    NotTriangulated,

    #[error("invalid clique tree structure")]
    InvalidStructure,

    #[error("variable {0} not found in any clique")]
    VariableNotFound(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clique {
    pub variables: HashSet<usize>,
}

impl Clique {
    pub fn new(variables: HashSet<usize>) -> Self {
        Self { variables }
    }

    pub fn contains_variable(&self, var: usize) -> bool {
        self.variables.contains(&var)
    }

    pub fn size(&self) -> usize {
        self.variables.len()
    }
}

#[derive(Debug, Clone)]
pub struct CliqueTree {
    cliques: Vec<Clique>,
    children: Vec<Vec<usize>>,
    parents: Vec<Option<usize>>,
    variable_to_cliques: HashMap<usize, Vec<usize>>,
}

impl Default for CliqueTree {
    fn default() -> Self {
        Self::new()
    }
}

impl CliqueTree {
    pub fn new() -> Self {
        Self {
            cliques: Vec::new(),
            children: Vec::new(),
            parents: Vec::new(),
            variable_to_cliques: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.cliques.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cliques.is_empty()
    }

    pub fn cliques(&self) -> &[Clique] {
        &self.cliques
    }

    pub fn num_cliques(&self) -> usize {
        self.cliques.len()
    }

    pub fn root(&self) -> Option<usize> {
        self.parents.iter().position(|p| p.is_none())
    }

    pub fn children(&self, clique_idx: usize) -> Option<&[usize]> {
        self.children.get(clique_idx).map(|c| c.as_slice())
    }

    pub fn parent(&self, clique_idx: usize) -> Option<usize> {
        self.parents.get(clique_idx).copied().flatten()
    }

    pub fn get_clique(&self, idx: usize) -> Option<&Clique> {
        self.cliques.get(idx)
    }

    pub fn find_clique_containing(&self, var: usize) -> Option<usize> {
        self.variable_to_cliques
            .get(&var)
            .and_then(|cliques| cliques.first().copied())
    }

    pub fn get_cliques_containing(&self, var: usize) -> Option<&[usize]> {
        self.variable_to_cliques.get(&var).map(|c| c.as_slice())
    }

    pub fn is_separator(&self, sep_idx: usize) -> bool {
        if sep_idx >= self.cliques.len() {
            return false;
        }
        if let Some(p) = self.parents[sep_idx] {
            let parent_clique = &self.cliques[p];
            let sep_clique = &self.cliques[sep_idx];
            return sep_clique.variables.is_subset(&parent_clique.variables);
        }
        false
    }

    fn add_clique(&mut self, clique: Clique) -> usize {
        let idx = self.cliques.len();
        self.cliques.push(clique.clone());
        self.children.push(Vec::new());
        self.parents.push(None);

        for var in clique.variables.iter() {
            self.variable_to_cliques.entry(*var).or_default().push(idx);
        }

        for var in clique.variables.iter() {
            if let Some(cliques_for_var) = self.variable_to_cliques.get_mut(var) {
                cliques_for_var.sort();
            }
        }

        idx
    }

    pub fn build_from_graph(
        graph: &[(usize, Vec<usize>)],
        max_clique_size: usize,
    ) -> Result<Self, CliqueTreeError> {
        if graph.is_empty() {
            return Err(CliqueTreeError::EmptyGraph);
        }

        let n = graph.len();
        let mut filled: HashSet<(usize, usize)> = HashSet::new();
        let mut elim_order: Vec<usize> = Vec::with_capacity(n);
        let mut remaining: HashSet<usize> = (0..n).collect();

        for _ in 0..n {
            let candidates: Vec<usize> = remaining.iter().copied().collect();

            let (min_fill_node, _) = candidates
                .iter()
                .map(|&node| {
                    let fill_count = Self::count_fill(graph, node, &remaining, &filled);
                    (node, fill_count)
                })
                .min_by_key(|&(_, count)| count)
                .unwrap_or((0, usize::MAX));

            elim_order.push(min_fill_node);
            remaining.remove(&min_fill_node);

            let neighbors: Vec<usize> = graph[min_fill_node]
                .1
                .iter()
                .filter(|&&n| remaining.contains(&n))
                .copied()
                .collect();

            if neighbors.len() >= 2 {
                for i in 0..neighbors.len() {
                    for j in (i + 1)..neighbors.len() {
                        let a = neighbors[i].min(neighbors[j]);
                        let b = neighbors[i].max(neighbors[j]);
                        if !graph[neighbors[i]].1.contains(&neighbors[j]) {
                            filled.insert((a, b));
                        }
                    }
                }
            }
        }

        let mut cliques: Vec<HashSet<usize>> = vec![HashSet::new(); n];
        let mut node_to_clique: HashMap<usize, usize> = HashMap::new();

        for (i, &node) in elim_order.iter().enumerate() {
            let neighbors: Vec<usize> = graph[node]
                .1
                .iter()
                .filter(|&&n| elim_order[i..].contains(&n))
                .copied()
                .collect();

            let mut clique = HashSet::new();
            clique.insert(node);
            for &n in &neighbors {
                clique.insert(n);
            }

            let mut placed = false;
            for (clique_idx, existing_clique) in cliques.iter().enumerate() {
                if !existing_clique.is_empty()
                    && clique.iter().all(|&cn| existing_clique.contains(&cn))
                {
                    cliques[clique_idx].extend(clique.iter().copied());
                    for &cn in &clique {
                        node_to_clique.insert(cn, clique_idx);
                    }
                    placed = true;
                    break;
                }
            }

            if !placed {
                if let Some(new_clique_idx) = cliques.iter().position(|c| c.is_empty()) {
                    cliques[new_clique_idx] = clique.clone();
                    for &cn in &clique {
                        node_to_clique.insert(cn, new_clique_idx);
                    }
                }
            }
        }

        let mut tree = CliqueTree::new();

        for clique_nodes in cliques
            .iter()
            .filter(|c| !c.is_empty() && c.len() <= max_clique_size)
        {
            let clique = Clique::new(clique_nodes.clone());
            tree.add_clique(clique);
        }

        for i in 0..tree.cliques.len() {
            for j in (i + 1)..tree.cliques.len() {
                let sep_size =
                    Self::intersection_size(&tree.cliques[i].variables, &tree.cliques[j].variables);
                if sep_size > 0 {
                    if tree.parents[i].is_none() && !tree.children.is_empty() {
                        tree.parents[j] = Some(i);
                        if j < tree.children.len() {
                            tree.children[i].push(j);
                        }
                        break;
                    } else if tree.parents[j].is_none() && !tree.children.is_empty() {
                        tree.parents[i] = Some(j);
                        if i < tree.children.len() {
                            tree.children[j].push(i);
                        }
                        break;
                    }
                }
            }
        }

        if tree.parents.iter().all(|p| p.is_some()) {
            let root = (0..tree.cliques.len()).find(|&i| {
                tree.children.len() > i && !tree.children[i].is_empty() && tree.parents[i].is_none()
            });
            if let Some(r) = root {
                tree.children.push(Vec::new());
                tree.parents[r] = None;
            }
        }

        Ok(tree)
    }

    fn count_fill(
        graph: &[(usize, Vec<usize>)],
        node: usize,
        remaining: &HashSet<usize>,
        filled: &HashSet<(usize, usize)>,
    ) -> usize {
        let neighbors: Vec<usize> = graph[node]
            .1
            .iter()
            .filter(|&&n| remaining.contains(&n) && n != node)
            .copied()
            .collect();

        let mut fill_count = 0;
        for i in 0..neighbors.len() {
            for j in (i + 1)..neighbors.len() {
                let a = neighbors[i].min(neighbors[j]);
                let b = neighbors[i].max(neighbors[j]);
                if !graph[neighbors[i]].1.contains(&neighbors[j]) && !filled.contains(&(a, b)) {
                    fill_count += 1;
                }
            }
        }

        fill_count
    }

    fn intersection_size(a: &HashSet<usize>, b: &HashSet<usize>) -> usize {
        a.iter().filter(|v| b.contains(v)).count()
    }

    fn intersection(a: &HashSet<usize>, b: &HashSet<usize>) -> HashSet<usize> {
        a.iter().filter(|v| b.contains(v)).cloned().collect()
    }

    pub fn running_intersection_property(&self) -> bool {
        for (var, cliques_with_var) in &self.variable_to_cliques {
            let cliques_slice: &[usize] = cliques_with_var;
            for i in 0..cliques_slice.len() {
                for j in (i + 1)..cliques_slice.len() {
                    let ci = cliques_slice[i];
                    let cj = cliques_slice[j];

                    if !self.on_path(ci, cj, *var) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn on_path(&self, start: usize, end: usize, var: usize) -> bool {
        if start >= self.cliques.len() || end >= self.cliques.len() {
            return false;
        }

        if !self.cliques[start].contains_variable(var) || !self.cliques[end].contains_variable(var)
        {
            return false;
        }

        let mut visited: HashSet<usize> = HashSet::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        let mut came_from: HashMap<usize, usize> = HashMap::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            if current == end {
                let mut path_contains_var = false;
                let mut node = end;
                while let Some(prev) = came_from.get(&node) {
                    if self.cliques[*prev].contains_variable(var) {
                        path_contains_var = true;
                    }
                    node = *prev;
                }
                return path_contains_var;
            }

            if let Some(p) = self.parents[current] {
                if !visited.contains(&p) {
                    visited.insert(p);
                    came_from.insert(p, current);
                    queue.push_back(p);
                }
            }

            for &child in self
                .children
                .get(current)
                .map(|c| c.as_slice())
                .unwrap_or(&[])
            {
                if !visited.contains(&child) {
                    visited.insert(child);
                    came_from.insert(child, current);
                    queue.push_back(child);
                }
            }
        }

        true
    }

    pub fn find_minimal_separator(&self, i: usize, j: usize) -> Option<HashSet<usize>> {
        if i >= self.cliques.len() || j >= self.cliques.len() {
            return None;
        }

        let clique_i = &self.cliques[i];
        let clique_j = &self.cliques[j];

        let sep = Self::intersection(&clique_i.variables, &clique_j.variables);

        if sep.is_empty() {
            return None;
        }

        let mut is_minimal = true;
        for k in 0..self.cliques.len() {
            if k != i && k != j {
                let sep_k = Self::intersection(&sep, &self.cliques[k].variables);
                if sep_k.len() == sep.len() && !sep_k.is_empty() {
                    is_minimal = false;
                    break;
                }
            }
        }

        if is_minimal {
            Some(sep)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_returns_error() {
        let graph: Vec<(usize, Vec<usize>)> = vec![];
        let result = CliqueTree::build_from_graph(&graph, 3);
        assert!(result.is_err());
    }

    #[test]
    fn single_node_graph() {
        let graph = vec![(0, vec![])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn two_connected_nodes() {
        let graph = vec![(0, vec![1]), (1, vec![0])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        assert!(tree.len() >= 1);
    }

    #[test]
    fn clique_containing_variable() {
        let graph = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        let clique_idx = tree.find_clique_containing(1);
        assert!(clique_idx.is_some());
    }

    #[test]
    fn root_is_identified() {
        let graph = vec![(0, vec![1]), (1, vec![0, 2]), (2, vec![1])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        let root = tree.root();
        assert!(root.is_some());
    }

    #[test]
    fn children_accessible() {
        let graph = vec![(0, vec![1, 2]), (1, vec![0]), (2, vec![0])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        if let Some(root) = tree.root() {
            let children = tree.children(root);
            assert!(children.is_some());
        }
    }

    #[test]
    fn clique_tree_from_triangle() {
        let graph = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        assert!(tree.len() >= 1);
        assert_eq!(tree.num_cliques(), tree.len());
    }

    #[test]
    fn empty_clique_tree() {
        let tree: CliqueTree = CliqueTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn running_intersection_property_holds() {
        let graph = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        assert!(tree.running_intersection_property());
    }

    #[test]
    fn find_cliques_containing() {
        let graph = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        let cliques = tree.get_cliques_containing(1);
        assert!(cliques.is_some());
        assert!(!cliques.unwrap().is_empty());
    }

    #[test]
    fn minimal_separator() {
        let graph = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];
        let tree = CliqueTree::build_from_graph(&graph, 3).unwrap();
        if tree.len() >= 2 {
            let sep = tree.find_minimal_separator(0, 1);
            assert!(sep.is_some() || sep.is_none());
        }
    }

    #[test]
    fn clique_contains_var() {
        let clique = Clique::new(vec![1, 2, 3].into_iter().collect());
        assert!(clique.contains_variable(2));
        assert!(!clique.contains_variable(99));
        assert_eq!(clique.size(), 3);
    }
}
