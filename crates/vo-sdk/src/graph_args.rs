//! `--graph` CLI argument handling and workflow specification types (ADR-004, ADR-009).

use std::fmt;

use serde::{Deserialize, Serialize};
pub use vo_types::NodeKind;
use vo_types::{NodeName, WorkflowName};

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

/// Full workflow graph specification produced by `--graph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphWorkflowSpec {
    pub workflow_name: WorkflowName,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
}
