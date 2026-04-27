#![allow(unexpected_cfgs)]

//! vo-sdk: Thin, zero-panic library for task binaries to read FD3 input and write FD4 output.
//!
//! ## Modules
//!
//! - [`io`] - I/O helpers: `read_input`, `write_success`, `write_failure` with single-write guard
//! - [`graph`] - Graph emission: `--graph` CLI argument handling and workflow specification types
//! - [`dag`] - DAG construction with compile-time type-safe workflow graph builder
//! - [`node_handle`] - Typed node handles for workflow connections
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
//! For concrete examples of the Workflow builder API, see the documentation for
//! [`Workflow`](dag::Workflow), [`Dag`](dag::Dag), [`execute_node`](execute::execute_node),
//! and [`emit_graph_if_requested`](graph::emit_graph_if_requested).

pub mod dag;
pub mod execute;
pub mod graph;
pub mod node_handle;

pub use dag::Workflow;
pub use execute::{
    execute_node, has_execute_flag, parse_execute_args, BoxedNodeFn, ExecuteArgs, ExecuteArgsError,
    NodeFn, NodeResult,
};
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

/// Errors from SDK I/O operations.
///
/// # Example
///
/// ```
/// use vo_sdk::SdkError;
///
/// // Each error variant describes a distinct failure mode
/// assert_eq!(SdkError::InvalidInput.to_string(), "InvalidInput");
/// assert_eq!(SdkError::FdNotOpen.to_string(), "FdNotOpen");
/// assert_eq!(SdkError::AlreadyWritten.to_string(), "AlreadyWritten");
/// assert_eq!(SdkError::WriteError.to_string(), "WriteError");
/// ```
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

// TODO(vel-edo): TaskFailureKind should live in vo-types per the contract.
// Kept here temporarily because this bead is scoped to vo-sdk only.
// See: contract.md precondition "vo-types must define the shared IPC types"
/// Kind of task failure, used to categorize errors from [`write_failure`](crate::write_failure).
///
/// # Example
///
/// ```
/// use vo_sdk::TaskFailureKind;
///
/// // Each variant represents a distinct failure category
/// assert_eq!(format!("{:?}", TaskFailureKind::User), "User");
/// assert_eq!(format!("{:?}", TaskFailureKind::System), "System");
/// assert_eq!(format!("{:?}", TaskFailureKind::Timeout), "Timeout");
/// ```
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TaskFailureKind {
    User,
    System,
    Timeout,
}

impl TaskFailureKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::System => "System",
            Self::Timeout => "Timeout",
        }
    }
}

// TaskInput re-exported from vo_types.
pub use vo_types::TaskInput;
