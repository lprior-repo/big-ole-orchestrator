#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};

use crate::diagnostic::{Diagnostic, LintCode, Severity};
use crate::rules::unused_steps::{DagGraph, Edge, Step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleError {
    pub path: Vec<String>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cycle detected: {}", self.path.join("->"))
    }
}

pub struct CycleDetector;

impl CycleDetector {
    pub fn check(graph: &DagGraph) -> Result<(), CycleError> {
        if graph.steps.is_empty() {
            return Ok(());
        }

        let adj = graph.adjacency_list();
        let mut white: HashSet<String> = graph.steps.iter().map(|s| s.name.clone()).collect();
        let mut gray: HashSet<String> = HashSet::new();
        let in_cycle: HashMap<String, Vec<String>> = HashMap::new();

        for step in &graph.steps {
            if white.contains(&step.name) {
                if let Some(cycle_path) = Self::dfs_visit(&step.name, &adj, &mut white, &mut gray, &mut HashMap::new()) {
                    return Err(CycleError { path: cycle_path });
                }
            }
        }

        Ok(())
    }

    fn dfs_visit(
        node: &str,
        adj: &HashMap<String, Vec<String>>,
        white: &mut HashSet<String>,
        gray: &mut HashSet<String>,
        stack: &mut HashMap<String, Vec<String>>,
    ) -> Option<Vec<String>> {
        white.remove(node);
        gray.insert(node.to_string());

        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if gray.contains(neighbor) {
                    let mut cycle_path = vec![];
                    if let Some(path) = stack.get(node) {
                        cycle_path.extend(path.clone());
                    }
                    cycle_path.push(node.to_string());
                    cycle_path.push(neighbor.to_string());
                    return Some(cycle_path);
                }
                if white.contains(neighbor) {
                    let mut new_stack = stack.clone();
                    new_stack.insert(neighbor.to_string(), vec![node.to_string()]);
                    if let Some(cycle) = Self::dfs_visit(neighbor, adj, white, gray, &mut new_stack) {
                        return Some(cycle);
                    }
                }
            }
        }

        gray.remove(node);
        None
    }
}

#[must_use]
pub fn check_cycles(graph: &DagGraph) -> Vec<Diagnostic> {
    match CycleDetector::check(graph) {
        Ok(()) => Vec::new(),
        Err(cycle_error) => {
            let cycle_str = cycle_error.path.join("->");
            vec![Diagnostic::new(LintCode::L008, format!("cycle detected: {}", cycle_str))
                .with_severity(Severity::Error)
                .with_suggestion("Remove the cyclic dependency to make the workflow a valid DAG")]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acyclic_graph_returns_ok() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_step(Step { name: "c".into(), is_entry: false })
            .add_edge("a", "b")
            .add_edge("b", "c");

        let result = CycleDetector::check(&graph);
        assert!(result.is_ok(), "acyclic graph should return Ok");
    }

    #[test]
    fn test_simple_self_referential_cycle() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_edge("a", "a");

        let result = CycleDetector::check(&graph);
        assert!(result.is_err(), "self-referential node should return Err");
        let err = result.unwrap_err();
        assert!(err.path.contains(&"a".to_string()));
    }

    #[test]
    fn test_two_node_cycle() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_edge("a", "b")
            .add_edge("b", "a");

        let result = CycleDetector::check(&graph);
        assert!(result.is_err(), "A->B->A cycle should return Err");
        let err = result.unwrap_err();
        assert!(err.path.contains(&"a".to_string()));
        assert!(err.path.contains(&"b".to_string()));
    }

    #[test]
    fn test_three_node_cycle_abc() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_step(Step { name: "c".into(), is_entry: false })
            .add_edge("a", "b")
            .add_edge("b", "c")
            .add_edge("c", "a");

        let result = CycleDetector::check(&graph);
        assert!(result.is_err(), "A->B->C->A cycle should return Err");
        let err = result.unwrap_err();
        assert!(err.path.contains(&"a".to_string()));
        assert!(err.path.contains(&"b".to_string()));
        assert!(err.path.contains(&"c".to_string()));
    }

    #[test]
    fn test_empty_graph_returns_ok() {
        let graph = DagGraph::new();
        let result = CycleDetector::check(&graph);
        assert!(result.is_ok(), "empty graph should return Ok");
    }

    #[test]
    fn test_single_node_no_edges_returns_ok() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true });

        let result = CycleDetector::check(&graph);
        assert!(result.is_ok(), "single node with no edges should return Ok");
    }

    #[test]
    fn test_diamond_dag_no_cycle() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_step(Step { name: "c".into(), is_entry: false })
            .add_step(Step { name: "d".into(), is_entry: false })
            .add_edge("a", "b")
            .add_edge("a", "c")
            .add_edge("b", "d")
            .add_edge("c", "d");

        let result = CycleDetector::check(&graph);
        assert!(result.is_ok(), "diamond DAG should not have cycles");
    }

    #[test]
    fn test_linear_chain_no_cycle() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_step(Step { name: "c".into(), is_entry: false })
            .add_step(Step { name: "d".into(), is_entry: false })
            .add_edge("a", "b")
            .add_edge("b", "c")
            .add_edge("c", "d");

        let result = CycleDetector::check(&graph);
        assert!(result.is_ok(), "linear chain should not have cycles");
    }

    #[test]
    fn test_cycle_diagnostic_contains_error_severity() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_edge("a", "a");

        let diags = check_cycles(&graph);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].code, LintCode::L008);
    }

    #[test]
    fn test_no_cycle_diagnostic_returns_empty() {
        let graph = DagGraph::new()
            .add_step(Step { name: "a".into(), is_entry: true })
            .add_step(Step { name: "b".into(), is_entry: false })
            .add_edge("a", "b");

        let diags = check_cycles(&graph);
        assert!(diags.is_empty());
    }
}