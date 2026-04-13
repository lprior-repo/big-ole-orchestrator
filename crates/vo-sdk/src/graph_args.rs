//! `--graph` CLI argument handling and workflow specification types (ADR-004, ADR-009).

use std::collections::HashSet;
use std::fmt;
use std::io::Write;

use serde::{Deserialize, Serialize};
pub use vo_types::NodeKind;
use vo_types::{NodeName, WorkflowName};

/// Detect if a graph has cycles using DFS.
///
/// # Arguments
///
/// * `nodes` - List of nodes in the graph
/// * `edges` - List of edges in the graph
///
/// # Returns
///
/// `true` if the graph contains a cycle, `false` otherwise.
fn has_cycle(nodes: &[NodeSpec], edges: &[EdgeSpec]) -> bool {
    if edges.is_empty() {
        return false;
    }

    // Build adjacency list
    let node_names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    let name_to_idx: std::collections::HashMap<&str, usize> = node_names
        .iter()
        .enumerate()
        .map(|(i, &n)| (n, i))
        .collect();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in edges {
        if let (Some(&from_idx), Some(&to_idx)) = (
            name_to_idx.get(edge.from.as_str()),
            name_to_idx.get(edge.to.as_str()),
        ) {
            adj[from_idx].push(to_idx);
        }
    }

    // DFS-based cycle detection
    const WHITE: u8 = 0; // Not visited
    const GRAY: u8 = 1; // Currently visiting
    const BLACK: u8 = 2; // Finished visiting

    let mut colors = vec![WHITE; nodes.len()];

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

    for i in 0..nodes.len() {
        if colors[i] == WHITE && dfs(i, &adj, &mut colors) {
            return true;
        }
    }

    false
}

/// Marker returned when `--graph` flag is present.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct GraphArgs;

/// Errors from parsing `--graph` arguments.
#[derive(Debug, PartialEq)]
pub enum GraphArgsError {
    /// An unrecognized argument was found alongside `--graph`.
    UnrecognizedArgument { arg: String },
    /// No `--graph` flag was found.
    NoGraphFlag,
}

impl fmt::Display for GraphArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrecognizedArgument { arg } => write!(f, "unrecognized argument: {arg}"),
            Self::NoGraphFlag => write!(f, "no --graph flag found"),
        }
    }
}

impl std::error::Error for GraphArgsError {}

/// Parse CLI arguments for the `--graph` flag.
///
/// # Errors
///
/// Returns `GraphArgsError::NoGraphFlag` when `--graph` is absent.
/// Returns `GraphArgsError::UnrecognizedArgument` when extra positional args follow `--graph`.
pub fn parse_graph_args(args: &[String]) -> Result<GraphArgs, GraphArgsError> {
    let mut found_graph = false;
    for arg in args.iter().skip(1) {
        if arg == "--graph" {
            found_graph = true;
        } else if found_graph {
            return Err(GraphArgsError::UnrecognizedArgument { arg: arg.clone() });
        }
    }
    if found_graph {
        Ok(GraphArgs)
    } else {
        Err(GraphArgsError::NoGraphFlag)
    }
}

/// Specification of a single workflow node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub name: NodeName,
    pub kind: NodeKind,
}

/// Specification of an edge between two workflow nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub from: NodeName,
    pub to: NodeName,
}

/// Internal representation for deserialization (bypasses validation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WorkflowSpecRaw {
    workflow_name: WorkflowName,
    nodes: Vec<NodeSpec>,
    edges: Vec<EdgeSpec>,
    version: u32,
}

/// Full workflow graph specification produced by `--graph` (ADR-004, ADR-009, ADR-031).
///
/// This is the canonical workflow representation emitted by the SDK when
/// `./binary --graph` is invoked. The Engine validates, hashes, and stores
/// this spec as a workflow version.
///
/// Validation is enforced on deserialization to prevent invalid specs from
/// being created via serde bypass.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowSpec {
    pub workflow_name: WorkflowName,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
    pub version: u32,
}

impl<'de> Deserialize<'de> for WorkflowSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = WorkflowSpecRaw::deserialize(deserializer)?;

        // Validate all node names
        for node in &raw.nodes {
            NodeName::parse(node.name.as_str()).map_err(serde::de::Error::custom)?;
        }

        // Validate all edge references exist
        let node_names: HashSet<&str> = raw.nodes.iter().map(|n| n.name.as_str()).collect();
        for edge in &raw.edges {
            if !node_names.contains(edge.from.as_str()) {
                return Err(serde::de::Error::custom(format!(
                    "edge from {} to {} references non-existent node",
                    edge.from, edge.to
                )));
            }
            if !node_names.contains(edge.to.as_str()) {
                return Err(serde::de::Error::custom(format!(
                    "edge from {} to {} references non-existent node",
                    edge.from, edge.to
                )));
            }
        }

        // Validate no cycles
        if has_cycle(&raw.nodes, &raw.edges) {
            return Err(serde::de::Error::custom("workflow contains a cycle"));
        }

        Ok(WorkflowSpec {
            workflow_name: raw.workflow_name,
            nodes: raw.nodes,
            edges: raw.edges,
            version: raw.version,
        })
    }
}

impl WorkflowSpec {
    /// Serialize to JSON bytes for `--graph` emission.
    #[must_use]
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("WorkflowSpec is always serializable")
    }

    /// Create a new `WorkflowSpec` with validation.
    ///
    /// This constructor validates:
    /// - All node names are valid per `NodeName::parse` rules
    /// - All edge references exist in the nodes list
    /// - The graph is acyclic (no cycles allowed)
    ///
    /// # Errors
    ///
    /// Returns `WorkflowSpecError` if validation fails.
    pub fn new(
        workflow_name: WorkflowName,
        nodes: Vec<NodeSpec>,
        edges: Vec<EdgeSpec>,
        version: u32,
    ) -> Result<Self, WorkflowSpecError> {
        // Validate all node names
        for node in &nodes {
            NodeName::parse(node.name.as_str())?;
        }

        // Validate all edge references exist
        let node_names: std::collections::HashSet<&str> =
            nodes.iter().map(|n| n.name.as_str()).collect();
        for edge in &edges {
            if !node_names.contains(edge.from.as_str()) {
                return Err(WorkflowSpecError::EdgeReferenceNotFound {
                    edge_from: edge.from.clone(),
                    edge_to: edge.to.clone(),
                });
            }
            if !node_names.contains(edge.to.as_str()) {
                return Err(WorkflowSpecError::EdgeReferenceNotFound {
                    edge_from: edge.from.clone(),
                    edge_to: edge.to.clone(),
                });
            }
        }

        // Validate no cycles
        if has_cycle(&nodes, &edges) {
            return Err(WorkflowSpecError::CycleDetected);
        }

        Ok(Self {
            workflow_name,
            nodes,
            edges,
            version,
        })
    }

    /// Get the default version for new specs.
    #[must_use]
    pub fn default_version() -> u32 {
        1
    }
}

/// Errors that can occur when constructing a `WorkflowSpec`.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowSpecError {
    /// A node name is invalid.
    InvalidNodeName { name: String },
    /// An edge references a node that doesn't exist.
    EdgeReferenceNotFound {
        edge_from: NodeName,
        edge_to: NodeName,
    },
    /// The graph contains a cycle.
    CycleDetected,
    /// The workflow has no nodes.
    EmptyWorkflow,
}

impl From<vo_types::ParseError> for WorkflowSpecError {
    fn from(err: vo_types::ParseError) -> Self {
        let type_name = match &err {
            vo_types::ParseError::Empty { type_name }
            | vo_types::ParseError::InvalidCharacters { type_name, .. }
            | vo_types::ParseError::InvalidFormat { type_name, .. }
            | vo_types::ParseError::ExceedsMaxLength { type_name, .. }
            | vo_types::ParseError::BoundaryViolation { type_name, .. }
            | vo_types::ParseError::ConsecutiveHyphens { type_name }
            | vo_types::ParseError::ConsecutiveSeparators { type_name }
            | vo_types::ParseError::NotAnInteger { type_name, .. }
            | vo_types::ParseError::ZeroValue { type_name }
            | vo_types::ParseError::OutOfRange { type_name, .. } => type_name,
        };
        WorkflowSpecError::InvalidNodeName {
            name: format!("{type_name}: {err}"),
        }
    }
}

impl std::fmt::Display for WorkflowSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNodeName { name } => write!(f, "invalid node name: {name}"),
            Self::EdgeReferenceNotFound { edge_from, edge_to } => {
                write!(
                    f,
                    "edge from {edge_from} to {edge_to} references non-existent node"
                )
            }
            Self::CycleDetected => write!(f, "workflow contains a cycle"),
            Self::EmptyWorkflow => write!(f, "workflow has no nodes"),
        }
    }
}

impl std::error::Error for WorkflowSpecError {}

/// Full workflow graph specification produced by `--graph`.
#[deprecated(
    since = "0.1.0",
    note = "Use `WorkflowSpec` instead. This type is kept for backward compatibility."
)]
pub type GraphWorkflowSpec = WorkflowSpec;

/// Emit the workflow spec as JSON to stdout and exit.
///
/// This function is called when the binary is invoked with `--graph`.
/// It serializes the `WorkflowSpec` to JSON, prints it to stdout,
/// and exits with code 0.
///
/// # Example
///
/// ```ignore
/// fn main() {
///     let args: Vec<String> = std::env::args().collect();
///     if let Err(()) = vo_sdk::emit_graph_if_requested(&args, workflow_spec) {
///         std::process::exit(1);
///     }
/// }
/// ```
///
/// # Errors
///
/// Returns `()` if `--graph` was not present. If `--graph` was present,
/// this function always terminates the process.
#[allow(clippy::result_unit_err)]
pub fn emit_graph_if_requested(args: &[String], spec: &WorkflowSpec) -> Result<(), ()> {
    match parse_graph_args(args) {
        Ok(_graph_args) => {
            let json = spec.to_json_bytes();
            std::io::stdout()
                .write_all(&json)
                .expect("stdout write should not fail");
            std::process::exit(0);
        }
        Err(GraphArgsError::NoGraphFlag) => Ok(()),
        Err(e) => {
            eprintln!("error: {e}");
            Err(())
        }
    }
}
