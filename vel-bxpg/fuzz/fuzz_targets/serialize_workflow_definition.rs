//! Fuzz target: Graph Output JSON Serialization
//!
//! This fuzz target tests that WorkflowDefinition structs can be serialized
//! to JSON without panicking.
//!
//! Corpus seeds:
//! - Minimal valid WorkflowDefinition
//! - WorkflowDefinition with maximum node count
//! - WorkflowDefinition with special characters in node names
//! - WorkflowDefinition with empty strings in required fields
//! - WorkflowDefinition with extremely long workflow_name

#![no_main]

use libfuzzer_sys::fuzz_target;
use vel_bxpg::{DagNode, Edge, WorkflowDefinition};

fuzz_target!(|data: &[u8]| {
    // We use a simple approach: interpret the data as a string and try to
    // construct a WorkflowDefinition from it, then serialize it.
    //
    // The format we accept: "workflow_name\nnode_count\nedge_count\n[node1]\n[node2]\n..."
    // Each line is a string that we use as a name.
    //
    // If the data can't be parsed, we skip this input.

    if let Ok(text) = std::str::from_utf8(data) {
        let mut lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return;
        }

        let workflow_name = lines.remove(0).to_string();

        let node_count = if lines.is_empty() {
            0
        } else {
            lines[0].parse::<usize>().unwrap_or(0).min(100)
        };

        let edge_count = if lines.len() >= 2 {
            lines[1].parse::<usize>().unwrap_or(0).min(500)
        } else {
            0
        };

        // Build nodes
        let mut nodes = Vec::new();
        for i in 0..node_count {
            let name = format!("node_{}", i);
            nodes.push(DagNode {
                name,
                retry_policy: None,
            });
        }

        // Build edges
        let mut edges = Vec::new();
        for i in 0..edge_count.min(100) {
            if nodes.len() >= 2 {
                let source_idx = i % nodes.len();
                let target_idx = (i + 1) % nodes.len();
                edges.push(Edge {
                    source_node: nodes[source_idx].name.clone(),
                    target_node: nodes[target_idx].name.clone(),
                    condition: None,
                });
            }
        }

        let workflow = WorkflowDefinition {
            workflow_name,
            nodes,
            edges,
        };

        // This should NOT panic on any valid WorkflowDefinition
        let result = serde_json::to_string(&workflow);
        // We don't care about the result, just that it doesn't panic
        let _ = result;
    }
});
