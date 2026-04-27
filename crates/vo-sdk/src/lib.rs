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
//!
//! ## Macros
//!
//! - [`vo_task`] - Generates executable entrypoints from functions
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
pub mod graph;
pub mod node_handle;
pub mod runtime;

mod signal;

pub use dag::Workflow;
pub use graph::{
    emit_graph_if_requested, parse_graph_args, EdgeSpec, GraphArgs, GraphArgsError, NodeSpec,
    SignalNodeMeta, ValidationError, WorkflowSpec,
};
pub mod io;

// Re-export the macro for use as #[vo_task]
//
// #[vo_task] generates a main() function that wraps the annotated function,
// making it executable as a workflow task binary.
//
// # Basic usage
//
// ```ignore
// use vo_sdk::vo_task;
//
// #[vo_task]
// fn my_task() {
//     // Task implementation
// }
// ```
//
// When compiled, this generates:
//
// ```ignore
// fn my_task() { /* ... */ }
// fn main() { my_task(); }
// ```
//
// # Task with input from environment
//
// Function arguments are bound from environment variables (uppercase names):
//
// ```ignore
// use vo_sdk::vo_task;
//
// #[vo_task]
// fn fetch_data(url: String) {
//     // URL is read from the URL environment variable
//     // ...
// }
// ```
//
// # Async task
//
// ```ignore
// use vo_sdk::vo_task;
//
// #[vo_task]
// async fn async_process(payload: String) {
//     // Runs on a tokio single-thread runtime
//     // ...
// }
// ```
//
// # Task with return type
//
// ```ignore
// use vo_sdk::{vo_task, write_success};
// use serde_json::json;
//
// #[vo_task]
// fn transform(input: String) -> Result<(), std::io::Error> {
//     let result = process(&input)?;
//     write_success(&json!({"result": result})).unwrap();
//     Ok(())
// }
// ```
//
// # Complete workflow definition pattern
//
// ```ignore
// use vo_sdk::{vo_task, Workflow, emit_graph_if_requested, write_success, write_failure, TaskFailureKind};
// use serde_json::json;
//
// #[vo_task]
// fn checkout_workflow() {
//     let mut wf = Workflow::new("checkout");
//
//     // Define nodes with input/output types
//     let validate = wf.pure("validate", |input: String| -> bool {
//         !input.is_empty()
//     }).unwrap();
//
//     let charge = wf.effect("charge", |input: bool| -> Result<String, String> {
//         if input {
//             Ok("charged".to_string())
//         } else {
//             Err("invalid".to_string())
//         }
//     }).unwrap();
//
//     let email = wf.effect("send_email", |input: String| -> () {
//         // Send confirmation email
//     }).unwrap();
//
//     // Connect nodes (output type of validate -> input type of charge)
//     wf.connect(&validate, &charge).unwrap();
//     wf.connect(&charge, &email).unwrap();
//
//     // Build and emit the workflow spec when --graph is passed
//     let spec = wf.build().unwrap();
//     emit_graph_if_requested(&std::env::args().collect::<Vec<_>>(), &spec);
//
//     // Runtime execution would read input and run the workflow
//     let input = vo_sdk::read_input().unwrap();
//     let validated = validate.fn_(input.payload);
//     match charge.fn_(validated) {
//         Ok(receipt) => {
//             email.fn_(receipt.clone());
//             write_success(&json!({"receipt": receipt})).unwrap();
//         }
//         Err(e) => {
//             write_failure(TaskFailureKind::User, &e).unwrap();
//         }
//     }
// }
// ```
//
// # Task input/output types
//
// Tasks receive input via [`read_input`] which returns a [`TaskInput`]:
//
// ```ignore
// use vo_sdk::{vo_task, read_input, write_success};
// use serde_json::json;
//
// #[vo_task]
// fn process_order() {
//     let input = read_input().unwrap();
//
//     // TaskInput provides access to payload and secrets
//     let order_id = &input.payload["order_id"];
//     let api_key = input.secret("API_KEY");
//
//     // Process and write result
//     let result = process(order_id, api_key);
//     write_success(&json!({"status": "ok", "data": result})).unwrap();
// }
// ```
pub use vo_sdk_macros::task_macro as vo_task;

#[cfg(test)]
mod tests;

use thiserror::Error;

// Re-export public API
pub use io::{is_read, is_written, read_input, secret, write_failure, write_success};
pub use signal::start;

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
