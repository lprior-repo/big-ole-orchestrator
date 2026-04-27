//! Execute module: `--execute-node <name>` CLI argument handling and node execution.
//!
//! ADR-009 defines two CLI contracts for workflow binaries:
//! 1. `--graph` - Discovery phase, emits WorkflowSpec JSON
//! 2. `--execute-node <name>` - Execution phase, runs a specific node
//!
//! This module provides:
//! - [`ExecuteArgs`] - Parsed execute arguments (node name)
//! - [`parse_execute_args`] - Parse `--execute-node <name>` from CLI args
//! - [`execute_node`] - Read input from FD3, execute node, write result to FD4
//!
//! # Full workflow binary example
//!
//! ```ignore
//! use vo_sdk::{Workflow, emit_graph_if_requested, execute_node, BoxedNodeFn};
//!
//! fn main() {
//!     let args: Vec<String> = std::env::args().collect::<Vec<_>>();
//!
//!     let mut wf = Workflow::new("checkout");
//!     let _charge = wf.effect("charge", |_input: serde_json::Value| -> serde_json::Value {
//!         serde_json::json!({ "receipt": "ok" })
//!     }).unwrap();
//!
//!     let spec = wf.build().unwrap();
//!
//!     // Phase 1: emit graph specification for engine discovery
//!     emit_graph_if_requested(&args, &spec);
//!
//!     // Phase 2: execute a named node
//!     let registry: Vec<(&str, BoxedNodeFn)> = vec![];
//!     execute_node(&args, &registry);
//! }
//! ```
//!
//! For [`parse_execute_args`] usage, see the [`ExecuteArgs`] docs.

use std::any::Any;

use thiserror::Error;
use vo_types::NodeName;

use crate::io::{read_input, write_failure, write_success};
use crate::TaskFailureKind;

/// Marker returned when `--execute-node <name>` flag is present.
#[derive(Debug, PartialEq, Clone)]
pub struct ExecuteArgs {
    pub node_name: NodeName,
}

/// Errors from parsing `--execute-node <name>` arguments.
#[derive(Debug, PartialEq, Error)]
pub enum ExecuteArgsError {
    #[error("unrecognized argument: {arg}")]
    UnrecognizedArgument { arg: String },
    #[error("no --execute-node flag found")]
    NoExecuteNodeFlag,
    #[error("missing node name after --execute-node")]
    MissingNodeName,
    #[error("duplicate --execute-node flag")]
    DuplicateFlag,
}

/// Parse CLI arguments for the `--execute-node <name>` flag.
///
/// # Example
///
/// ```
/// use vo_sdk::execute::parse_execute_args;
///
/// // Valid: --execute-node followed by a name
/// let args = vec![
///     "binary".to_string(),
///     "--execute-node".to_string(),
///     "charge".to_string(),
/// ];
/// let result = parse_execute_args(&args).unwrap();
/// assert_eq!(result.node_name.as_str(), "charge");
///
/// // Returns NoExecuteNodeFlag when absent
/// let args = vec!["binary".to_string()];
/// assert!(parse_execute_args(&args).is_err());
/// ```
///
/// # Errors
///
/// Returns `ExecuteArgsError::NoExecuteNodeFlag` when `--execute-node` is absent.
/// Returns `ExecuteArgsError::MissingNodeName` when `--execute-node` has no following argument.
/// Returns `ExecuteArgsError::DuplicateFlag` when `--execute-node` appears twice.
/// Returns `ExecuteArgsError::UnrecognizedArgument` when extra positional args follow the node name.
pub fn parse_execute_args(args: &[String]) -> Result<ExecuteArgs, ExecuteArgsError> {
    let mut found_execute = false;
    let mut node_name: Option<NodeName> = None;

    let mut args_iter = args.iter().skip(1).peekable();

    while let Some(arg) = args_iter.next() {
        if arg == "--execute-node" {
            if found_execute {
                return Err(ExecuteArgsError::DuplicateFlag);
            }
            found_execute = true;

            let name_str = match args_iter.next() {
                Some(s) => s,
                None => return Err(ExecuteArgsError::MissingNodeName),
            };

            if name_str.starts_with('-') {
                return Err(ExecuteArgsError::MissingNodeName);
            }

            node_name = Some(NodeName::parse(name_str).map_err(|_| {
                ExecuteArgsError::UnrecognizedArgument { arg: name_str.clone() }
            })?);
        } else if arg.starts_with("--graph") {
            return Err(ExecuteArgsError::UnrecognizedArgument { arg: arg.clone() });
        } else if found_execute {
            return Err(ExecuteArgsError::UnrecognizedArgument { arg: arg.clone() });
        }
    }

    if found_execute {
        Ok(ExecuteArgs {
            node_name: node_name.unwrap(),
        })
    } else {
        Err(ExecuteArgsError::NoExecuteNodeFlag)
    }
}

/// Result type for node execution.
pub type NodeResult = Result<serde_json::Value, String>;

/// A executable node function with input/output.
pub trait NodeFn: Any + Send + Sync {
    fn execute(&self, input: serde_json::Value) -> NodeResult;
    fn name(&self) -> &str;
}

/// Wrapper for boxed node functions.
pub struct BoxedNodeFn {
    name: String,
    f: Box<dyn NodeFn>,
}

impl BoxedNodeFn {
    /// Create a new boxed node function with the given name.
    ///
    /// The function must implement the [`NodeFn`] trait.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use vo_sdk::{BoxedNodeFn, NodeFn, NodeResult};
    ///
    /// struct ChargeNode;
    ///
    /// impl NodeFn for ChargeNode {
    ///     fn execute(&self, input: serde_json::Value) -> NodeResult {
    ///         Ok(serde_json::json!({ "receipt": "ok" }))
    ///     }
    ///     fn name(&self) -> &str {
    ///         "charge"
    ///     }
    /// }
    ///
    /// let boxed: BoxedNodeFn = BoxedNodeFn::new("charge", ChargeNode);
    /// ```
    #[must_use]
    pub fn new<F>(name: &str, f: F) -> Self
    where
        F: NodeFn + 'static,
    {
        Self {
            name: name.to_string(),
            f: Box::new(f),
        }
    }
}

/// Execute a node by name, reading input from FD3 and writing output to FD4.
///
/// This function is called when the binary is invoked with `--execute-node <name>`.
/// It reads the task input from FD3, dispatches to the appropriate node function,
/// and writes the result to FD4.
///
/// # Arguments
///
/// * `args` - Command line arguments (typically from `std::env::args()`)
/// * `registry` - Map from node name to executable function
///
/// # Errors
///
/// Returns `()` if `--execute-node` was not present. If `--execute-node` was present,
/// this function always terminates the process (success or failure).
#[allow(clippy::result_unit_err)]
pub fn execute_node(
    args: &[String],
    registry: &[(&str, BoxedNodeFn)],
) -> Result<(), ()> {
    match parse_execute_args(args) {
        Ok(execute_args) => {
            let node_name_str = execute_args.node_name.as_str();

            let handler = registry
                .iter()
                .find(|(name, _)| *name == node_name_str);

            match handler {
                Some((_, node_fn)) => {
                    let input = match read_input() {
                        Ok(input) => input,
                        Err(e) => {
                            let msg = format!("failed to read input: {}", e);
                            let _ = write_failure(TaskFailureKind::System, &msg);
                            std::process::exit(1);
                        }
                    };

                    let result = (node_fn.f).execute(input.data().clone());

                    match result {
                        Ok(output) => {
                            if let Err(e) = write_success(&output) {
                                eprintln!("error: failed to write success: {}", e);
                                std::process::exit(1);
                            }
                            std::process::exit(0);
                        }
                        Err(err_msg) => {
                            if let Err(e) = write_failure(TaskFailureKind::User, &err_msg) {
                                eprintln!("error: failed to write failure: {}", e);
                                std::process::exit(1);
                            }
                            std::process::exit(0);
                        }
                    }
                }
                None => {
                    let msg = format!("node not found: {}", node_name_str);
                    let _ = write_failure(TaskFailureKind::User, &msg);
                    std::process::exit(1);
                }
            }
        }
        Err(ExecuteArgsError::NoExecuteNodeFlag) => Ok(()),
        Err(e) => {
            eprintln!("error: {}", e);
            Err(())
        }
    }
}

/// Check if `--execute-node` flag is present (without parsing the full args).
///
/// Useful for checking which mode a binary should operate in without
/// the full error handling of [`parse_execute_args`].
///
/// # Example
///
/// ```
/// use vo_sdk::execute::has_execute_flag;
///
/// let args = vec![
///     "binary".to_string(),
///     "--execute-node".to_string(),
///     "charge".to_string(),
/// ];
/// assert!(has_execute_flag(&args));
///
/// let args = vec!["binary".to_string(), "--graph".to_string()];
/// assert!(!has_execute_flag(&args));
/// ```
#[must_use]
pub fn has_execute_flag(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--execute-node")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_execute_args_valid_single_arg() {
        let args = vec!["binary".to_string(), "--execute-node".to_string(), "charge".to_string()];
        let result = parse_execute_args(&args);
        assert!(result.is_ok());
        let execute_args = result.unwrap();
        assert_eq!(execute_args.node_name.as_str(), "charge");
    }

    #[test]
    fn parse_execute_args_no_flag() {
        let args = vec!["binary".to_string()];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::NoExecuteNodeFlag)));
    }

    #[test]
    fn parse_execute_args_missing_node_name() {
        let args = vec!["binary".to_string(), "--execute-node".to_string()];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::MissingNodeName)));
    }

    #[test]
    fn parse_execute_args_duplicate_flag() {
        let args = vec![
            "binary".to_string(),
            "--execute-node".to_string(),
            "charge".to_string(),
            "--execute-node".to_string(),
            "validate".to_string(),
        ];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::DuplicateFlag)));
    }

    #[test]
    fn parse_execute_args_extra_args_after_node() {
        let args = vec![
            "binary".to_string(),
            "--execute-node".to_string(),
            "charge".to_string(),
            "extra".to_string(),
        ];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::UnrecognizedArgument { .. })));
    }

    #[test]
    fn parse_execute_args_graph_flag_incompatible() {
        let args = vec![
            "binary".to_string(),
            "--execute-node".to_string(),
            "charge".to_string(),
            "--graph".to_string(),
        ];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::UnrecognizedArgument { arg }) if arg == "--graph"));
    }

    #[test]
    fn has_execute_flag_true() {
        let args = vec!["binary".to_string(), "--execute-node".to_string(), "charge".to_string()];
        assert!(has_execute_flag(&args));
    }

    #[test]
    fn has_execute_flag_false() {
        let args = vec!["binary".to_string(), "--graph".to_string()];
        assert!(!has_execute_flag(&args));
    }

    #[test]
    fn parse_execute_args_rejects_dash_prefixed_name() {
        let args = vec![
            "binary".to_string(),
            "--execute-node".to_string(),
            "--invalid".to_string(),
        ];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::MissingNodeName)));
    }

    #[test]
    fn execute_args_display() {
        let args = ExecuteArgs {
            node_name: NodeName::parse("test_node").unwrap(),
        };
        assert!(format!("{:?}", args).contains("test_node"));
    }

    #[test]
    fn execute_args_error_display() {
        let err = ExecuteArgsError::MissingNodeName;
        assert!(format!("{}", err).contains("missing node name"));

        let err = ExecuteArgsError::NoExecuteNodeFlag;
        assert!(format!("{}", err).contains("no --execute-node flag"));

        let err = ExecuteArgsError::UnrecognizedArgument { arg: "foo".to_string() };
        assert!(format!("{}", err).contains("unrecognized argument"));
        assert!(format!("{}", err).contains("foo"));
    }
}