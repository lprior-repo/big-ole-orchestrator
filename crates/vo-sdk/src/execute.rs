//! Execute-node CLI argument handling and node execution dispatch (ADR-009, ADR-011).
//!
//! This module provides the CLI contract for the execution phase of a multi-task
//! binary. When the Engine invokes `./binary --execute-node <name>`, this module
//! parses the node name and dispatches to the registered node implementation.
//!
//! ## Multi-Task Binary Usage
//!
//! A workflow binary uses this module by:
//! 1. Creating a `NodeRegistry` and registering all node handlers
//! 2. Building the `WorkflowSpec`
//! 3. Calling [`dispatch_execute`] which handles both `--graph` and `--execute-node`
//!
//! ```ignore
//! use vo_sdk::{Workflow, emit_graph_if_requested, NodeRegistry, dispatch_execute};
//!
//! fn main() {
//!     let mut wf = Workflow::new("checkout_flow");
//!     let validate = wf.pure("validate", |input: String| -> i32 { 0 }).unwrap();
//!     let charge = wf.effect("charge", |input: i32| -> bool { true }).unwrap();
//!     wf.connect(&validate, &charge).unwrap();
//!     let spec = wf.build().unwrap();
//!
//!     let registry = NodeRegistry::new();
//!     registry.register("validate", |input| {
//!         Ok(serde_json::json!({"validated": true}))
//!     });
//!     registry.register("charge", |input| {
//!         Ok(serde_json::json!({"charged": true}))
//!     });
//!
//!     dispatch_execute(&std::env::args().collect::<Vec<_>>(), &spec, &registry);
//! }
//! ```

use std::sync::{Arc, Mutex};
use thiserror::Error;
use vo_types::{TaskFailureKind, TaskInput};

use serde_json::Value;

use crate::graph::WorkflowSpec;
use crate::io::{read_input, write_failure, write_success};

#[derive(Debug, Error)]
pub enum ExecuteArgsError {
    #[error("unrecognized argument: {arg}")]
    UnrecognizedArgument { arg: String },
    #[error("no --execute-node flag found")]
    NoExecuteNodeFlag,
    #[error("missing node name after --execute-node")]
    MissingNodeName,
    #[error("node not found: {name}")]
    NodeNotFound { name: String },
    #[error("execution failed: {reason}")]
    ExecutionFailed { reason: String },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ExecuteArgs {
    pub node_name: &'static str,
}

pub struct NodeRegistry {
    entries: Mutex<std::collections::HashMap<&'static str, NodeEntry>>,
}

struct NodeEntry {
    handler: Arc<dyn Fn(TaskInput) -> Result<Value, String> + Send + Sync + 'static>,
}

impl NodeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn register<F>(&self, name: &'static str, f: F)
    where
        F: Fn(TaskInput) -> Result<Value, String> + Send + Sync + 'static,
    {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(
            name,
            NodeEntry {
                handler: Arc::new(f),
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<Box<dyn Fn(TaskInput) -> Result<Value, String> + Send + Sync + 'static>> {
        let entries = self.entries.lock().unwrap();
        entries.get(name).map(|e| {
            let handler = e.handler.clone();
            Box::new(move |input: TaskInput| handler(input)) as Box<dyn Fn(TaskInput) -> Result<Value, String> + Send + Sync + 'static>
        })
    }

    pub fn node_names(&self) -> Vec<&'static str> {
        let entries = self.entries.lock().unwrap();
        entries.keys().copied().collect()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_execute_args(args: &[String]) -> Result<ExecuteArgs, ExecuteArgsError> {
    let mut found_execute = false;
    let mut node_name: Option<&str> = None;

    for arg in args.iter().skip(1) {
        if arg == "--execute-node" {
            if found_execute {
                return Err(ExecuteArgsError::UnrecognizedArgument {
                    arg: arg.clone(),
                });
            }
            found_execute = true;
        } else if found_execute && node_name.is_none() {
            let leaked: &'static str = Box::leak(arg.clone().into_boxed_str());
            node_name = Some(leaked);
        } else {
            return Err(ExecuteArgsError::UnrecognizedArgument {
                arg: arg.clone(),
            });
        }
    }

    if found_execute {
        let name = node_name.ok_or(ExecuteArgsError::MissingNodeName)?;
        Ok(ExecuteArgs { node_name: name })
    } else {
        Err(ExecuteArgsError::NoExecuteNodeFlag)
    }
}

/// Dispatch to either graph emission or node execution based on CLI arguments.
///
/// This is the main entry point for multi-task binaries. It:
///
/// 1. Checks for `--graph` first — if present, emits `WorkflowSpec` JSON and exits
/// 2. If `--execute-node <name>` is present, looks up the handler in `registry`,
///    reads input from FD3, executes the handler, and writes the result to FD4
///
/// # Arguments
///
/// * `args` - Command-line arguments (typically `std::env::args().collect()`)
/// * `spec` - The workflow specification (used for `--graph` emission)
/// * `registry` - Node registry containing registered node handlers
///
/// # Example
///
/// ```ignore
/// use vo_sdk::{Workflow, NodeRegistry, dispatch_execute};
///
/// fn main() {
///     let mut wf = Workflow::new("checkout");
///     let v = wf.pure("validate", |i: String| -> i32 { 0 }).unwrap();
///     let c = wf.effect("charge", |i: i32| -> bool { true }).unwrap();
///     wf.connect(&v, &c).unwrap();
///
///     let registry = NodeRegistry::new();
///     registry.register("validate", |input| Ok(serde_json::json!({})));
///     registry.register("charge", |input| Ok(serde_json::json!({})));
///
///     dispatch_execute(&std::env::args().collect::<Vec<_>>(), &wf.build().unwrap(), &registry);
/// }
/// ```
pub fn dispatch_execute(args: &[String], spec: &WorkflowSpec, registry: &NodeRegistry) {
    if let Err(()) = crate::graph::emit_graph_if_requested(args, spec) {
        return;
    }

    match parse_execute_args(args) {
        Ok(execute_args) => {
            let handler = registry.get(execute_args.node_name);
            match handler {
                Some(handler) => {
                    let input = match read_input() {
                        Ok(input) => input,
                        Err(e) => {
                            eprintln!("vo-sdk: failed to read input: {}", e);
                            std::process::exit(1);
                        }
                    };

                    let result = handler(input);

                    match result {
                        Ok(value) => {
                            if let Err(e) = write_success(&value) {
                                eprintln!("vo-sdk: failed to write success: {}", e);
                                std::process::exit(1);
                            }
                            std::process::exit(0);
                        }
                        Err(err_msg) => {
                            if let Err(e) =
                                write_failure(TaskFailureKind::User, &err_msg)
                            {
                                eprintln!("vo-sdk: failed to write failure: {}", e);
                                std::process::exit(1);
                            }
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    eprintln!(
                        "vo-sdk: node not found: {}",
                        execute_args.node_name
                    );
                    eprintln!("available nodes: {:?}", registry.node_names());
                    std::process::exit(1);
                }
            }
        }
        Err(ExecuteArgsError::NoExecuteNodeFlag) => {
            let _ = spec;
        }
        Err(e) => {
            eprintln!("vo-sdk: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_execute_args_valid() {
        let args: Vec<String> = vec!["binary".into(), "--execute-node".into(), "charge".into()];
        let result = parse_execute_args(&args);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.node_name, "charge");
    }

    #[test]
    fn parse_execute_args_no_flag() {
        let args: Vec<String> = vec!["binary".into(), "--graph".into()];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::NoExecuteNodeFlag)));
    }

    #[test]
    fn parse_execute_args_missing_node_name() {
        let args: Vec<String> = vec!["binary".into(), "--execute-node".into()];
        let result = parse_execute_args(&args);
        assert!(matches!(result, Err(ExecuteArgsError::MissingNodeName)));
    }

    #[test]
    fn parse_execute_args_extra_arg() {
        let args: Vec<String> = vec![
            "binary".into(),
            "--execute-node".into(),
            "charge".into(),
            "extra".into(),
        ];
        let result = parse_execute_args(&args);
        assert!(matches!(
            result,
            Err(ExecuteArgsError::UnrecognizedArgument { .. })
        ));
    }

    #[test]
    fn node_registry_register_and_get() {
        let registry = NodeRegistry::new();
        registry.register("test_node", |_input: TaskInput| {
            Ok(serde_json::json!({"result": "ok"}))
        });

        let handler = registry.get("test_node");
        assert!(handler.is_some());

        let handler = registry.get("nonexistent");
        assert!(handler.is_none());
    }

    #[test]
    fn node_registry_node_names() {
        let registry = NodeRegistry::new();
        registry.register("node_a", |_input: TaskInput| Ok(serde_json::json!({})));
        registry.register("node_b", |_input: TaskInput| Ok(serde_json::json!({})));

        let names = registry.node_names();
        assert!(names.contains(&"node_a"));
        assert!(names.contains(&"node_b"));
    }
}