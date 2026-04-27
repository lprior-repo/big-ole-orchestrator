//! Dag: compile-time type-safe workflow graph construction (ADR-010).
//!
//! The [`Workflow`] struct provides a fluent builder API for constructing
//! workflow graphs. After building with [`Workflow::build`], a validated
//! [`WorkflowSpec`](crate::graph_args::WorkflowSpec) is emitted.

use std::any::Any;

use thiserror::Error;
use vo_types::{BufferPolicy, DedupeScope, GuaranteeClass, LineageScope, NodeKind, NodeName, RetryPolicy, WorkflowName};

use crate::graph::{EdgeSpec, NodeSpec, SignalNodeMeta, WorkflowSpec};
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
    #[error("cycle detected: {cycle}")]
    CycleDetected { cycle: String },
    #[error("duplicate node name: {name}")]
    DuplicateNodeName { name: String },
    #[error("self-loop not allowed on node: {name}")]
    SelfLoop { name: String },
}

/// Internal node record with name, kind, and optional signal metadata.
#[derive(Debug, Clone)]
struct DagNodeRecord {
    name: NodeName,
    kind: NodeKind,
    signal_meta: Option<SignalNodeMeta>,
}

/// A directed acyclic graph of typed workflow nodes.
///
/// Nodes are registered via `add_node` and connected via `connect`.
/// The `connect` method enforces at compile time that the output type
/// of the source node matches the input type of the target node.
///
/// # Example
///
/// ```
/// use vo_sdk::dag::Dag;
/// use vo_sdk::node_handle::NodeHandle;
/// use vo_sdk::graph::NodeKind;
///
/// let mut dag = Dag::new();
/// let a: NodeHandle<(), i32> = dag.add_node_with_kind("a", NodeKind::Pure, |_input: ()| -> i32 { 42 }).unwrap();
/// let b: NodeHandle<i32, i32> = dag.add_node_with_kind("b", NodeKind::ManagedEffect, |x: i32| -> i32 { x + 1 }).unwrap();
/// dag.connect(&a, &b).unwrap();
/// assert_eq!(dag.node_count(), 2);
/// assert_eq!(dag.edge_count(), 1);
/// assert_eq!(dag.edges(), vec![("a", "b")]);
/// ```
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
        if self.nodes.iter().any(|n| n.name == node_name) {
            return Err(DagError::DuplicateNodeName {
                name: name.to_string(),
            });
        }
        self.nodes.push(DagNodeRecord {
            name: node_name.clone(),
            kind,
            signal_meta: None,
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

    /// Set signal metadata on the most recently added node.
    ///
    /// This is a no-op for non-signal/wait nodes.
    pub fn set_signal_meta(&mut self, meta: SignalNodeMeta) {
        if let Some(last) = self.nodes.last_mut() {
            if matches!(last.kind, NodeKind::Signal | NodeKind::Wait) {
                last.signal_meta = Some(meta);
            }
        }
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
        if from.name() == to.name() {
            return Err(DagError::SelfLoop {
                name: from.name().to_string(),
            });
        }
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

    fn find_cycle_nodes(nodes: &[DagNodeRecord], edges: &[(usize, usize)]) -> String {
        let n = nodes.len();
        let mut visited = vec![0u8; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut cycle_path: Vec<usize> = Vec::new();

        fn dfs(
            node: usize,
            edges: &[(usize, usize)],
            visited: &mut [u8],
            stack: &mut Vec<usize>,
            cycle_path: &mut Vec<usize>,
        ) -> bool {
            visited[node] = 1;
            stack.push(node);
            for &(_, to) in edges.iter().filter(|&&(_from, _)| _from == node) {
                if visited[to] == 0 {
                    if dfs(to, edges, visited, stack, cycle_path) {
                        return true;
                    }
                } else if visited[to] == 1 {
                    if let Some(pos) = stack.iter().position(|&x| x == to) {
                        let cycle: Vec<usize> = stack[pos..].to_vec();
                        cycle_path.extend(cycle);
                        return true;
                    }
                }
            }
            stack.pop();
            visited[node] = 2;
            false
        }

        for i in 0..n {
            if visited[i] == 0 && dfs(i, edges, &mut visited, &mut stack, &mut cycle_path) {
                break;
            }
        }

        if cycle_path.is_empty() {
            return "unknown cycle".to_string();
        }

        let cycle_names: Vec<String> = cycle_path
            .iter()
            .map(|&idx| nodes[idx].name.as_str().to_string())
            .collect();
        cycle_names.join(" -> ")
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
            let cycle_nodes = Self::find_cycle_nodes(&self.nodes, &self.edges);
            return Err(DagError::CycleDetected { cycle: cycle_nodes });
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
                retry_policy: RetryPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                    backoff_multiplier: 1.0,
                    max_backoff_ms: u64::MAX,
                },
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
            dedupe_scope: DedupeScope::default(),
            guarantee_class: GuaranteeClass::default(),
        })
    }

    /// Check if the DAG contains a cycle.
    ///
    /// Uses DFS-based cycle detection with coloring:
    /// - WHITE: not visited
    /// - GRAY: currently visiting
    /// - BLACK: finished visiting
    ///
    /// A back edge to a GRAY node indicates a cycle.
    #[allow(dead_code)]
    fn has_cycle(&self) -> bool {
        if self.edges.is_empty() {
            return false;
        }

        const WHITE: u8 = 0;
        const GRAY: u8 = 1;
        const BLACK: u8 = 2;

        let mut colors = vec![WHITE; self.nodes.len()];

        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (from, to) in &self.edges {
            adj[*from].push(*to);
        }

        fn dfs(node: usize, adj: &[Vec<usize>], colors: &mut [u8]) -> bool {
            colors[node] = GRAY;
            for &neighbor in &adj[node] {
                if colors[neighbor] == GRAY {
                    return true; // Back edge found, cycle exists
                }
                if colors[neighbor] == WHITE && dfs(neighbor, adj, colors) {
                    return true;
                }
            }
            colors[node] = BLACK;
            false
        }

        for i in 0..self.nodes.len() {
            if colors[i] == WHITE && dfs(i, &adj, &mut colors) {
                return true;
            }
        }

        false
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}

/// Fluent builder for constructing workflows (ADR-004).
///
/// # Compile-tested Examples
///
/// ## Basic Workflow with Pure and Effect Nodes
///
/// ```
/// use vo_sdk::Workflow;
///
/// let mut wf = Workflow::new("checkout");
/// let validate = wf.pure("validate", |input: String| -> bool {
///     !input.is_empty()
/// }).unwrap();
/// let charge = wf.effect("charge", |input: bool| -> Result<String, String> {
///     if input {
///         Ok("charged".to_string())
///     } else {
///         Err("invalid".to_string())
///     }
/// }).unwrap();
/// wf.connect(&validate, &charge).unwrap();
/// let spec = wf.build().unwrap();
/// assert_eq!(spec.workflow_name.as_str(), "checkout");
/// assert_eq!(spec.nodes.len(), 2);
/// assert_eq!(spec.edges.len(), 1);
/// ```
///
/// ## Workflow with Wait Node
///
/// ```
/// use vo_sdk::Workflow;
///
/// let mut wf = Workflow::new("async-workflow");
/// let fetch = wf.pure("fetch", |_input: ()| -> Vec<u8> {
///     vec![1, 2, 3]
/// }).unwrap();
/// let process = wf.wait("process", |input: Vec<u8>| -> String {
///     format!("processed {} bytes", input.len())
/// }).unwrap();
/// wf.connect(&fetch, &process).unwrap();
/// let spec = wf.build().unwrap();
/// assert_eq!(spec.nodes.len(), 2);
/// ```
///
/// ## Workflow with Signal Node
///
/// ```
/// use vo_sdk::Workflow;
/// use vo_sdk::graph::SignalNodeMeta;
///
/// let mut wf = Workflow::new("signal-flow");
/// let trigger = wf.signal("trigger", |_input: ()| -> bool {
///     true
/// }).unwrap();
/// let wait = wf.wait_with_meta(
///     "wait",
///     |_input: bool| -> String {
///         "done".to_string()
///     },
///     SignalNodeMeta { signal_name: Some("external-event".to_string()), timeout_ms: Some(5000) },
/// ).unwrap();
/// wf.connect(&trigger, &wait).unwrap();
/// let spec = wf.build().unwrap();
/// assert_eq!(spec.nodes.len(), 2);
/// ```
///
/// ## Type-Safe Node Connections
///
/// The type parameters enforce that only compatible nodes can be connected:
/// ```
/// use vo_sdk::Workflow;
///
/// let mut wf = Workflow::new("typed-flow");
/// let string_node = wf.pure("string_node", |_input: ()| -> String {
///     "hello".to_string()
/// }).unwrap();
/// let len_node = wf.pure("len_node", |s: String| -> usize {
///     s.len()
/// }).unwrap();
/// wf.connect(&string_node, &len_node).unwrap();
/// let spec = wf.build().unwrap();
/// assert_eq!(spec.edges[0].from.as_str(), "string_node");
/// assert_eq!(spec.edges[0].to.as_str(), "len_node");
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

    /// Add a wait node with signal metadata to the workflow.
    pub fn wait_with_meta<I, O, F>(
        &mut self,
        name: &str,
        f: F,
        meta: SignalNodeMeta,
    ) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        let handle = self.dag.add_node_with_kind(name, NodeKind::Wait, f)?;
        self.dag.set_signal_meta(meta);
        Ok(handle)
    }

    /// Add a signal node to the workflow.
    pub fn signal<I, O, F>(&mut self, name: &str, f: F) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        self.dag.add_node_with_kind(name, NodeKind::Signal, f)
    }

    /// Add a signal node with signal metadata to the workflow.
    pub fn signal_with_meta<I, O, F>(
        &mut self,
        name: &str,
        f: F,
        meta: SignalNodeMeta,
    ) -> Result<NodeHandle<I, O>, DagError>
    where
        F: Fn(I) -> O + 'static,
    {
        let handle = self.dag.add_node_with_kind(name, NodeKind::Signal, f)?;
        self.dag.set_signal_meta(meta);
        Ok(handle)
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
