//! Comprehensive tests for vo-sdk.
//!
//! These exercise the `_inner` variants of `read_input` / `write_success` / `write_failure`
//! using in-memory readers/writers, since actual FD3/FD4 are unavailable in test.

mod dag_tests;
mod graph_args_tests;
mod proptest_dag;
mod proptest_read_write;
mod read_tests;
mod red_queen_workflow_spec;
mod type_tests;
mod workflow_builder_tests;
mod workflow_spec_validation_tests;
mod write_failure_tests;
mod write_success_tests;

use serde_json::{json, Value};

/// Build a valid JSON input envelope as bytes.
pub(super) fn valid_envelope(key: &str, data: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "idempotency_key": key,
        "data": data,
    }))
    .expect("test helper: serialization should not fail")
}
