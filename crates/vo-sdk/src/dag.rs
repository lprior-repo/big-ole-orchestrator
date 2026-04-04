//! Dag: compile-time type-safe workflow graph construction (ADR-010).

use std::any::Any;

use vo_types::NodeName;

use crate::node_handle::NodeHandle;

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
    /// The closure `f` is accepted but not stored -- this is the builder
    /// phase; execution happens later via the engine's subprocess model.
    ///
    /// # Panics
    /// Panics if `name` is not a valid `NodeName`.
    pub fn add_node<I, O, F>(&mut self, name: &str, _f: F) -> NodeHandle<I, O> {
        let node_name = NodeName::parse(name)
            .unwrap_or_else(|_| panic!("invalid node name: {name}"));
        self.node_names.push(node_name.clone());
        NodeHandle::new(node_name)
    }

    /// Connect two nodes with compile-time type safety.
    ///
    /// The output type `T` of `from` must match the input type `T` of `to`.
    /// If types don't align, the program will not compile.
    ///
    /// ```compile_fail
    /// // This MUST not compile: String output != i32 input
    /// use vo_sdk::dag::Dag;
    /// let mut dag = Dag::new();
    /// let a = dag.add_node("a", |_: i32| -> String { String::new() });
    /// let b = dag.add_node("b", |_: bool| -> () {});
    /// dag.connect(&a, &b); // ERROR: expected String, found bool
    /// ```
    pub fn connect<T>(
        &mut self,
        from: &NodeHandle<impl Any, T>,
        to: &NodeHandle<T, impl Any>,
    ) {
        let from_idx = self.find_index(from.name());
        let to_idx = self.find_index(to.name());
        self.edges.push((from_idx, to_idx));
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

    fn find_index(&self, name: &str) -> usize {
        self.node_names
            .iter()
            .position(|n| n.as_str() == name)
            .unwrap_or_else(|| panic!("node not found: {name}"))
    }
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}
