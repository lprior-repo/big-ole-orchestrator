//! Graph emission: `--graph` CLI argument handling and workflow specification types (ADR-004, ADR-009).
//!
//! This module provides the types and functions needed to emit workflow graph
//! specifications when a binary is invoked with `--graph`. The Engine validates,
//! hashes, and stores this spec as a workflow version.

use std::io::Write;

use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use vo_types::NodeKind;
use vo_types::{NodeName, WorkflowName};

/// Marker returned when `--graph` flag is present.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct GraphArgs;

/// Errors from parsing `--graph` arguments.
#[derive(Debug, PartialEq, Error)]
pub enum GraphArgsError {
    #[error("unrecognized argument: {arg}")]
    UnrecognizedArgument { arg: String },
    #[error("no --graph flag found")]
    NoGraphFlag,
}

/// Parse CLI arguments for the `--graph` flag.
///
/// # Errors
///
/// Returns `GraphArgsError::NoGraphFlag` when `--graph` is absent.
/// Returns `GraphArgsError::UnrecognizedArgument` when extra positional args follow `--graph` or when `--graph` appears twice.
pub fn parse_graph_args(args: &[String]) -> Result<GraphArgs, GraphArgsError> {
    let mut found_graph = false;
    for arg in args.iter().skip(1) {
        if arg == "--graph" {
            if found_graph {
                return Err(GraphArgsError::UnrecognizedArgument { arg: arg.clone() });
            }
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

/// Full workflow graph specification produced by `--graph` (ADR-004, ADR-009, ADR-031).
///
/// This is the canonical workflow representation emitted by the SDK when
/// `./binary --graph` is invoked. The Engine validates, hashes, and stores
/// this spec as a workflow version.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowSpec {
    pub workflow_name: WorkflowName,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
}

impl<'de> serde::Deserialize<'de> for WorkflowSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct RawWorkflowSpec {
            workflow_name: WorkflowName,
            nodes: Vec<NodeSpec>,
            edges: Vec<EdgeSpec>,
        }

        let raw: RawWorkflowSpec = RawWorkflowSpec::deserialize(deserializer)?;

        let node_names: std::collections::HashSet<&str> =
            raw.nodes.iter().map(|n| n.name.as_str()).collect();

        for edge in &raw.edges {
            if edge.from == edge.to {
                return Err(serde::de::Error::custom(format!(
                    "workflow contains a cycle: self-loop edge on {}",
                    edge.from.as_str()
                )));
            }
            if !node_names.contains(edge.from.as_str()) {
                return Err(serde::de::Error::custom(format!(
                    "edge references non-existent node: {}",
                    edge.from.as_str()
                )));
            }
            if !node_names.contains(edge.to.as_str()) {
                return Err(serde::de::Error::custom(format!(
                    "edge references non-existent node: {}",
                    edge.to.as_str()
                )));
            }
        }

        let name_to_idx: std::collections::HashMap<&str, usize> = raw
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();

        let n = raw.nodes.len();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for edge in &raw.edges {
            if let (Some(&from), Some(&to)) = (
                name_to_idx.get(edge.from.as_str()),
                name_to_idx.get(edge.to.as_str()),
            ) {
                adj[from].push(to);
            }
        }

        const WHITE: u8 = 0;
        const GRAY: u8 = 1;
        let mut colors = vec![WHITE; n];

        fn has_cycle_from(node: usize, adj: &[Vec<usize>], colors: &mut [u8]) -> bool {
            colors[node] = GRAY;
            for &neighbor in &adj[node] {
                if colors[neighbor] == GRAY {
                    return true;
                }
                if colors[neighbor] == WHITE && has_cycle_from(neighbor, adj, colors) {
                    return true;
                }
            }
            colors[node] = 2;
            false
        }

        for i in 0..n {
            if colors[i] == WHITE && has_cycle_from(i, &adj, &mut colors) {
                return Err(serde::de::Error::custom("workflow contains a cycle"));
            }
        }

        Ok(WorkflowSpec {
            workflow_name: raw.workflow_name,
            nodes: raw.nodes,
            edges: raw.edges,
        })
    }
}

impl WorkflowSpec {
    /// Serialize to JSON bytes for `--graph` emission.
    #[must_use]
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("WorkflowSpec is always serializable")
    }

    pub(crate) fn detect_cycle(&self) -> Option<String> {
        let n = self.nodes.len();
        if n == 0 {
            return None;
        }
        let name_to_idx: std::collections::HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();
        let edges: Vec<(usize, usize)> = self
            .edges
            .iter()
            .filter_map(|e| {
                let from_idx = name_to_idx.get(e.from.as_str()).copied();
                let to_idx = name_to_idx.get(e.to.as_str()).copied();
                from_idx.and_then(|f| to_idx.map(|t| (f, t)))
            })
            .collect();
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
            if visited[i] == 0 && dfs(i, &edges, &mut visited, &mut stack, &mut cycle_path) {
                break;
            }
        }

        if cycle_path.is_empty() {
            return None;
        }

        let cycle_names: Vec<String> = cycle_path
            .iter()
            .map(|&idx| self.nodes[idx].name.as_str().to_string())
            .collect();
        Some(cycle_names.join(" -> "))
    }
}

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
            if let Some(cycle) = spec.detect_cycle() {
                eprintln!("error: cycle detected: {}", cycle);
                std::process::exit(1);
            }
            let json = spec.to_json_bytes();
            if let Err(e) = std::io::stdout().write_all(&json) {
                eprintln!("error: failed to write graph output: {e}");
                std::process::exit(1);
            }
            std::process::exit(0);
        }
        Err(GraphArgsError::NoGraphFlag) => Ok(()),
        Err(e) => {
            eprintln!("error: {e}");
            Err(())
        }
    }
}
