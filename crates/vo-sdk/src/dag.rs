//! Dag: compile-time type-safe workflow graph construction (ADR-010).

use std::any::Any;
use std::fmt;

use vo_types::NodeName;

use crate::node_handle::NodeHandle;

/// Errors that can occur when building a DAG.
#[derive(Debug, PartialEq, Clone)]
pub enum DagError {
    /// The provided node name could not be parsed as a valid `NodeName`.
    InvalidNodeName { name: String },
    /// A node referenced in `connect` was not found in the DAG.
    NodeNotFound { name: String },
}

impl fmt::Display for DagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNodeName { name } => write!(f, "invalid node name: {name}"),
            Self::NodeNotFound { name } => write!(f, "node not found: {name}"),
        }
    }
}

impl std::error::Error for DagError {}

/// A directed acyclic graph of typed workflow nodes.
///
/// Nodes are registered via `add_node` and connected via `connect`.
/// The `connect` method enforces at compile time that the output type
/// of the source node matches the input type of the target node.
pub struct Dag {
    node_names: Vec<NodeName>,
    edges: Vec<(usize, usize)>,
}

impl Dag {
    /// Create an empty DAG.
    #[must_use]
    pub fn new() -> Self {
        Self {
            node_names: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Register a node in the DAG and return a typed handle.
    ///
    /// # Errors
    ///
    /// Returns `DagError::InvalidNodeName` if `name` cannot be parsed.
    pub fn add_node<I, O, F>(&mut self, name: &str, _f: F) -> Result<NodeHandle<I, O>, DagError> {
        let node_name = NodeName::parse(name).map_err(|_| DagError::InvalidNodeName {
            name: name.to_string(),
        })?;
        self.node_names.push(node_name.clone());
        Ok(NodeHandle::new(node_name))
    }

    /// Connect two nodes with compile-time type safety.
    ///
    /// # Errors
    ///
    /// Returns `DagError::NodeNotFound` if either node is not in the DAG.
    pub fn connect<T>(
        &mut self,
        from: &NodeHandle<impl Any, T>,
        to: &NodeHandle<T, impl Any>,
    ) -> Result<(), DagError> {
        let from_idx = self.find_index(from.name())?;
        let to_idx = self.find_index(to.name())?;
        self.edges.push((from_idx, to_idx));
        Ok(())
    }

    /// Number of registered nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.node_names.len()
    }

    /// Number of registered edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns the list of edges as (from_name, to_name) pairs.
    #[must_use]
    pub fn edges(&self) -> Vec<(&str, &str)> {
        self.edges
            .iter()
            .map(|(from, to)| {
                (
                    self.node_names[*from].as_str(),
                    self.node_names[*to].as_str(),
                )
            })
            .collect()
    }

    fn find_index(&self, name: &str) -> Result<usize, DagError> {
        self.node_names
            .iter()
            .position(|n| n.as_str() == name)
            .ok_or_else(|| DagError::NodeNotFound {
                name: name.to_string(),
            })
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}
