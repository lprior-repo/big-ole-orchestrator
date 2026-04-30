//! Directed acyclic graph representation for workflow step analysis.
//!
//! This module defines a minimal DAG structure used by the unused-step
//! linting rule to detect orphaned workflow nodes.

use std::collections::HashMap;

/// A named workflow step in the DAG.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Step {
    pub name: String,
    pub is_entry: bool,
}

/// A directed edge from one step to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// A directed acyclic graph of workflow steps.
///
/// The graph is defined by a set of named nodes and directed edges between them.
/// The entry node is identified by the `is_entry` flag on a [`Step`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DagGraph {
    pub steps: Vec<Step>,
    pub edges: Vec<Edge>,
}

impl DagGraph {
    /// Create a new empty DAG.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a step to the graph.
    #[must_use]
    pub fn add_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Add a directed edge to the graph.
    #[must_use]
    pub fn add_edge(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.edges.push(Edge {
            from: from.into(),
            to: to.into(),
        });
        self
    }

    /// Build an adjacency list from the edges.
    #[must_use]
    pub fn adjacency_list(&self) -> HashMap<String, Vec<String>> {
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for step in &self.steps {
            adj.entry(step.name.clone()).or_default();
        }
        for edge in &self.edges {
            adj.entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
        }
        adj
    }

    /// Find the entry node (the step with `is_entry == true`).
    /// Returns None if no entry node is defined.
    #[must_use]
    pub fn entry_node(&self) -> Option<String> {
        self.steps
            .iter()
            .find(|s| s.is_entry)
            .map(|s| s.name.clone())
    }

    /// Get the set of all node names.
    #[must_use]
    pub fn node_names(&self) -> std::collections::HashSet<&str> {
        self.steps.iter().map(|s| s.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_graph_new_is_empty() {
        let graph = DagGraph::new();
        assert!(graph.steps.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_dag_graph_add_step() {
        let graph = DagGraph::new()
            .add_step(Step {
                name: "a".to_string(),
                is_entry: true,
            })
            .add_step(Step {
                name: "b".to_string(),
                is_entry: false,
            });
        assert_eq!(graph.steps.len(), 2);
    }

    #[test]
    fn test_dag_graph_add_edge() {
        let graph = DagGraph::new()
            .add_step(Step {
                name: "a".to_string(),
                is_entry: true,
            })
            .add_step(Step {
                name: "b".to_string(),
                is_entry: false,
            })
            .add_edge("a", "b");
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "a");
        assert_eq!(graph.edges[0].to, "b");
    }

    #[test]
    fn test_entry_node_returns_entry() {
        let graph = DagGraph::new().add_step(Step {
            name: "entry".to_string(),
            is_entry: true,
        });
        assert_eq!(graph.entry_node(), Some("entry".to_string()));
    }

    #[test]
    fn test_entry_node_none_when_no_entry() {
        let graph = DagGraph::new().add_step(Step {
            name: "no_entry".to_string(),
            is_entry: false,
        });
        assert!(graph.entry_node().is_none());
    }

    #[test]
    fn test_adjacency_list() {
        let graph = DagGraph::new()
            .add_step(Step {
                name: "a".to_string(),
                is_entry: false,
            })
            .add_step(Step {
                name: "b".to_string(),
                is_entry: false,
            })
            .add_step(Step {
                name: "c".to_string(),
                is_entry: false,
            })
            .add_edge("a", "b")
            .add_edge("a", "c");
        let adj = graph.adjacency_list();
        assert_eq!(
            adj.get("a").map(|v| v.as_slice()),
            Some(&["b".to_string(), "c".to_string()][..])
        );
    }
}
