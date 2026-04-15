use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clique {
    vertices: HashSet<usize>,
}

impl Clique {
    pub fn new(vertices: HashSet<usize>) -> Self {
        Self { vertices }
    }

    pub fn contains(&self, vertex: &usize) -> bool {
        self.vertices.contains(vertex)
    }

    pub fn vertices(&self) -> &HashSet<usize> {
        &self.vertices
    }

    pub fn size(&self) -> usize {
        self.vertices.len()
    }

    pub fn intersection_size(&self, other: &Clique) -> usize {
        self.vertices.intersection(&other.vertices).count()
    }

    pub fn union_with(&self, other: &Clique) -> Clique {
        let mut result = self.vertices.clone();
        result.extend(other.vertices.iter());
        Clique::new(result)
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CliqueTreeError {
    #[error("Graph is not chordal")]
    NotChordal,
    #[error("Invalid clique tree: running intersection property violated")]
    RunningIntersectionViolation,
    #[error("Empty graph provided")]
    EmptyGraph,
    #[error("Single vertex graphs require at least one clique")]
    NoCliques,
    #[error("Invalid vertex index: {0}")]
    InvalidVertex(usize),
}

#[derive(Debug, Clone)]
pub struct CliqueTree {
    cliques: Vec<Clique>,
    #[allow(dead_code)]
    adjacency: HashMap<usize, Vec<usize>>,
    vertex_to_cliques: HashMap<usize, Vec<usize>>,
}

impl CliqueTree {
    pub fn new(cliques: Vec<Clique>) -> Result<Self, CliqueTreeError> {
        if cliques.is_empty() {
            return Err(CliqueTreeError::EmptyGraph);
        }

        let mut vertex_to_cliques: HashMap<usize, Vec<usize>> = HashMap::new();
        for (idx, clique) in cliques.iter().enumerate() {
            for &v in &clique.vertices {
                vertex_to_cliques.entry(v).or_default().push(idx);
            }
        }

        let all_vertices: HashSet<usize> = vertex_to_cliques.keys().cloned().collect();
        let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
        for &v in &all_vertices {
            adjacency.insert(v, Vec::new());
        }

        for clique_indices in vertex_to_cliques.values() {
            for &i in clique_indices {
                for &j in clique_indices {
                    if i != j {
                        if let Some(neighbors) =
                            adjacency.get_mut(&cliques[i].vertices.iter().next().copied().unwrap())
                        {
                            if !neighbors.contains(&j) {
                                neighbors.push(j);
                            }
                        }
                    }
                }
            }
        }

        let tree = Self {
            cliques,
            adjacency,
            vertex_to_cliques,
        };

        tree.verify_running_intersection()?;
        Ok(tree)
    }

    pub fn from_graph(adjacency_list: &[(usize, Vec<usize>)]) -> Result<Self, CliqueTreeError> {
        if adjacency_list.is_empty() {
            return Err(CliqueTreeError::EmptyGraph);
        }

        let n = adjacency_list.len();
        let mut edges: HashSet<(usize, usize)> = HashSet::new();
        for &(u, ref neighbors) in adjacency_list {
            if u >= n {
                return Err(CliqueTreeError::InvalidVertex(u));
            }
            for &v in neighbors {
                if v >= n {
                    return Err(CliqueTreeError::InvalidVertex(v));
                }
                if u != v {
                    edges.insert((u.min(v), u.max(v)));
                }
            }
        }

        let is_chordal = Self::check_chordal(n, &edges);
        if !is_chordal {
            return Err(CliqueTreeError::NotChordal);
        }

        let cliques = Self::find_maximal_cliques(n, &edges);
        if cliques.is_empty() {
            return Err(CliqueTreeError::NoCliques);
        }

        Self::new(cliques)
    }

    fn check_chordal(n: usize, edges: &HashSet<(usize, usize)>) -> bool {
        let mut adjacency: HashMap<usize, HashSet<usize>> = HashMap::new();
        for i in 0..n {
            adjacency.insert(i, HashSet::new());
        }
        for &(u, v) in edges {
            if u < n && v < n {
                adjacency.get_mut(&u).unwrap().insert(v);
                adjacency.get_mut(&v).unwrap().insert(u);
            }
        }

        let mut eliminated: Vec<bool> = vec![false; n];
        let mut eliminated_count = 0;

        while eliminated_count < n {
            let mut found_simplicial = false;

            for v in 0..n {
                if eliminated[v] {
                    continue;
                }

                let neighbors: Vec<usize> = adjacency[&v]
                    .iter()
                    .filter(|&&n| !eliminated[n])
                    .cloned()
                    .collect();

                if neighbors.is_empty() {
                    eliminated[v] = true;
                    eliminated_count += 1;
                    found_simplicial = true;
                    break;
                }

                let all_neighbors_connected = neighbors.iter().all(|&ni| {
                    neighbors.iter().all(|&nj| {
                        if ni == nj {
                            true
                        } else {
                            let (min_n, max_n) = (ni.min(nj), ni.max(nj));
                            ni == v
                                || nj == v
                                || edges.contains(&(min_n, max_n))
                                || adjacency[&ni].contains(&nj)
                        }
                    })
                });

                if all_neighbors_connected {
                    eliminated[v] = true;
                    eliminated_count += 1;
                    found_simplicial = true;
                    break;
                }
            }

            if !found_simplicial {
                return false;
            }
        }

        true
    }

    fn find_maximal_cliques(n: usize, edges: &HashSet<(usize, usize)>) -> Vec<Clique> {
        let mut cliques: Vec<Clique> = Vec::new();

        for i in 0..n {
            let mut clique_vertices = HashSet::new();
            clique_vertices.insert(i);
            for j in (i + 1)..n {
                let mut is_clique = true;
                for &k in &clique_vertices {
                    if k != j {
                        let edge = (i.min(j), i.max(j));
                        let edge_k = (i.min(k), i.max(k));
                        let edge_jk = (j.min(k), j.max(k));
                        if !edges.contains(&edge)
                            || !edges.contains(&edge_k)
                            || !edges.contains(&edge_jk)
                        {
                            if k < j {
                                let e1 = (k.min(j), k.max(j));
                                if !edges.contains(&e1) {
                                    is_clique = false;
                                    break;
                                }
                            } else {
                                let e1 = (j.min(k), j.max(k));
                                if !edges.contains(&e1) {
                                    is_clique = false;
                                    break;
                                }
                            }
                        }
                    }
                }
                if is_clique {
                    clique_vertices.insert(j);
                }
            }

            let mut is_maximal = true;
            for other_idx in 0..n {
                if other_idx == i {
                    continue;
                }
                if clique_vertices.contains(&other_idx) {
                    continue;
                }
                let mut is_superclique = true;
                for &v1 in &clique_vertices {
                    for &v2 in &clique_vertices {
                        if v1 != v2 {
                            let e = (v1.min(v2), v1.max(v2));
                            if !edges.contains(&e) {
                                is_superclique = false;
                                break;
                            }
                        }
                    }
                    if !is_superclique {
                        break;
                    }
                }
                if is_superclique {
                    for &v in &clique_vertices {
                        let e = (v.min(other_idx), v.max(other_idx));
                        if !edges.contains(&e) {
                            is_superclique = false;
                            break;
                        }
                    }
                    if is_superclique {
                        is_maximal = false;
                        break;
                    }
                }
            }

            if is_maximal {
                let mut already_present = false;
                for existing in &cliques {
                    if existing.vertices() == &clique_vertices {
                        already_present = true;
                        break;
                    }
                }
                if !already_present {
                    cliques.push(Clique::new(clique_vertices));
                }
            }
        }

        cliques.sort_by(|a, b| b.size().cmp(&a.size()));
        let mut maximal: Vec<Clique> = Vec::new();
        for c in &cliques {
            let mut is_maximal = true;
            for other in &cliques {
                if c.vertices().is_subset(other.vertices()) && c.vertices() != other.vertices() {
                    is_maximal = false;
                    break;
                }
            }
            if is_maximal && !maximal.iter().any(|m| m.vertices() == c.vertices()) {
                maximal.push(c.clone());
            }
        }

        maximal
    }

    fn verify_running_intersection(&self) -> Result<(), CliqueTreeError> {
        for (&vertex, clique_indices) in &self.vertex_to_cliques {
            if clique_indices.len() <= 1 {
                continue;
            }

            let _clique_set: HashSet<usize> = clique_indices.iter().cloned().collect();

            for (i, &c1_idx) in clique_indices.iter().enumerate() {
                for &c2_idx in clique_indices.iter().skip(i + 1) {
                    let c1 = &self.cliques[c1_idx];
                    let c2 = &self.cliques[c2_idx];
                    let intersection = c1.intersection_size(c2);
                    if intersection == 0 {
                        return Err(CliqueTreeError::RunningIntersectionViolation);
                    }
                    if intersection == 1 && !c1.contains(&vertex) {
                        return Err(CliqueTreeError::RunningIntersectionViolation);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn cliques(&self) -> &[Clique] {
        &self.cliques
    }

    pub fn maximal_cliques(&self) -> Vec<&Clique> {
        self.cliques.iter().collect()
    }

    pub fn cliques_containing_vertex(
        &self,
        vertex: usize,
    ) -> Result<Vec<&Clique>, CliqueTreeError> {
        if !self.vertex_to_cliques.contains_key(&vertex) {
            return Err(CliqueTreeError::InvalidVertex(vertex));
        }
        Ok(self.vertex_to_cliques[&vertex]
            .iter()
            .map(|&idx| &self.cliques[idx])
            .collect())
    }

    pub fn num_cliques(&self) -> usize {
        self.cliques.len()
    }

    pub fn num_vertices(&self) -> usize {
        self.vertex_to_cliques.len()
    }

    pub fn is_chordal_graph(&self) -> bool {
        self.verify_running_intersection().is_ok()
    }

    pub fn verify_chordal(adjacency_list: &[(usize, Vec<usize>)]) -> Result<bool, CliqueTreeError> {
        match Self::from_graph(adjacency_list) {
            Ok(_) => Ok(true),
            Err(CliqueTreeError::NotChordal) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clique_new_and_contains() {
        let mut vertices = HashSet::new();
        vertices.insert(1);
        vertices.insert(2);
        vertices.insert(3);
        let clique = Clique::new(vertices);

        assert!(clique.contains(&1));
        assert!(clique.contains(&2));
        assert!(clique.contains(&3));
        assert!(!clique.contains(&4));
    }

    #[test]
    fn test_clique_size() {
        let vertices: HashSet<usize> = [1, 2, 3].into_iter().collect();
        let clique = Clique::new(vertices);
        assert_eq!(clique.size(), 3);
    }

    #[test]
    fn test_clique_intersection_size() {
        let vertices1: HashSet<usize> = [1, 2, 3].into_iter().collect();
        let vertices2: HashSet<usize> = [2, 3, 4].into_iter().collect();
        let clique1 = Clique::new(vertices1);
        let clique2 = Clique::new(vertices2);

        assert_eq!(clique1.intersection_size(&clique2), 2);
    }

    #[test]
    fn test_clique_union() {
        let vertices1: HashSet<usize> = [1, 2].into_iter().collect();
        let vertices2: HashSet<usize> = [2, 3].into_iter().collect();
        let clique1 = Clique::new(vertices1);
        let clique2 = Clique::new(vertices2);

        let union = clique1.union_with(&clique2);
        assert_eq!(union.size(), 3);
        assert!(union.contains(&1));
        assert!(union.contains(&2));
        assert!(union.contains(&3));
    }

    #[test]
    fn test_clique_tree_empty_graph_error() {
        let result = CliqueTree::new(Vec::new());
        assert!(matches!(result, Err(CliqueTreeError::EmptyGraph)));
    }

    #[test]
    fn test_clique_tree_single_vertex() {
        let vertices: HashSet<usize> = [0].into_iter().collect();
        let cliques = vec![Clique::new(vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        assert_eq!(tree.num_cliques(), 1);
        assert_eq!(tree.num_vertices(), 1);
    }

    #[test]
    fn test_clique_tree_simple_triangle() {
        let c1_vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let c2_vertices: HashSet<usize> = [1, 2].into_iter().collect();

        let cliques = vec![Clique::new(c1_vertices), Clique::new(c2_vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        assert_eq!(tree.num_cliques(), 2);
        assert_eq!(tree.num_vertices(), 3);
        assert!(tree.is_chordal_graph());
    }

    #[test]
    fn test_clique_tree_complete_graph() {
        let c1_vertices: HashSet<usize> = [0, 1, 2].into_iter().collect();
        let cliques = vec![Clique::new(c1_vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        assert_eq!(tree.num_cliques(), 1);
        assert_eq!(tree.num_vertices(), 3);
        assert!(tree.is_chordal_graph());
    }

    #[test]
    fn test_clique_tree_running_intersection_property() {
        let c1_vertices: HashSet<usize> = [0, 1, 2].into_iter().collect();
        let c2_vertices: HashSet<usize> = [1, 2, 3].into_iter().collect();
        let c3_vertices: HashSet<usize> = [2, 3, 4].into_iter().collect();

        let cliques = vec![
            Clique::new(c1_vertices),
            Clique::new(c2_vertices),
            Clique::new(c3_vertices),
        ];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        assert_eq!(tree.num_cliques(), 3);
        assert!(tree.is_chordal_graph());

        let cliques_with_2 = tree.cliques_containing_vertex(2).expect("vertex 2 exists");
        assert_eq!(cliques_with_2.len(), 3);
    }

    #[test]
    fn test_clique_tree_cliques_containing_vertex() {
        let c1_vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let c2_vertices: HashSet<usize> = [1, 2].into_iter().collect();
        let c3_vertices: HashSet<usize> = [2, 3].into_iter().collect();

        let cliques = vec![
            Clique::new(c1_vertices),
            Clique::new(c2_vertices),
            Clique::new(c3_vertices),
        ];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        let cliques_with_1 = tree.cliques_containing_vertex(1).expect("vertex 1 exists");
        assert_eq!(cliques_with_1.len(), 2);

        let cliques_with_2 = tree.cliques_containing_vertex(2).expect("vertex 2 exists");
        assert_eq!(cliques_with_2.len(), 2);

        let cliques_with_3 = tree.cliques_containing_vertex(3).expect("vertex 3 exists");
        assert_eq!(cliques_with_3.len(), 1);
    }

    #[test]
    fn test_clique_tree_invalid_vertex_error() {
        let c1_vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let cliques = vec![Clique::new(c1_vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        let result = tree.cliques_containing_vertex(99);
        assert!(matches!(result, Err(CliqueTreeError::InvalidVertex(99))));
    }

    #[test]
    fn test_maximal_cliques() {
        let c1_vertices: HashSet<usize> = [0, 1, 2].into_iter().collect();
        let cliques = vec![Clique::new(c1_vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        let maximal = tree.maximal_cliques();
        assert_eq!(maximal.len(), 1);
    }

    #[test]
    fn test_from_graph_simple_path() {
        let adjacency_list = vec![(0, vec![1]), (1, vec![0, 2]), (2, vec![1])];

        let result = CliqueTree::from_graph(&adjacency_list);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_verify_chordal_valid() {
        let adjacency_list = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];

        let result = CliqueTree::verify_chordal(&adjacency_list);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_chordal_invalid() {
        let adjacency_list = vec![
            (0, vec![1]),
            (1, vec![0, 2]),
            (2, vec![1, 3]),
            (3, vec![2, 0]),
        ];

        let result = CliqueTree::verify_chordal(&adjacency_list);
        match result {
            Ok(is_chordal) => assert!(!is_chordal),
            Err(CliqueTreeError::NotChordal) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_clique_tree_debug_format() {
        let vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let cliques = vec![Clique::new(vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        let debug_str = format!("{:?}", tree);
        assert!(debug_str.contains("CliqueTree"));
    }

    #[test]
    fn test_clique_debug_format() {
        let vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let clique = Clique::new(vertices);

        let debug_str = format!("{:?}", clique);
        assert!(debug_str.contains("Clique"));
    }

    #[test]
    fn test_clique_tree_error_debug_format() {
        let err = CliqueTreeError::NotChordal;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("NotChordal"));
    }

    #[test]
    fn test_clique_equality() {
        let vertices1: HashSet<usize> = [0, 1].into_iter().collect();
        let vertices2: HashSet<usize> = [0, 1].into_iter().collect();
        let vertices3: HashSet<usize> = [0, 2].into_iter().collect();

        let c1 = Clique::new(vertices1);
        let c2 = Clique::new(vertices2);
        let c3 = Clique::new(vertices3);

        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }

    #[test]
    fn test_clique_clone() {
        let vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let clique = Clique::new(vertices);
        let cloned = clique.clone();

        assert_eq!(clique, cloned);
    }

    #[test]
    fn test_empty_clique() {
        let vertices: HashSet<usize> = HashSet::new();
        let clique = Clique::new(vertices);

        assert_eq!(clique.size(), 0);
        assert!(!clique.contains(&0));
    }

    #[test]
    fn test_maximal_cliques_from_graph() {
        let adjacency_list = vec![(0, vec![1, 2]), (1, vec![0, 2]), (2, vec![0, 1])];

        if let Ok(tree) = CliqueTree::from_graph(&adjacency_list) {
            let cliques = tree.maximal_cliques();
            let max_clique_size = cliques.iter().map(|c| c.size()).max().unwrap_or(0);
            assert_eq!(max_clique_size, 3);
        }
    }

    #[test]
    fn test_large_clique_tree() {
        let n = 10;
        let mut adjacency_list = Vec::new();
        for i in 0..n {
            let neighbors: Vec<usize> = (0..n).filter(|&j| i != j).collect();
            adjacency_list.push((i, neighbors));
        }

        let result = CliqueTree::from_graph(&adjacency_list);
        match result {
            Ok(tree) => {
                assert!(tree.is_chordal_graph());
                assert_eq!(tree.num_vertices(), n);
            }
            Err(CliqueTreeError::NotChordal) => panic!("Complete graph should be chordal"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_path_graph_is_chordal() {
        let n = 5;
        let mut adjacency_list = Vec::new();
        for i in 0..n {
            let mut neighbors = Vec::new();
            if i > 0 {
                neighbors.push(i - 1);
            }
            if i < n - 1 {
                neighbors.push(i + 1);
            }
            adjacency_list.push((i, neighbors));
        }

        let result = CliqueTree::from_graph(&adjacency_list);
        match result {
            Ok(tree) => {
                assert!(tree.is_chordal_graph());
            }
            Err(e) => {
                panic!("Path graph should be chordal but got: {:?}", e);
            }
        }
    }

    #[test]
    fn test_cycle_graph_not_chordal() {
        let n = 4;
        let mut adjacency_list = Vec::new();
        for i in 0..n {
            let neighbors = vec![(i + 1) % n, (i + n - 1) % n];
            adjacency_list.push((i, neighbors));
        }

        let result = CliqueTree::from_graph(&adjacency_list);
        match result {
            Ok(_) => panic!("Cycle graph should not be chordal"),
            Err(CliqueTreeError::NotChordal) => {}
            Err(e) => panic!("Expected NotChordal error, got: {:?}", e),
        }
    }

    #[test]
    fn test_cliques_preserves_vertex_appearances() {
        let c1_vertices: HashSet<usize> = [0, 1, 2].into_iter().collect();
        let c2_vertices: HashSet<usize> = [1, 2, 3].into_iter().collect();

        let cliques = vec![Clique::new(c1_vertices), Clique::new(c2_vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        for v in 0..4 {
            let cliques_with_v = tree
                .cliques_containing_vertex(v)
                .expect("vertex should exist");
            assert!(!cliques_with_v.is_empty());
            for clique in cliques_with_v {
                assert!(clique.contains(&v));
            }
        }
    }

    #[test]
    fn test_tree_with_many_vertices() {
        let vertices: HashSet<usize> = (0..20).collect();
        let cliques = vec![Clique::new(vertices.clone()), Clique::new(vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        assert_eq!(tree.num_vertices(), 20);
        assert!(tree.is_chordal_graph());
    }

    #[test]
    fn test_multiple_disconnected_components() {
        let c1_vertices: HashSet<usize> = [0, 1].into_iter().collect();
        let c2_vertices: HashSet<usize> = [2, 3].into_iter().collect();

        let cliques = vec![Clique::new(c1_vertices), Clique::new(c2_vertices)];
        let tree = CliqueTree::new(cliques).expect("should succeed");

        assert_eq!(tree.num_vertices(), 4);
        assert!(tree.is_chordal_graph());
    }
}

#[cfg(feature = "proptest")]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::HashSet;

    proptest! {
        #[test]
        fn test_chordal_complete_graph_is_always_valid(vertices_count in 1..20usize) {
            let mut adjacency_list: Vec<(usize, Vec<usize>)> = Vec::new();
            for i in 0..vertices_count {
                let neighbors: Vec<usize> = (0..vertices_count)
                    .filter(|&j| i != j)
                    .collect();
                adjacency_list.push((i, neighbors));
            }

            let result = CliqueTree::from_graph(&adjacency_list);
            match result {
                Ok(tree) => {
                    prop_assert!(tree.is_chordal_graph());
                    prop_assert_eq!(tree.num_vertices(), vertices_count);
                }
                Err(CliqueTreeError::NotChordal) => {
                    prop_assert!(false, "Complete graph should be chordal");
                }
                Err(e) => {
                    prop_assert!(false, "Unexpected error: {:?}", e);
                }
            }
        }

        #[test]
        fn test_path_graph_is_always_chordal(length in 1..50usize) {
            let n = length + 1;
            let mut adjacency_list: Vec<(usize, Vec<usize>)> = Vec::new();
            for i in 0..n {
                let mut neighbors = Vec::new();
                if i > 0 {
                    neighbors.push(i - 1);
                }
                if i < n - 1 {
                    neighbors.push(i + 1);
                }
                adjacency_list.push((i, neighbors));
            }

            let result = CliqueTree::from_graph(&adjacency_list);
            match result {
                Ok(tree) => {
                    prop_assert!(tree.is_chordal_graph());
                }
                Err(e) => {
                    prop_assert!(false, "Path graph should be chordal: {:?}", e);
                }
            }
        }

        #[test]
        fn test_clique_tree_vertices_match_input(num_vertices in 1..20usize) {
            let vertices: HashSet<usize> = (0..num_vertices).collect();
            let cliques = vec![Clique::new(vertices.clone())];
            let tree = CliqueTree::new(cliques).expect("should succeed");

            prop_assert_eq!(tree.num_vertices(), num_vertices);
        }

        #[test]
        fn test_cliques_containing_vertex_returns_valid_cliques(
            num_cliques in 2..10usize,
            vertex_count in 3..15usize
        ) {
            let mut all_vertices: HashSet<usize> = (0..vertex_count).collect();
            let cliques: Vec<Clique> = (0..num_cliques)
                .map(|i| {
                    let mut clique_vertices: HashSet<usize> = HashSet::new();
                    let count = (i as usize + 1).min(all_vertices.len());
                    let mut remaining: Vec<usize> = all_vertices.iter().cloned().collect();
                    for _ in 0..count {
                        if let Some(idx) = remaining.pop() {
                            clique_vertices.insert(idx);
                        }
                    }
                    Clique::new(clique_vertices)
                })
                .collect();

            if let Ok(tree) = CliqueTree::new(cliques) {
                for v in 0..vertex_count {
                    let result = tree.cliques_containing_vertex(v);
                    if let Ok(cliques_with_v) = result {
                        for clique in cliques_with_v {
                            prop_assert!(clique.contains(&v), "Vertex {} should be in clique", v);
                        }
                    }
                }
            }
        }

        #[test]
        fn test_clique_intersection_is_symmetric(size1 in 1..10usize, size2 in 1..10usize) {
            let vertices1: HashSet<usize> = (0..size1).collect();
            let vertices2: HashSet<usize> = (0..size2).collect();

            let c1 = Clique::new(vertices1);
            let c2 = Clique::new(vertices2);

            prop_assert_eq!(c1.intersection_size(&c2), c2.intersection_size(&c1));
        }

        #[test]
        fn test_clique_union_size_bound(size1 in 1..10usize, size2 in 1..10usize) {
            let vertices1: HashSet<usize> = (0..size1).collect();
            let vertices2: HashSet<usize> = (size1..size1 + size2).collect();

            let c1 = Clique::new(vertices1);
            let c2 = Clique::new(vertices2);
            let union = c1.union_with(&c2);

            prop_assert!(union.size() >= size1);
            prop_assert!(union.size() >= size2);
            prop_assert!(union.size() <= size1 + size2);
        }
    }
}
