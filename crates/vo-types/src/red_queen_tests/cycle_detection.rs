//! Red Queen tests: cycle-detection-advanced dimension.
//!
//! Tests disconnected cycles, diamond+cycle, large cycles, and self-loops.

use crate::*;

// RQ-21: Cycle in disconnected component (not reachable from nodes[0])
#[test]
fn rq_cycle_in_disconnected_component_detected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "d", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "c", "target_node": "d", "condition": "Always"},
            {"source_node": "d", "target_node": "c", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
}

// RQ-22: Diamond with cycle in one branch
#[test]
fn rq_diamond_with_cycle_in_branch_detected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "d", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "e", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "a", "target_node": "c", "condition": "Always"},
            {"source_node": "b", "target_node": "d", "condition": "Always"},
            {"source_node": "c", "target_node": "e", "condition": "Always"},
            {"source_node": "e", "target_node": "c", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
}

// RQ-23: Large 5-node cycle
#[test]
fn rq_large_5_node_cycle_detected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "d", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "e", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "c", "condition": "Always"},
            {"source_node": "c", "target_node": "d", "condition": "Always"},
            {"source_node": "d", "target_node": "e", "condition": "Always"},
            {"source_node": "e", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            // 5-node cycle should produce [a, b, c, d, e, a] = 6 elements
            assert_eq!(
                cycle_nodes.len(),
                6,
                "expected 5-node cycle path with repeated start"
            );
            assert_eq!(
                cycle_nodes[0].0, cycle_nodes[5].0,
                "first and last should be same node"
            );
        }
        _ => panic!("expected CycleDetected, got {:?}", result),
    }
}

// RQ-24: Self-loop on non-first node (not nodes[0])
#[test]
fn rq_self_loop_on_non_first_node_detected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "b", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(
        matches!(result, Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) if cycle_nodes.len() == 2)
    );
}

// RQ-25: Complex graph: two separate cycles in disconnected components
#[test]
fn rq_two_separate_cycles_both_detected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "d", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "a", "condition": "Always"},
            {"source_node": "c", "target_node": "d", "condition": "Always"},
            {"source_node": "d", "target_node": "c", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    // At least one cycle must be detected (first one found)
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
}

// RQ-26: Isolated node (no edges in or out) with cycle elsewhere
#[test]
fn rq_isolated_node_with_cycle_elsewhere_detected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [
            {"node_name": "isolated", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
}
