#![allow(unexpected_cfgs)]

//! vo-sdk: Thin, zero-panic library for task binaries to read FD3 input and write FD4 output.
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

pub mod dag;
pub mod graph_args;
pub mod node_handle;
pub mod runtime;
pub use dag::Workflow;
pub use graph_args::{
    emit_graph_if_requested, parse_graph_args, EdgeSpec, GraphArgs, GraphArgsError, NodeSpec,
    WorkflowSpec,
};
pub use runtime::start;
mod read;
mod write;

#[cfg(test)]
mod tests;

use serde_json::Value;
use std::fmt::Display;
use vo_types::IdempotencyKey;

// Re-export public API
pub use read::read_input;
pub use write::{write_failure, write_success};

#[derive(Debug, PartialEq)]
pub enum SdkError {
    InvalidInput,
    FdNotOpen,
    AlreadyWritten,
    WriteError,
}

impl Display for SdkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SdkError {}

// TODO(vel-edo): TaskFailureKind should live in vo-types per the contract.
// Kept here temporarily because this bead is scoped to vo-sdk only.
// See: contract.md precondition "vo-types must define the shared IPC types"
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

#[derive(Debug, PartialEq)]
pub struct TaskInput {
    pub idempotency_key: IdempotencyKey,
    pub data: Value,
}

impl TaskInput {
    #[must_use]
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

// TODO(vel-edo): TaskInputEnvelope should live in vo-types per the contract.
// Kept here temporarily because this bead is scoped to vo-sdk only.
#[derive(serde::Deserialize)]
pub(crate) struct TaskInputEnvelope {
    idempotency_key: String,
    data: Value,
}
