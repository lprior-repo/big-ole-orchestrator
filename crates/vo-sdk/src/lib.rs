#![allow(unexpected_cfgs)]

//! vo-sdk: Thin, zero-panic library for task binaries to read FD3 input and write FD4 output.
//!
//! ## Modules
//!
//! - [`runtime`] - Async task execution with current-thread Tokio runtime (ADR-011)
//! - [`io`] - I/O helpers: `read_input`, `write_success`, `write_failure` with single-write guard
//! - [`graph`] - Graph emission: `--graph` CLI argument handling and workflow specification types
//! - [`dag`] - DAG construction with compile-time type-safe workflow graph builder
//! - [`node_handle`] - Typed node handles for workflow connections
//! - [`execute`] - Node execution: `--execute-node` CLI argument handling and node registry
//!
//! ## Write-once invariant
//! `write_success` / `write_failure` may be called at most once per process lifetime.
//! The guard is set *before* any I/O attempt — even if the write fails, subsequent
//! calls are rejected with `SdkError::AlreadyWritten`.
//!
//! ## Message limit
//! The failure message limit (1024) is enforced in **bytes**, not characters.
//! A multibyte UTF-8 message may be rejected below 1024 chars if it exceeds 1024 bytes.
//!
//! ## Workflow Builder API
//!
//! The [`Workflow`] struct provides a fluent builder for constructing workflow graphs.
//! Use [`Workflow::build`] to produce a [`WorkflowSpec`] which can be emitted via
//! [`emit_graph_if_requested`] or serialized to JSON.
//!
//! # Example
//!
//! ```ignore
//! use vo_sdk::{Workflow, emit_graph_if_requested};
//!
//! let mut wf = Workflow::new("checkout");
//! let validate = wf.pure("validate", |input: String| -> i32 { 0 }).unwrap();
//! let charge = wf.effect("charge", |input: i32| -> bool { true }).unwrap();
//! wf.connect(&validate, &charge).unwrap();
//!
//! let spec = wf.build().unwrap();
//! emit_graph_if_requested(&std::env::args().collect::<Vec<_>>(), &spec);
//! ```
//!
//! ## Task Execution
//!
//! Task binaries use [`runtime::start`] as their entry point instead of `#[tokio::main]`:
//!
//! ```ignore
//! use vo_sdk::runtime::start;
//! use vo_sdk::TaskInput;
//!
//! fn main() {
//!     start(|input: TaskInput| async move {
//!         // Your async task logic here
//!         Ok(serde_json::json!({"result": "done"}))
//!     });
//! }
//! ```
//!
//! This provides sub-millisecond startup vs ~200ms for a full Tokio multi-threaded runtime.

pub mod dag;
pub mod execute;
pub mod graph;
pub mod node_handle;
pub mod runtime;

pub use dag::Workflow;
pub use execute::{dispatch_execute, parse_execute_args, ExecuteArgs, ExecuteArgsError, NodeRegistry};
pub use graph::{
    emit_graph_if_requested, parse_graph_args, EdgeSpec, GraphArgs, GraphArgsError, NodeSpec,
    SignalNodeMeta, ValidationError, WorkflowSpec,
};
pub mod io;

#[cfg(test)]
mod tests;

use thiserror::Error;

// Re-export public API
pub use io::{is_read, is_written, read_input, secret, write_failure, write_success};
pub use vo_types::TaskFailureKind;

#[derive(Debug, PartialEq, Error)]
pub enum SdkError {
    #[error("InvalidInput")]
    InvalidInput,
    #[error("FdNotOpen")]
    FdNotOpen,
    #[error("AlreadyWritten")]
    AlreadyWritten,
    #[error("WriteError")]
    WriteError,
}

// TaskInput and TaskFailureKind re-exported from vo_types.
pub use vo_types::TaskInput;
