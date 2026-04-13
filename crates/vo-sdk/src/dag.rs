//! Dag: compile-time type-safe workflow graph construction (ADR-010).
//!
//! The [`Workflow`] struct provides a fluent builder API for constructing
//! workflow graphs. After building with [`Workflow::build`], a validated
//! [`WorkflowSpec`](crate::graph_args::WorkflowSpec) is emitted.

use std::any::Any;

use thiserror::Error;
use vo_types::{NodeKind, NodeName, WorkflowName};

use crate::graph_args::{EdgeSpec, NodeSpec, WorkflowSpec};
use crate::node_handle::NodeHandle;

/// Errors that can occur when building a DAG.
#[derive(Debug, PartialEq, Clone, Error)]
pub enum DagError {
    #[error("invalid node name: {name}")]
    InvalidNodeName { name: String },
    #[error("node not found: {name}")]
    NodeNotFound { name: String },
    #[error("workflow has no nodes")]
    EmptyWorkflow,
    #[error("workflow contains a cycle")]
    CycleDetected,
}

/// Internal node record with name and kind.
#[derive(Debug, Clone)]
struct DagNodeRecord {
    name: NodeName,
    kind: NodeKind,
}

/// A directed acyclic graph of typed workflow nodes.
///
/// Nodes are registered via `add_node` and connected via `connect`.
/// The `connect` method enforces at compile time that the output type
/// of the source node matches the input type of the target node.
#[derive(Debug)]
pub struct Dag {
    nodes: Vec<DagNodeRecord>,
    edges: Vec<(usize, usize)>,
}

impl Dag {
    /// Create an empty DAG.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Register a node in the DAG with a given kind and return a typed handle.
    ///
    /// # Errors
    ///
    /// Returns `DagError::InvalidNodeName` if `name` cannot be parsed.
    pub fn add_node_with_kind<I, O, F>(
        &mut self,
        name: &str,
        kind: NodeKind,
        _f: F,
    ) -> Result<NodeHandle<I, O>, DagError> {
        let node_name = NodeName::parse(name).map_err(|_| DagError::InvalidNodeName {
            name: name.to_string(),
        })?;
        self.nodes.push(DagNodeRecord {
            name: node_name.clone(),
            kind,
        });
        Ok(NodeHandle::new(node_name))
    }

    /// Register a node in the DAG and return a typed handle.
    /// The node kind defaults to `NodeKind::Pure`.
    ///
    /// # Errors
    ///
    /// Returns `DagError::InvalidNodeName` if `name` cannot be parsed.
    #[deprecated(
        since = "0.1.0",
        note = "Use add_node_with_kind or Workflow builder instead"
    )]
    pub fn add_node<I, O, F>(&mut self, name: &str, _f: F) -> Result<NodeHandle<I, O>, DagError> {
        self.add_node_with_kind(name, NodeKind::Pure, _f)
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
        self.nodes.len()
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
                    self.nodes[*from].name.as_str(),
                    self.nodes[*to].name.as_str(),
                )
            })
            .collect()
    }

    fn find_index(&self, name: &str) -> Result<usize, DagError> {
        self.nodes
            .iter()
            .position(|n| n.name.as_str() == name)
            .ok_or_else(|| DagError::NodeNotFound {
                name: name.to_string(),
            })
    }

    /// Build a [`WorkflowSpec`] from this DAG.
    ///
    /// # Errors
    ///
    /// Returns `DagError::EmptyWorkflow` if the DAG has no nodes.
    /// Returns `DagError::CycleDetected` if the DAG contains a cycle.
    pub fn build(self, workflow_name: &str) -> Result<WorkflowSpec, DagError> {
        if self.nodes.is_empty() {
            return Err(DagError::EmptyWorkflow);
        }

        // Cycle detection via Kahn's algorithm (topological sort).
        // Failing test: dag_tests::build_detects_simple_cycle
        let n = self.nodes.len();
        let mut in_degree = vec![0u32; n];
        for &(_, to) in &self.edges {
            in_degree[to] += 1;
        }
        let mut queue: std::collections::VecDeque<usize> =
            (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut visited = 0usize;
        while let Some(node) = queue.pop_front() {
            visited += 1;
            for &(_, to) in self.edges.iter().filter(|&&(_from, _)| _from == node) {
                in_degree[to] -= 1;
                if in_degree[to] == 0 {
                    queue.push_back(to);
                }
            }
        }
        if visited != n {
            return Err(DagError::CycleDetected);
        }

        let wf_name =
            WorkflowName::parse(workflow_name).map_err(|_| DagError::InvalidNodeName {
                name: workflow_name.to_string(),
            })?;

        let node_specs: Vec<NodeSpec> = self
            .nodes
            .iter()
            .map(|n| NodeSpec {
                name: n.name.clone(),
                kind: n.kind,
            })
            .collect();

        let edge_specs: Vec<EdgeSpec> = self
            .edges
            .iter()
            .map(|(from, to)| EdgeSpec {
                from: self.nodes[*from].name.clone(),
                to: self.nodes[*to].name.clone(),
            })
            .collect();

        Ok(WorkflowSpec {
            workflow_name: wf_name,
            nodes: node_specs,
            edges: edge_specs,
        })
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}

/// Fluent builder for constructing workflows (ADR-004).
///
/// # Example
///
/// ```ignore
/// let mut wf = Workflow::new("checkout_flow");
///
/// let validate = wf.pure("validate", |input: Cart| -> ValidatedCart { ... });
/// let charge = wf.effect("charge", |input: ValidatedCart| -> Receipt { ... });
///
/// wf.connect(&validate, &charge);
///
/// let spec = wf.build().expect("valid workflow");
/// ```
#[derive(Debug)]
pub struct Workflow {
    dag: Dag,
    workflow_name: String,
}

impl Workflow {
    /// Create a new workflow with the given name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            dag: Dag::new(),
            workflow_name: name.to_string(),
        }
    }

    /// Add a pure (side-effect free) node to the workflow.
    pub fn pure<I, O, F>(&mut self, name: &str, f: F) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        self.dag.add_node_with_kind(name, NodeKind::Pure, f)
    }

    /// Add a managed-effect node to the workflow.
    pub fn effect<I, O, F>(&mut self, name: &str, f: F) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        self.dag
            .add_node_with_kind(name, NodeKind::ManagedEffect, f)
    }

    /// Add a wait node to the workflow.
    pub fn wait<I, O, F>(&mut self, name: &str, f: F) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        self.dag.add_node_with_kind(name, NodeKind::Wait, f)
    }

    /// Add a signal node to the workflow.
    pub fn signal<I, O, F>(&mut self, name: &str, f: F) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        self.dag.add_node_with_kind(name, NodeKind::Signal, f)
    }

    /// Add an unsafe node to the workflow.
    pub fn unsafe_node<I, O, F>(&mut self, name: &str, f: F) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        self.dag.add_node_with_kind(name, NodeKind::Unsafe, f)
    }

    /// Connect two nodes with compile-time type safety.
    ///
    /// # Errors
    ///
    /// Returns `DagError::NodeNotFound` if either node is not in the workflow.
    pub fn connect<T>(
        &mut self,
        from: &NodeHandle<impl Any, T>,
        to: &NodeHandle<T, impl Any>,
    ) -> Result<(), DagError> {
        self.dag.connect(from, to)
    }

    /// Build and return the validated [`WorkflowSpec`](crate::graph_args::WorkflowSpec).
    ///
    /// # Errors
    ///
    /// Returns `DagError::EmptyWorkflow` if the workflow has no nodes.
    pub fn build(self) -> Result<WorkflowSpec, DagError> {
        self.dag.build(&self.workflow_name)
    }
}
