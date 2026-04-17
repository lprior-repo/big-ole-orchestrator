//! Comprehensive tests for vo-sdk.
//!
//! These exercise the `_inner` variants of `read_input` / `write_success` / `write_failure`
//! using in-memory readers/writers, since actual FD3/FD4 are unavailable in test.

mod adversarial_tests;
mod dag_tests;
mod graph_args_tests;
mod proptest_dag;
mod proptest_read_write;
mod read_tests;
mod red_queen_workflow_spec;
mod type_tests;
mod workflow_builder_tests;
<<<<<<< HEAD
mod workflow_spec_validation_tests;
=======
>>>>>>> origin/polecat/synth-mnw6kj8v
mod write_failure_tests;
mod write_success_tests;

// Re-export internal functions for tests
pub use crate::graph::{parse_graph_args, WorkflowSpec};
pub use crate::io::{
    read_input_inner_with_atomic_guard, read_input_inner_with_state,
    write_failure_inner_with_state, write_success_inner_with_state,
};

use serde_json::{json, Value};

/// Build a valid JSON input envelope as bytes.
pub(super) fn valid_envelope(key: &str, data: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "idempotency_key": key,
        "data": data,
    }))
    .expect("test helper: serialization should not fail")
}
