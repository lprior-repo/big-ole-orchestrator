//! Fuzz target: WorkflowDefinition JSON Deserialization
//!
//! This fuzz target tests that arbitrary bytes can be passed to the JSON
//! deserializer without panicking.
//!
//! Corpus seeds:
//! - Valid minimal WorkflowDefinition: {"workflow_name": "x", "nodes": [], "edges": []}
//! - Single node, no edges
//! - Nodes with various RetryPolicy configurations
//! - Edge with various condition values (null, string, object)
//! - Self-loop
//! - Deeply nested JSON (stack overflow risk)
//! - Invalid UTF-8 bytes

#![no_main]

use libfuzzer_sys::fuzz_target;
use vel_bxpg::WorkflowDefinition;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse the input as a WorkflowDefinition JSON string
    // We first try to parse as UTF-8 string, then as JSON
    if let Ok(json_str) = std::str::from_utf8(data) {
        // This should NOT panic on any input
        // The deserializer may return an error, but must not panic
        let _: Result<WorkflowDefinition, _> = serde_json::from_str(json_str);
    }
});
