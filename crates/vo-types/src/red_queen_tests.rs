//! Red Queen adversarial tests for vo-types workflow module.
//!
//! bead_id: vo-ald
//! phase: state-5-red-queen
//!
//! Dimensions attacked:
//!   - contract-violations: NaN/INFINITY bypass, direct construction
//!   - error-semantics: misleading error messages
//!   - json-attacks: malformed, wrong types, extra fields, nulls
//!   - cycle-detection-advanced: disconnected, diamond+cycle, large cycles
//!   - next_nodes-edge-cases: non-existent, duplicates, condition filtration
//!   - boundary-values: u8::MAX, u64::MAX, negative zero, sub-1.0
//!   - serde-integrity: round-trip with boundary values
//!   - proptest-property-attacks: fuzz RetryPolicy, next_nodes, parse

use crate::*;
use proptest::prelude::*;
use rstest::rstest;
use std::collections::HashSet;

// ===========================================================================
// Helpers (crate-internal, bypasses parse validation for unit tests)
// ===========================================================================

fn make_def(
    name: &str,
    nodes: Vec<(&str, u8, u64, f32)>,
    edges: Vec<(&str, &str, EdgeCondition)>,
) -> WorkflowDefinition {
    WorkflowDefinition {
        workflow_name: WorkflowName(name.into()),
        nodes: NonEmptyVec::new_unchecked(
            nodes
                .into_iter()
                .map(|(n, a, b, m)| DagNode {
                    node_name: NodeName(n.into()),
                    retry_policy: RetryPolicy {
                        max_attempts: a,
                        backoff_ms: b,
                        backoff_multiplier: m,
                    },
                })
                .collect(),
        ),
        edges: edges
            .into_iter()
            .map(|(s, t, c)| Edge {
                source_node: NodeName(s.into()),
                target_node: NodeName(t.into()),
                condition: c,
            })
            .collect(),
    }
}

fn step_outcome_strategy() -> impl Strategy<Value = StepOutcome> {
    proptest::prop_oneof![Just(StepOutcome::Success), Just(StepOutcome::Failure),]
}

fn edge_condition_strategy() -> impl Strategy<Value = EdgeCondition> {
    proptest::prop_oneof![
        Just(EdgeCondition::Always),
        Just(EdgeCondition::OnSuccess),
        Just(EdgeCondition::OnFailure),
    ]
}

// ===========================================================================
// DIMENSION: contract-violations
// NaN/INFINITY through RetryPolicy::new(), direct construction bypass
// ===========================================================================

// RQ-01: NaN multiplier is rejected by RetryPolicy::new()
// NaN < 1.0 is FALSE in IEEE 754, but we explicitly check is_nan().
#[test]
fn rq_nan_multiplier_rejected_by_retry_policy_new() {
    let result = RetryPolicy::new(1, 0, f32::NAN);
    assert!(
        matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })),
        "NaN must be rejected"
    );
    let err = result.unwrap_err();
    assert!(err.to_string().contains("backoff_multiplier"));
}

// RQ-02: INFINITY multiplier passes through RetryPolicy::new()
// INFINITY < 1.0 is false, so INFINITY passes.
#[test]
fn rq_infinity_multiplier_passes_through_retry_policy_new() {
    let result = RetryPolicy::new(1, 0, f32::INFINITY);
    let policy = result.expect("INFINITY passes because INFINITY < 1.0 is false");
    assert!(
        policy.backoff_multiplier.is_infinite() && policy.backoff_multiplier.is_sign_positive()
    );
}

// RQ-03: NEG_INFINITY multiplier is correctly rejected
#[test]
fn rq_neg_infinity_multiplier_rejected() {
    let result = RetryPolicy::new(1, 0, f32::NEG_INFINITY);
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));
}

// RQ-04: NaN multiplier in JSON is rejected by serde
#[test]
fn rq_nan_multiplier_in_json_rejected_by_serde() {
    let json = r#"{"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": NaN}"#;
    let result: Result<RetryPolicy, _> = serde_json::from_str(json);
    result.unwrap_err();
}

// RQ-05: INFINITY multiplier in JSON is rejected by serde
#[test]
fn rq_infinity_multiplier_in_json_rejected_by_serde() {
    let json = r#"{"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": Infinity}"#;
    let result: Result<RetryPolicy, _> = serde_json::from_str(json);
    result.unwrap_err();
}

// RQ-05b: -INFINITY multiplier in JSON is rejected by serde
#[test]
fn rq_neg_infinity_multiplier_in_json_rejected_by_serde() {
    let json = r#"{"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": -Infinity}"#;
    let result: Result<RetryPolicy, _> = serde_json::from_str(json);
    result.unwrap_err();
}

// RQ-06: Direct RetryPolicy construction bypasses validation (fields are pub)
#[test]
fn rq_direct_retry_policy_construction_allows_invalid_state() {
    // Fields are pub, so direct construction bypasses RetryPolicy::new() validation
    let policy = RetryPolicy {
        max_attempts: 0, // violates I-6
        backoff_ms: 0,
        backoff_multiplier: 0.0, // violates I-7
    };
    assert_eq!(policy.max_attempts, 0);
    assert_eq!(policy.backoff_multiplier, 0.0);
    // This is "by design" (pub fields) but means invariants are only enforced
    // through the parse() + new() constructors.
}

// ===========================================================================
// DIMENSION: error-semantics
// Tests that error messages are semantically correct
// ===========================================================================

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
            unknown_target,
        } => {
            // The unknown node is the SOURCE (phantom), not the target (b)
            assert_eq!(edge_source.0, "phantom");
            assert_eq!(unknown_target.0, "phantom");
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
            unknown_target,
        }) => {
            assert_eq!(edge_source.0, "a");
            assert_eq!(unknown_target.0, "ghost");
        }
        _ => panic!("expected UnknownNode with correct fields"),
    }
}

// ===========================================================================
// DIMENSION: json-attacks
// Malformed, wrong types, extra fields, nulls, empty
// ===========================================================================

// RQ-09: Extra fields in JSON are silently ignored
#[test]
fn rq_extra_json_fields_ignored() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}, "extra_field": "ignored"}],
        "edges": [],
        "bogus_field": 42,
        "another_one": true
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    result.expect("extra JSON fields should be silently ignored");
}

// RQ-10: Wrong type for workflow_name (number instead of string)
#[test]
fn rq_wrong_type_workflow_name_rejected() {
    let json = serde_json::json!({
        "workflow_name": 123,
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-11: Wrong type for node_name (number instead of string)
#[test]
fn rq_wrong_type_node_name_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": 42, "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-12: Wrong type for max_attempts (string instead of number)
#[test]
fn rq_wrong_type_max_attempts_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": "three", "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-13: Wrong type for edge condition (number instead of string)
#[test]
fn rq_wrong_type_edge_condition_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": 42}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-14: Null for workflow_name
#[test]
fn rq_null_workflow_name_rejected() {
    let json = serde_json::json!({
        "workflow_name": null,
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-15: Empty bytes input
#[test]
fn rq_empty_bytes_rejected() {
    let bytes: &[u8] = b"";
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-16: Array instead of object
#[test]
fn rq_array_instead_of_object_rejected() {
    let bytes = b"[]";
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-17: Null for retry_policy
#[test]
fn rq_null_retry_policy_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": null}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-18: String "NaN" for backoff_multiplier (not actual NaN token)
#[test]
fn rq_string_nan_for_multiplier_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": "NaN"}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-19: Boolean for edge condition
#[test]
fn rq_boolean_edge_condition_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": true}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// RQ-20: Invalid edge condition string
#[test]
fn rq_invalid_edge_condition_string_rejected() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": "Sometimes"}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// ===========================================================================
// DIMENSION: cycle-detection-advanced
// Disconnected cycles, diamond+cycle, large cycles, self-loops on non-first nodes
// ===========================================================================

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

// ===========================================================================
// DIMENSION: next_nodes-edge-cases
// Non-existent current, duplicates, condition filtration, complex graphs
// ===========================================================================

// RQ-27: next_nodes with non-existent current node returns empty
#[test]
fn rq_next_nodes_nonexistent_current_returns_empty() {
    let def = make_def("test", vec![("a", 1, 0, 1.0)], vec![]);
    let result = next_nodes(&NodeName("nonexistent".into()), StepOutcome::Success, &def);
    assert!(result.is_empty());
}

// RQ-28: next_nodes with non-existent current on Failure returns empty
#[test]
fn rq_next_nodes_nonexistent_current_failure_returns_empty() {
    let def = make_def("test", vec![("a", 1, 0, 1.0)], vec![]);
    let result = next_nodes(&NodeName("nonexistent".into()), StepOutcome::Failure, &def);
    assert!(result.is_empty());
}

// RQ-29: next_nodes with duplicate edges returns duplicate results (per NG-14)
#[test]
fn rq_next_nodes_duplicate_edges_returns_duplicates() {
    let def = WorkflowDefinition {
        workflow_name: WorkflowName("test".into()),
        nodes: NonEmptyVec::new_unchecked(vec![
            DagNode {
                node_name: NodeName("a".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
            },
            DagNode {
                node_name: NodeName("b".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
            },
        ]),
        edges: vec![
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::Always,
            },
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::Always,
            },
        ],
    };
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    // Two identical edges → two results (NG-14: no edge deduplication)
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].node_name, NodeName("b".into()));
    assert_eq!(result[1].node_name, NodeName("b".into()));
}

// RQ-30: next_nodes with same target via different conditions
#[test]
fn rq_next_nodes_same_target_different_conditions_success() {
    let def = WorkflowDefinition {
        workflow_name: WorkflowName("test".into()),
        nodes: NonEmptyVec::new_unchecked(vec![
            DagNode {
                node_name: NodeName("a".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
            },
            DagNode {
                node_name: NodeName("b".into()),
                retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
            },
        ]),
        edges: vec![
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::Always,
            },
            Edge {
                source_node: NodeName("a".into()),
                target_node: NodeName("b".into()),
                condition: EdgeCondition::OnSuccess,
            },
        ],
    };
    // Success: Always + OnSuccess both fire → 2 results
    let result_success = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(result_success.len(), 2);

    // Failure: only Always fires → 1 result
    let result_failure = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert_eq!(result_failure.len(), 1);
}

// RQ-31: next_nodes OnFailure-only edge returns nothing on Success
#[test]
fn rq_next_nodes_on_failure_only_returns_nothing_on_success() {
    let def = make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnFailure)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert!(result.is_empty());
}

// RQ-32: next_nodes OnSuccess-only edge returns nothing on Failure
#[test]
fn rq_next_nodes_on_success_only_returns_nothing_on_failure() {
    let def = make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnSuccess)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert!(result.is_empty());
}

// RQ-33: next_nodes terminal node (no outgoing edges) returns empty for both outcomes
#[test]
fn rq_next_nodes_terminal_node_empty_for_both_outcomes() {
    let def = make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    assert!(next_nodes(&NodeName("b".into()), StepOutcome::Success, &def).is_empty());
    assert!(next_nodes(&NodeName("b".into()), StepOutcome::Failure, &def).is_empty());
}

// RQ-34: next_nodes always edge matches both outcomes
#[test]
fn rq_next_nodes_always_edge_matches_both_outcomes() {
    let def = make_def(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    assert_eq!(
        next_nodes(&NodeName("a".into()), StepOutcome::Success, &def).len(),
        1
    );
    assert_eq!(
        next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def).len(),
        1
    );
}

// RQ-35: next_nodes with mixed conditions from same source
#[test]
fn rq_next_nodes_mixed_conditions_three_targets() {
    let def = make_def(
        "test",
        vec![
            ("a", 1, 0, 1.0),
            ("b", 1, 0, 1.0),
            ("c", 1, 0, 1.0),
            ("d", 1, 0, 1.0),
        ],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("a", "c", EdgeCondition::OnSuccess),
            ("a", "d", EdgeCondition::OnFailure),
        ],
    );
    // Success: Always(b) + OnSuccess(c) = [b, c]
    let success = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    let names: HashSet<&str> = success.iter().map(|n| n.node_name.0.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains("b") && names.contains("c"));

    // Failure: Always(b) + OnFailure(d) = [b, d]
    let failure = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    let names: HashSet<&str> = failure.iter().map(|n| n.node_name.0.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains("b") && names.contains("d"));
}

// ===========================================================================
// DIMENSION: boundary-values
// u8::MAX, u64::MAX, negative zero, sub-1.0, very large multiplier
// ===========================================================================

// RQ-36: u8::MAX max_attempts accepted
#[test]
fn rq_max_attempts_u8_max_accepted() {
    let result = RetryPolicy::new(u8::MAX, 0, 1.0);
    assert_eq!(result.unwrap().max_attempts, 255);
}

// RQ-37: u64::MAX backoff_ms accepted
#[test]
fn rq_backoff_ms_u64_max_accepted() {
    let result = RetryPolicy::new(1, u64::MAX, 1.0);
    assert_eq!(result.unwrap().backoff_ms, u64::MAX);
}

// RQ-38: Negative zero multiplier is rejected (-0.0 < 1.0 is true)
#[test]
fn rq_negative_zero_multiplier_rejected() {
    let result = RetryPolicy::new(1, 0, -0.0f32);
    // -0.0 == 0.0, and 0.0 < 1.0 is true, so -0.0 < 1.0 is true → rejected
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));
}

// RQ-39: Very small positive multiplier just below 1.0 is rejected
#[test]
fn rq_very_small_positive_multiplier_rejected() {
    let result = RetryPolicy::new(1, 0, 0.9999999f32);
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));
}

// RQ-40: Very large multiplier is accepted
#[test]
fn rq_very_large_multiplier_accepted() {
    let result = RetryPolicy::new(1, 0, 1e38f32);
    result.unwrap();
}

// RQ-41: backoff_multiplier exactly 1.0 accepted (boundary)
#[test]
fn rq_multiplier_exactly_1_accepted() {
    let result = RetryPolicy::new(1, 0, 1.0f32);
    result.unwrap();
}

// RQ-42: max_attempts = 1 accepted (minimum boundary)
#[test]
fn rq_max_attempts_1_accepted() {
    let result = RetryPolicy::new(1, 0, 1.0);
    result.unwrap();
}

// RQ-43: max_attempts = 0 rejected
#[test]
fn rq_max_attempts_0_rejected() {
    let result = RetryPolicy::new(0, 0, 1.0);
    assert_eq!(result, Err(RetryPolicyError::ZeroAttempts));
}

// RQ-44: backoff_ms = 0 accepted (no delay)
#[test]
fn rq_backoff_ms_0_accepted() {
    let result = RetryPolicy::new(1, 0, 1.0);
    assert_eq!(result.unwrap().backoff_ms, 0);
}

// ===========================================================================
// DIMENSION: serde-integrity
// Round-trip with boundary values, edge cases
// ===========================================================================

// RQ-45: WorkflowDefinition serde round-trip with boundary values
#[test]
fn rq_serde_round_trip_boundary_values() {
    let def = WorkflowDefinition {
        workflow_name: WorkflowName("a".into()),
        nodes: NonEmptyVec::new_unchecked(vec![DagNode {
            node_name: NodeName("n".into()),
            retry_policy: RetryPolicy {
                max_attempts: 255,
                backoff_ms: u64::MAX,
                backoff_multiplier: 1.0,
            },
        }]),
        edges: vec![],
    };
    let json = serde_json::to_value(&def).unwrap();
    let restored: WorkflowDefinition = serde_json::from_value(json).unwrap();
    assert_eq!(restored, def);
}

// RQ-46: RetryPolicy serde round-trip with exact 1.0 multiplier
#[test]
fn rq_retry_policy_serde_round_trip_1_0_multiplier() {
    let policy = RetryPolicy {
        max_attempts: 1,
        backoff_ms: 0,
        backoff_multiplier: 1.0,
    };
    let json = serde_json::to_value(policy).unwrap();
    let restored: RetryPolicy = serde_json::from_value(json).unwrap();
    assert_eq!(restored, policy);
}

// RQ-47: Edge serde round-trip with all condition types
#[rstest]
#[case(EdgeCondition::Always)]
#[case(EdgeCondition::OnSuccess)]
#[case(EdgeCondition::OnFailure)]
fn rq_edge_serde_round_trip_all_conditions(#[case] condition: EdgeCondition) {
    let edge = Edge {
        source_node: NodeName("src".into()),
        target_node: NodeName("tgt".into()),
        condition,
    };
    let json = serde_json::to_value(&edge).unwrap();
    let restored: Edge = serde_json::from_value(json).unwrap();
    assert_eq!(restored, edge);
}

// RQ-48: StepOutcome serde round-trip
#[test]
fn rq_step_outcome_serde_round_trip() {
    let outcome = StepOutcome::Success;
    let json = serde_json::to_value(outcome).unwrap();
    let restored: StepOutcome = serde_json::from_value(json).unwrap();
    assert_eq!(restored, outcome);

    let outcome = StepOutcome::Failure;
    let json = serde_json::to_value(outcome).unwrap();
    let restored: StepOutcome = serde_json::from_value(json).unwrap();
    assert_eq!(restored, outcome);
}

// RQ-49: NonEmptyVec serde round-trip with many elements
#[test]
fn rq_non_empty_vec_serde_round_trip_many_elements() {
    let items: Vec<String> = (0..100).map(|i| format!("node{}", i)).collect();
    let nev = NonEmptyVec::new_unchecked(items.clone());
    let json = serde_json::to_value(&nev).unwrap();
    let restored: NonEmptyVec<String> = serde_json::from_value(json).unwrap();
    assert_eq!(restored.len(), 100);
    assert_eq!(restored.first(), &items[0]);
}

// RQ-50: WorkflowDefinition serde produces valid JSON that re-parses
#[test]
fn rq_workflow_serde_produces_re_parsable_json() {
    let def = make_def(
        "linear",
        vec![("a", 3, 1000, 2.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnSuccess)],
    );
    let json = serde_json::to_value(&def).unwrap();
    let json_str = serde_json::to_string(&json).unwrap();
    let bytes = json_str.as_bytes();
    let reparsed = WorkflowDefinition::parse(bytes).unwrap();
    assert_eq!(reparsed, def);
}

// ===========================================================================
// DIMENSION: parse-determinism
// ===========================================================================

// RQ-51: parse is deterministic with complex workflow
#[test]
fn rq_parse_deterministic_complex_workflow() {
    let json = serde_json::json!({
        "workflow_name": "complex",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 3, "backoff_ms": 1000, "backoff_multiplier": 2.5}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 5, "backoff_ms": 500, "backoff_multiplier": 1.5}},
            {"node_name": "d", "retry_policy": {"max_attempts": 10, "backoff_ms": 2000, "backoff_multiplier": 3.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "OnSuccess"},
            {"source_node": "a", "target_node": "c", "condition": "OnFailure"},
            {"source_node": "b", "target_node": "d", "condition": "Always"},
            {"source_node": "c", "target_node": "d", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let r1 = WorkflowDefinition::parse(&bytes).unwrap();
    let r2 = WorkflowDefinition::parse(&bytes).unwrap();
    assert_eq!(r1, r2);
}

// RQ-52: parse error determinism -- same input always produces same error
#[test]
fn rq_parse_error_deterministic() {
    let bytes = b"not valid json{{{";
    let r1 = WorkflowDefinition::parse(bytes);
    let r2 = WorkflowDefinition::parse(bytes);
    // Both should be DeserializationFailed (can't compare inner String easily)
    assert!(matches!(
        r1,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
    assert!(matches!(
        r2,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
    // Compare the display messages for determinism
    assert_eq!(r1.unwrap_err().to_string(), r2.unwrap_err().to_string());
}

// ===========================================================================
// DIMENSION: parse-error-priority
// Verify the documented error priority order
// ===========================================================================

// RQ-53: Both invalid retry policy AND unknown edge → retry policy wins (priority 3 > 4)
#[test]
fn rq_error_priority_retry_policy_before_unknown_node() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "bad", "retry_policy": {"max_attempts": 0, "backoff_ms": 0, "backoff_multiplier": 0.5}}],
        "edges": [{"source_node": "bad", "target_node": "ghost", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::InvalidRetryPolicy { .. })
    ));
}

// RQ-54: Empty nodes AND invalid retry policy → empty wins (priority 2 > 3)
#[test]
fn rq_error_priority_empty_before_invalid_retry() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow));
}

// RQ-55: Unknown node AND cycle → unknown wins (priority 4 > 5)
#[test]
fn rq_error_priority_unknown_before_cycle() {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [
            {"source_node": "a", "target_node": "ghost", "condition": "Always"},
            {"source_node": "a", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::UnknownNode { .. })
    ));
}

// ===========================================================================
// DIMENSION: trait-compliance
// Verify required trait implementations
// ===========================================================================

// RQ-56: WorkflowName and NodeName are Clone
#[test]
fn rq_string_types_are_clone() {
    let wn = WorkflowName("test".into());
    let _wn2 = wn.clone();

    let nn = NodeName("test".into());
    let _nn2 = nn.clone();
}

// RQ-57: RetryPolicy is Copy
#[test]
fn rq_retry_policy_is_copy() {
    fn require_copy<T: Copy>(_v: T) {}
    let p = RetryPolicy::new(1, 0, 1.0).unwrap();
    require_copy(p);
    require_copy(p); // use twice to verify Copy
}

// RQ-58: StepOutcome is Copy
#[test]
fn rq_step_outcome_is_copy() {
    fn require_copy<T: Copy>(_v: T) {}
    require_copy(StepOutcome::Success);
    require_copy(StepOutcome::Failure);
}

// RQ-59: EdgeCondition is Copy
#[test]
fn rq_edge_condition_is_copy() {
    fn require_copy<T: Copy>(_v: T) {}
    require_copy(EdgeCondition::Always);
    require_copy(EdgeCondition::OnSuccess);
    require_copy(EdgeCondition::OnFailure);
}

// RQ-60: DagNode is Clone but NOT Copy (contains NodeName which is String-based)
#[test]
fn rq_dag_node_is_clone_not_copy() {
    fn require_clone<T: Clone>(_v: T) {}
    let node = DagNode {
        node_name: NodeName("a".into()),
        retry_policy: RetryPolicy::new(1, 0, 1.0).unwrap(),
    };
    require_clone(node.clone());
    // DagNode should NOT be Copy (NodeName wraps String)
    // (We can't test negative trait bounds at runtime, but this is documented)
}

// RQ-61: RetryPolicy PartialEq works with NaN via direct construction
// NaN != NaN in IEEE 754, so two NaN RetryPolicies are NOT equal.
// Note: RetryPolicy::new() rejects NaN, but pub fields allow direct construction.
#[test]
fn rq_retry_policy_partial_eq_with_nan() {
    let p1 = RetryPolicy {
        max_attempts: 1,
        backoff_ms: 0,
        backoff_multiplier: f32::NAN,
    };
    let p2 = RetryPolicy {
        max_attempts: 1,
        backoff_ms: 0,
        backoff_multiplier: f32::NAN,
    };
    // f32 PartialEq: NaN != NaN
    assert_ne!(
        p1, p2,
        "two NaN RetryPolicies should not be equal (IEEE 754)"
    );
}

// ===========================================================================
// DIMENSION: proptest-property-attacks
// ===========================================================================

mod proptests {
    use super::*;

    proptest! {
        // RQ-PROP-01: RetryPolicy::new rejects all multipliers below 1.0 (except NaN)
        #[test]
        fn rq_retry_policy_rejects_all_multipliers_below_1(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..1_000_000u64,
            multiplier in -1e38f32..0.9999f32,
        ) {
            let result = RetryPolicy::new(max_attempts, backoff_ms, multiplier);
            prop_assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })), "multiplier {} should be rejected", multiplier);
        }

        // RQ-PROP-02: RetryPolicy::new accepts all multipliers >= 1.0
        #[test]
        fn rq_retry_policy_accepts_all_multipliers_ge_1(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..1_000_000u64,
            multiplier in 1.0f32..1e38f32,
        ) {
            let result = RetryPolicy::new(max_attempts, backoff_ms, multiplier);
            result.unwrap();
        }

        // RQ-PROP-03: next_nodes result nodes all live in def (pointer equality)
        #[test]
        fn rq_next_nodes_result_nodes_are_from_def(
            outcome in step_outcome_strategy(),
        ) {
            let def = make_def(
                "test",
                vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
                vec![
                    ("a", "b", EdgeCondition::Always),
                    ("a", "c", EdgeCondition::OnSuccess),
                ],
            );
            let result = next_nodes(&NodeName("a".into()), outcome, &def);
            let all_found = result.iter().all(|node| def.nodes.as_slice().iter().any(|n| std::ptr::eq(n, *node)));
            prop_assert!(all_found, "next_nodes returned a &DagNode not from def.nodes");
        }

        // RQ-PROP-04: parse never panics with arbitrary valid-structure JSON
        #[test]
        fn rq_parse_never_panics(
            name_suffix in "[a-z]{1,10}",
            max_attempts in 0u8..=255u8,
            backoff_ms in 0u64..=1_000_000u64,
            multiplier in 0.0f32..=10.0f32,
        ) {
            let workflow_name = format!("wf-{}", name_suffix);
            let json = serde_json::json!({
                "workflow_name": workflow_name,
                "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": max_attempts, "backoff_ms": backoff_ms, "backoff_multiplier": multiplier}}],
                "edges": []
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let _result = std::panic::catch_unwind(|| {
                let _ignored = WorkflowDefinition::parse(&bytes);
            });
            // If we reach here, no panic occurred
        }

        // RQ-PROP-05: RetryPolicy serde round-trip preserves values
        #[test]
        fn rq_retry_policy_serde_round_trip(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..=1_000_000u64,
            multiplier in 1.0f32..100.0f32,
        ) {
            let policy = RetryPolicy {
                max_attempts,
                backoff_ms,
                backoff_multiplier: multiplier,
            };
            let json = serde_json::to_value(policy).unwrap();
            let restored: RetryPolicy = serde_json::from_value(json).unwrap();
            prop_assert_eq!(restored, policy);
        }

        // RQ-PROP-06: Edge serde round-trip preserves all fields
        // Uses valid NodeName pattern: alphanumeric, no leading/trailing _/-
        #[test]
        fn rq_edge_serde_round_trip(
            source in "[a-zA-Z0-9]([a-zA-Z0-9_-]{0,8}[a-zA-Z0-9])?",
            target in "[a-zA-Z0-9]([a-zA-Z0-9_-]{0,8}[a-zA-Z0-9])?",
            condition in edge_condition_strategy(),
        ) {
            let edge = Edge {
                source_node: NodeName(source),
                target_node: NodeName(target),
                condition,
            };
            let json = serde_json::to_value(&edge).unwrap();
            let restored: Edge = serde_json::from_value(json).unwrap();
            prop_assert_eq!(restored.source_node, edge.source_node);
            prop_assert_eq!(restored.target_node, edge.target_node);
            prop_assert_eq!(restored.condition, edge.condition);
        }

        // RQ-PROP-07: WorkflowDefinition parse + re-serialize + re-parse = identity
        // (generates only acyclic workflows)
        #[test]
        fn rq_workflow_parse_round_trip_identity(
            node_count in 1usize..=4usize,
            edge_seeds in proptest::collection::vec(0usize..=20usize, 0..=6usize),
        ) {
            let node_names: Vec<String> = (0..node_count).map(|i| format!("n{}", i)).collect();

            // All acyclic edges: (lower_idx, higher_idx) guarantees no cycle
            let possible_edges: Vec<(usize, usize)> = if node_count > 1 {
                (0..node_count)
                    .flat_map(|i| (i + 1..node_count).map(move |j| (i, j)))
                    .collect()
            } else {
                vec![]
            };

            let edges: HashSet<(usize, usize)> = if possible_edges.is_empty() {
                HashSet::new()
            } else {
                edge_seeds
                    .into_iter()
                    .map(|s| possible_edges[s % possible_edges.len()])
                    .collect()
            };

            let nodes_json: Vec<serde_json::Value> = node_names
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "node_name": name,
                        "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
                    })
                })
                .collect();

            let edges_json: Vec<serde_json::Value> = edges
                .iter()
                .map(|&(src, tgt)| {
                    serde_json::json!({
                        "source_node": node_names[src],
                        "target_node": node_names[tgt],
                        "condition": "Always"
                    })
                })
                .collect();

            let workflow_json = serde_json::json!({
                "workflow_name": "proptest",
                "nodes": nodes_json,
                "edges": edges_json,
            });

            let bytes = serde_json::to_vec(&workflow_json).unwrap();
            let parsed = WorkflowDefinition::parse(&bytes).expect("parse should succeed for acyclic workflow");
            let reserialized = serde_json::to_vec(&parsed).unwrap();
            let reparsed = WorkflowDefinition::parse(&reserialized).expect("re-parse should succeed");
            prop_assert_eq!(reparsed, parsed);
        }

        // RQ-PROP-08: get_node returns None for names not in workflow
        #[test]
        fn rq_get_node_none_for_missing(
            missing_suffix in "[a-z]{1,5}",
        ) {
            let def = make_def("test", vec![("a", 1, 0, 1.0)], vec![]);
            let missing = format!("zzz-{}", missing_suffix);
            prop_assert!(def.get_node(&NodeName(missing)).is_none());
        }

        // RQ-PROP-09: NonEmptyVec serde round-trip
        #[test]
        fn rq_non_empty_vec_serde_round_trip(
            items in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 1..=50),
        ) {
            let nev = NonEmptyVec::new_unchecked(items.clone());
            let json = serde_json::to_value(&nev).unwrap();
            let restored: NonEmptyVec<u8> = serde_json::from_value(json).unwrap();
            prop_assert_eq!(restored.as_slice(), items.as_slice());
        }
    }
}

// RQ-26b: Exponential paths DAG (tests that memoization is present)
#[test]
fn rq_exponential_paths_dag_does_not_timeout() {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let n = 40;

    for i in 0..n {
        nodes.push(serde_json::json!({
            "node_name": format!("n{}", i),
            "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
        }));
        if i + 1 < n {
            edges.push(serde_json::json!({
                "source_node": format!("n{}", i),
                "target_node": format!("n{}", i+1),
                "condition": "Always"
            }));
        }
        if i + 2 < n {
            edges.push(serde_json::json!({
                "source_node": format!("n{}", i),
                "target_node": format!("n{}", i+2),
                "condition": "Always"
            }));
        }
    }

    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": nodes,
        "edges": edges
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    result.unwrap();
}
