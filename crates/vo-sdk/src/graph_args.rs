//! `--graph` CLI argument handling and workflow specification types (ADR-004, ADR-009).

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

/// Full workflow graph specification produced by `--graph` (ADR-004, ADR-009, ADR-031).
///
/// This is the canonical workflow representation emitted by the SDK when
/// `./binary --graph` is invoked. The Engine validates, hashes, and stores
/// this spec as a workflow version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub workflow_name: WorkflowName,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
}

impl WorkflowSpec {
    /// Serialize to JSON bytes for `--graph` emission.
    #[must_use]
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("WorkflowSpec is always serializable")
    }
}

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
