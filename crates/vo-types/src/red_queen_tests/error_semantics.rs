//! Red Queen tests: error-semantics dimension.
//!
//! Tests that error messages are semantically correct.

use crate::*;

// RQ-07: UnknownNode error when SOURCE is unknown has misleading semantics
// The error variant says "references unknown target node '{unknown_target}'"
// but when the SOURCE is unknown, both edge_source and unknown_target are set
// to the source name. The message reads "edge from 'phantom' references
// unknown target node 'phantom'" which is semantically wrong.
#[test]
fn rq_unknown_source_error_message_is_misleading() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "phantom", "target_node": "b", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected UnknownNode error"),
    };
    match &err {
        WorkflowDefinitionError::UnknownNode {
            edge_source,
            unknown_node,
        } => {
            // The unknown node is the SOURCE (phantom), not the target (b)
            assert_eq!(edge_source.0, "phantom");
            assert_eq!(unknown_node.0, "phantom");
            // The display says "unknown target node" but the unknown is the SOURCE
            let msg = err.to_string();
            assert!(
                msg.contains("unknown target node"),
                "message says 'target' but the unknown node is the source"
            );
            // This is a MINOR defect: the error message is semantically misleading
        }
        _ => panic!("expected UnknownNode, got {:?}", err),
    }
}

// RQ-08: UnknownNode error when TARGET is unknown is correct
#[test]
fn rq_unknown_target_error_message_is_correct() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "ghost", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    match result {
        Err(WorkflowDefinitionError::UnknownNode {
            edge_source,
            unknown_node,
        }) => {
            assert_eq!(edge_source.0, "a");
            assert_eq!(unknown_node.0, "ghost");
        }
        _ => panic!("expected UnknownNode with correct fields"),
    }
}
