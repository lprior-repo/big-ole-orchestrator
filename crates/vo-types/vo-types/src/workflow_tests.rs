use crate::*;
use proptest::prelude::*;
use std::collections::HashSet;

// -----------------------------------------------------------------------
// Test helper: construct a WorkflowDefinition directly (bypasses parse)
// -----------------------------------------------------------------------
fn make_workflow(
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

// -----------------------------------------------------------------------
// Proptest strategy helpers
// -----------------------------------------------------------------------
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

// ===================================================================
// StepOutcome
// ===================================================================

// B-11: StepOutcome has exactly two variants Success and Failure
#[test]
fn step_outcome_has_exactly_two_variants_when_checked() {
    // Exhaustive match ensures compiler catches missing variants
    fn _exhaustiveness(v: StepOutcome) -> bool {
        match v {
            StepOutcome::Success | StepOutcome::Failure => true,
        }
    }
    assert!(_exhaustiveness(StepOutcome::Success));
    assert!(_exhaustiveness(StepOutcome::Failure));
    // Verify exactly 2 variants
    let all: [StepOutcome; 2] = [StepOutcome::Success, StepOutcome::Failure];
    assert_eq!(all.len(), 2);
}

// B-12: StepOutcome serde round-trips for both variants
#[test]
fn step_outcome_serde_round_trips_for_both_variants() -> Result<(), Box<dyn std::error::Error>> {
    let variant = StepOutcome::Success;
    let json = serde_json::to_value(variant)?;
    let restored: StepOutcome = serde_json::from_value(json)?;
    assert_eq!(restored, variant);

    let variant = StepOutcome::Failure;
    let json = serde_json::to_value(variant)?;
    let restored: StepOutcome = serde_json::from_value(json)?;
    assert_eq!(restored, variant);

    Ok(())
}

// ===================================================================
// EdgeCondition
// ===================================================================

// B-13: EdgeCondition has exactly three variants
#[test]
fn edge_condition_has_exactly_three_variants_when_checked() {
    fn _exhaustiveness(v: EdgeCondition) -> bool {
        match v {
            EdgeCondition::Always | EdgeCondition::OnSuccess | EdgeCondition::OnFailure => true,
        }
    }
    assert!(_exhaustiveness(EdgeCondition::Always));
    assert!(_exhaustiveness(EdgeCondition::OnSuccess));
    assert!(_exhaustiveness(EdgeCondition::OnFailure));
    let all: [EdgeCondition; 3] = [
        EdgeCondition::Always,
        EdgeCondition::OnSuccess,
        EdgeCondition::OnFailure,
    ];
    assert_eq!(all.len(), 3);
}

// B-14: EdgeCondition serde round-trips for all variants
#[test]
fn edge_condition_serde_round_trips_for_all_variants() -> Result<(), Box<dyn std::error::Error>> {
    let variant = EdgeCondition::Always;
    let json = serde_json::to_value(variant)?;
    let restored: EdgeCondition = serde_json::from_value(json)?;
    assert_eq!(restored, variant);

    let variant = EdgeCondition::OnSuccess;
    let json = serde_json::to_value(variant)?;
    let restored: EdgeCondition = serde_json::from_value(json)?;
    assert_eq!(restored, variant);

    let variant = EdgeCondition::OnFailure;
    let json = serde_json::to_value(variant)?;
    let restored: EdgeCondition = serde_json::from_value(json)?;
    assert_eq!(restored, variant);

    Ok(())
}

// ===================================================================
// RetryPolicy
// ===================================================================

// B-15: RetryPolicy accepts valid parameters
#[test]
fn retry_policy_accepts_valid_params_when_all_constraints_satisfied() -> Result<(), RetryPolicyError>
{
    let policy = RetryPolicy::new(3, 1000, 2.0)?;
    assert_eq!(
        policy,
        RetryPolicy {
            max_attempts: 3,
            backoff_ms: 1000,
            backoff_multiplier: 2.0,
        }
    );
    Ok(())
}

// B-16: RetryPolicy rejects zero max_attempts
#[test]
fn retry_policy_rejects_zero_attempts_with_zero_attempts_error_when_max_is_zero() {
    let result = RetryPolicy::new(0, 100, 1.0);
    assert_eq!(result, Err(RetryPolicyError::ZeroAttempts));
}

// B-17: RetryPolicy rejects backoff_multiplier < 1.0
#[test]
fn retry_policy_rejects_low_multiplier_with_invalid_multiplier_error_when_below_1() {
    let result = RetryPolicy::new(3, 100, 0.5);
    assert_eq!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { got: 0.5 })
    );
}

// B-18: RetryPolicy priority: ZeroAttempts wins over InvalidMultiplier
#[test]
fn retry_policy_returns_zero_attempts_when_both_zero_and_low_multiplier() {
    let result = RetryPolicy::new(0, 100, 0.5);
    assert_eq!(result, Err(RetryPolicyError::ZeroAttempts));
}

// B-19: RetryPolicy accepts max_attempts = 1 (minimum boundary)
#[test]
fn retry_policy_accepts_max_attempts_1_at_minimum_boundary() -> Result<(), RetryPolicyError> {
    let policy = RetryPolicy::new(1, 100, 1.0)?;
    assert_eq!(
        policy,
        RetryPolicy {
            max_attempts: 1,
            backoff_ms: 100,
            backoff_multiplier: 1.0,
        }
    );
    Ok(())
}

// B-20: RetryPolicy accepts backoff_multiplier = 1.0 (minimum boundary)
#[test]
fn retry_policy_accepts_multiplier_1_at_minimum_boundary() -> Result<(), RetryPolicyError> {
    let policy = RetryPolicy::new(1, 100, 1.0)?;
    assert_eq!(
        policy,
        RetryPolicy {
            max_attempts: 1,
            backoff_ms: 100,
            backoff_multiplier: 1.0,
        }
    );
    Ok(())
}

// B-21: RetryPolicy accepts max_attempts = 255 (u8::MAX)
#[test]
fn retry_policy_accepts_max_attempts_255_at_maximum_boundary() -> Result<(), RetryPolicyError> {
    let policy = RetryPolicy::new(255, 100, 1.0)?;
    assert_eq!(
        policy,
        RetryPolicy {
            max_attempts: 255,
            backoff_ms: 100,
            backoff_multiplier: 1.0,
        }
    );
    Ok(())
}

// B-22: RetryPolicy accepts backoff_ms = 0
#[test]
fn retry_policy_accepts_backoff_ms_zero_when_no_delay_requested() -> Result<(), RetryPolicyError> {
    let policy = RetryPolicy::new(1, 0, 1.0)?;
    assert_eq!(
        policy,
        RetryPolicy {
            max_attempts: 1,
            backoff_ms: 0,
            backoff_multiplier: 1.0,
        }
    );
    Ok(())
}

// B-23: RetryPolicy serde round-trip
#[test]
fn retry_policy_serde_round_trips_for_valid_policy() -> Result<(), Box<dyn std::error::Error>> {
    let policy = RetryPolicy {
        max_attempts: 5,
        backoff_ms: 2000,
        backoff_multiplier: 1.5,
    };
    let json = serde_json::to_value(policy)?;
    let restored: RetryPolicy = serde_json::from_value(json)?;
    assert_eq!(restored, policy);
    Ok(())
}

// ===================================================================
// RetryPolicyError display
// ===================================================================

// B-24: RetryPolicyError::ZeroAttempts display
#[test]
fn retry_policy_error_zero_attempts_displays_correct_message_when_formatted() {
    let err = RetryPolicyError::ZeroAttempts;
    let msg = err.to_string();
    assert!(msg.contains("max_attempts must be >= 1, got 0"));
}

// B-25: RetryPolicyError::InvalidMultiplier display
#[test]
fn retry_policy_error_invalid_multiplier_displays_got_value_when_formatted() {
    let err = RetryPolicyError::InvalidMultiplier { got: 0.5 };
    let msg = err.to_string();
    assert!(msg.contains("backoff_multiplier must be >= 1.0, got 0.5"));
}

// ===================================================================
// DagNode
// ===================================================================

// B-26: DagNode has no binary_path field (verified via serialization)
#[test]
fn dag_node_has_no_binary_path_field_when_serialized() -> Result<(), Box<dyn std::error::Error>> {
    let node = DagNode {
        node_name: NodeName("a".into()),
        retry_policy: RetryPolicy {
            max_attempts: 1,
            backoff_ms: 0,
            backoff_multiplier: 1.0,
        },
    };
    let value = serde_json::to_value(&node)?;
    let obj = value.as_object().ok_or("expected JSON object")?;
    assert!(
        !obj.contains_key("binary_path"),
        "DagNode must not have binary_path field"
    );
    assert!(obj.contains_key("node_name"));
    assert!(obj.contains_key("retry_policy"));
    assert_eq!(obj["node_name"], "a");
    let rp = &obj["retry_policy"];
    assert!(rp.get("max_attempts").is_some());
    assert!(rp.get("backoff_ms").is_some());
    assert!(rp.get("backoff_multiplier").is_some());
    assert_eq!(obj.keys().len(), 2);
    Ok(())
}

// ===================================================================
// Edge
// ===================================================================

// B-27: Edge holds source_node, target_node, and condition fields
#[test]
fn edge_holds_source_target_and_condition_when_constructed() {
    let edge = Edge {
        source_node: NodeName("a".into()),
        target_node: NodeName("b".into()),
        condition: EdgeCondition::Always,
    };
    assert_eq!(edge.source_node, NodeName("a".into()));
    assert_eq!(edge.target_node, NodeName("b".into()));
    assert_eq!(edge.condition, EdgeCondition::Always);
}

// B-28: Edge serde round-trip
#[test]
fn edge_serde_round_trips_for_valid_edge() -> Result<(), Box<dyn std::error::Error>> {
    let edge = Edge {
        source_node: NodeName("x".into()),
        target_node: NodeName("y".into()),
        condition: EdgeCondition::OnSuccess,
    };
    let json = serde_json::to_value(&edge)?;
    let restored: Edge = serde_json::from_value(json)?;
    assert_eq!(restored, edge);
    Ok(())
}

// ===================================================================
// WorkflowDefinition::parse
// ===================================================================

// B-29: parse accepts valid single-node workflow with no edges
#[test]
fn parse_accepts_single_node_workflow_when_no_edges() -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "solo",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let def = WorkflowDefinition::parse(&bytes)?;
    assert_eq!(def.workflow_name, WorkflowName("solo".into()));
    assert_eq!(def.nodes.len(), 1);
    assert_eq!(def.nodes.first().node_name, NodeName("a".into()));
    assert_eq!(
        def.nodes.first().retry_policy,
        RetryPolicy {
            max_attempts: 1,
            backoff_ms: 0,
            backoff_multiplier: 1.0,
        }
    );
    assert_eq!(def.edges.len(), 0);
    Ok(())
}

// B-30: parse accepts linear 3-node workflow
#[test]
fn parse_linear_3_node_workflow_a_b_c_succeeds_and_next_nodes_a_returns_b(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "linear",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "c", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json)?;
    let def = WorkflowDefinition::parse(&bytes)?;
    assert_eq!(def.workflow_name, WorkflowName("linear".into()));
    assert_eq!(def.nodes.len(), 3);
    assert_eq!(def.edges.len(), 2);
    assert_eq!(def.edges[0].source_node, NodeName("a".into()));
    assert_eq!(def.edges[0].target_node, NodeName("b".into()));
    assert_eq!(def.edges[0].condition, EdgeCondition::Always);
    assert_eq!(def.edges[1].source_node, NodeName("b".into()));
    assert_eq!(def.edges[1].target_node, NodeName("c".into()));
    assert_eq!(def.edges[1].condition, EdgeCondition::Always);
    let successors = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(successors.len(), 1);
    assert_eq!(successors[0].node_name, NodeName("b".into()));
    Ok(())
}

// B-31: parse accepts diamond workflow
#[test]
fn parse_diamond_workflow_succeeds_and_next_nodes_a_returns_b_and_c(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "diamond",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "d", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "a", "target_node": "c", "condition": "Always"},
            {"source_node": "b", "target_node": "d", "condition": "OnSuccess"},
            {"source_node": "c", "target_node": "d", "condition": "OnSuccess"}
        ]
    });
    let bytes = serde_json::to_vec(&json)?;
    let def = WorkflowDefinition::parse(&bytes)?;
    assert_eq!(def.workflow_name, WorkflowName("diamond".into()));
    assert_eq!(def.nodes.len(), 4);
    assert_eq!(def.edges.len(), 4);
    let successors = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    let names: HashSet<&str> = successors.iter().map(|n| n.node_name.as_str()).collect();
    assert!(names.contains("b"));
    assert!(names.contains("c"));
    Ok(())
}

// B-32: parse rejects empty nodes list
#[test]
fn parse_empty_nodes_returns_empty_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "empty",
        "nodes": [],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow));
    Ok(())
}

// B-33: parse rejects invalid JSON
#[test]
fn parse_rejects_malformed_json_with_deserialization_failed_when_bytes_invalid() {
    let bytes = b"not valid json{{{";
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// B-34: parse rejects missing required fields
#[test]
fn parse_rejects_missing_fields_with_deserialization_failed_when_json_incomplete() {
    let bytes = br#"{"workflow_name": "test"}"#;
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
}

// B-35: parse rejects node with zero max_attempts
#[test]
fn parse_rejects_zero_max_attempts_with_invalid_retry_policy_when_node_has_zero_attempts(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "bad-retry",
        "nodes": [{"node_name": "bad_node", "retry_policy": {"max_attempts": 0, "backoff_ms": 100, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::InvalidRetryPolicy {
            node_name: NodeName("bad_node".into()),
            reason: RetryPolicyError::ZeroAttempts,
        })
    );
    Ok(())
}

// B-36: parse rejects node with low backoff_multiplier
#[test]
fn parse_rejects_low_multiplier_with_invalid_retry_policy_when_node_has_low_multiplier(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "bad-retry",
        "nodes": [{"node_name": "bad_node", "retry_policy": {"max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 0.5}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::InvalidRetryPolicy {
            node_name: NodeName("bad_node".into()),
            reason: RetryPolicyError::InvalidMultiplier { got: 0.5 },
        })
    );
    Ok(())
}

// B-37: parse rejects edge with unknown target node
#[test]
fn parse_rejects_dangling_edge_with_unknown_node_when_target_missing(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "ghost", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::UnknownNode {
            edge_source: NodeName("a".into()),
            unknown_target: NodeName("ghost".into()),
        })
    );
    Ok(())
}

// B-38: parse rejects edge with unknown source node
#[test]
fn parse_rejects_dangling_edge_with_unknown_node_when_source_missing(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": [{"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "phantom", "target_node": "b", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::UnknownNode {
            edge_source: NodeName("phantom".into()),
            unknown_target: NodeName("phantom".into()),
        })
    );
    Ok(())
}

// B-39: parse rejects cyclic workflow (a -> b -> a)
#[test]
fn parse_cyclic_workflow_a_b_a_returns_cycle_detected() -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "cycle",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::CycleDetected {
            cycle_nodes: vec![
                NodeName("a".into()),
                NodeName("b".into()),
                NodeName("a".into()),
            ],
        })
    );
    Ok(())
}

// B-40: parse rejects self-loop (a -> a)
#[test]
fn parse_rejects_self_loop_with_cycle_detected_when_node_edges_to_itself(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "self-loop",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "a", "target_node": "a", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::CycleDetected {
            cycle_nodes: vec![NodeName("a".into()), NodeName("a".into())],
        })
    );
    Ok(())
}

// B-65: parse rejects 3-node cycle (a -> b -> c -> a)
#[test]
fn parse_rejects_3_node_cycle_a_b_c_a_with_cycle_detected() -> Result<(), Box<dyn std::error::Error>>
{
    let json = serde_json::json!({
        "workflow_name": "3-cycle",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "c", "condition": "Always"},
            {"source_node": "c", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::CycleDetected {
            cycle_nodes: vec![
                NodeName("a".into()),
                NodeName("b".into()),
                NodeName("c".into()),
                NodeName("a".into()),
            ],
        })
    );
    Ok(())
}

// B-41: error priority -- DeserializationFailed before EmptyWorkflow
#[test]
fn parse_returns_deserialization_failed_before_empty_workflow_when_json_malformed() {
    let bytes = b"not valid json{{{";
    let result = WorkflowDefinition::parse(bytes);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::DeserializationFailed { .. })
    ));
    assert!(!matches!(
        result,
        Err(WorkflowDefinitionError::EmptyWorkflow)
    ));
}

// B-42: error priority -- EmptyWorkflow before InvalidRetryPolicy
#[test]
fn parse_returns_empty_workflow_before_invalid_retry_policy_when_nodes_empty(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "empty",
        "nodes": [],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow));
    Ok(())
}

// B-43: error priority -- InvalidRetryPolicy before UnknownNode
#[test]
fn parse_returns_invalid_retry_policy_before_unknown_node_when_both_present(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "both",
        "nodes": [{"node_name": "bad_node", "retry_policy": {"max_attempts": 0, "backoff_ms": 100, "backoff_multiplier": 1.0}}],
        "edges": [{"source_node": "bad_node", "target_node": "ghost", "condition": "Always"}]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::InvalidRetryPolicy {
            node_name: NodeName("bad_node".into()),
            reason: RetryPolicyError::ZeroAttempts,
        })
    );
    Ok(())
}

// B-44: error priority -- UnknownNode before CycleDetected
#[test]
fn parse_returns_unknown_node_before_cycle_detected_when_both_present(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "both",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": [
            {"source_node": "a", "target_node": "ghost", "condition": "Always"},
            {"source_node": "a", "target_node": "a", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json)?;
    let result = WorkflowDefinition::parse(&bytes);
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::UnknownNode {
            edge_source: NodeName("a".into()),
            unknown_target: NodeName("ghost".into()),
        })
    );
    Ok(())
}

// B-45: get_node returns Some when node exists
#[test]
fn get_node_returns_some_when_node_name_exists() -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "solo",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let def = WorkflowDefinition::parse(&bytes)?;
    let node = def
        .get_node(&NodeName("a".into()))
        .ok_or("expected Some for node 'a'")?;
    assert_eq!(node.node_name, NodeName("a".into()));
    Ok(())
}

// B-46: get_node returns None when node does not exist
#[test]
fn get_node_returns_none_when_node_name_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "solo",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let def = WorkflowDefinition::parse(&bytes)?;
    let node = def.get_node(&NodeName("nonexistent".into()));
    assert_eq!(node, None);
    Ok(())
}

// B-47: parse is deterministic
#[test]
fn parse_returns_identical_result_when_called_twice_with_same_input(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "solo",
        "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
        "edges": []
    });
    let bytes = serde_json::to_vec(&json)?;
    let result1 = WorkflowDefinition::parse(&bytes)?;
    let result2 = WorkflowDefinition::parse(&bytes)?;
    assert_eq!(result1, result2);
    assert_eq!(result1.workflow_name, WorkflowName("solo".into()));
    assert_eq!(result1.nodes.len(), 1);
    assert_eq!(result1.edges.len(), 0);
    Ok(())
}

// B-48: WorkflowDefinition JSON round-trip
#[test]
fn workflow_definition_json_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "linear",
        "nodes": [
            {"node_name": "a", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "b", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}},
            {"node_name": "c", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}
        ],
        "edges": [
            {"source_node": "a", "target_node": "b", "condition": "Always"},
            {"source_node": "b", "target_node": "c", "condition": "Always"}
        ]
    });
    let bytes = serde_json::to_vec(&json)?;
    let def = WorkflowDefinition::parse(&bytes)?;
    let reserialized = serde_json::to_value(&def)?;
    let reparsed = WorkflowDefinition::parse(&serde_json::to_vec(&reserialized)?)?;
    assert_eq!(reparsed, def);
    assert_eq!(reparsed.workflow_name, WorkflowName("linear".into()));
    assert_eq!(reparsed.nodes.len(), 3);
    assert_eq!(reparsed.edges.len(), 2);
    Ok(())
}

// ===================================================================
// next_nodes()
// ===================================================================

// B-49: next_nodes returns single successor when Always edge
#[test]
fn next_nodes_returns_successor_when_always_edge_matches() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].node_name, NodeName("b".into()));
}

// B-49b: next_nodes returns successor when Always edge AND Failure outcome
// Kills mutation M1 (EdgeCondition::Always → OnSuccess).
#[test]
fn next_nodes_returns_successor_when_always_edge_and_failure_outcome() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].node_name, NodeName("b".into()));
}

// B-50: next_nodes returns successor when OnSuccess edge and Success outcome
#[test]
fn next_nodes_with_on_success_edge_routes_correctly_on_success_outcome() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnSuccess)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].node_name, NodeName("b".into()));
}

// B-51: next_nodes returns empty vec when OnSuccess edge and Failure outcome
#[test]
fn next_nodes_returns_empty_when_on_success_edge_and_failure_outcome() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnSuccess)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert!(result.is_empty());
}

// B-52: next_nodes returns successor when OnFailure edge and Failure outcome
#[test]
fn next_nodes_with_on_failure_edge_routes_correctly_on_failure_outcome() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnFailure)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].node_name, NodeName("b".into()));
}

// B-53: next_nodes returns empty vec when OnFailure edge and Success outcome
#[test]
fn next_nodes_returns_empty_when_on_failure_edge_and_success_outcome() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::OnFailure)],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert!(result.is_empty());
}

// B-54: next_nodes returns empty vec when node has no outgoing edges
#[test]
fn next_nodes_returns_empty_when_node_has_no_outgoing_edges() {
    let def = make_workflow("test", vec![("z", 1, 0, 1.0)], vec![]);
    let result_success = next_nodes(&NodeName("z".into()), StepOutcome::Success, &def);
    let result_failure = next_nodes(&NodeName("z".into()), StepOutcome::Failure, &def);
    assert!(result_success.is_empty());
    assert!(result_failure.is_empty());
}

// B-55: next_nodes returns multiple successors for diamond fan-out
#[test]
fn next_nodes_returns_multiple_successors_when_multiple_always_edges() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("a", "c", EdgeCondition::Always),
        ],
    );
    let result = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(result.len(), 2);
    let names: HashSet<&str> = result.iter().map(|n| n.node_name.as_str()).collect();
    assert!(names.contains("b"));
    assert!(names.contains("c"));
}

// B-56: next_nodes respects mixed edge conditions
#[test]
fn next_nodes_returns_correct_nodes_when_mixed_edge_conditions() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("a", "c", EdgeCondition::OnSuccess),
        ],
    );
    // Success: Always + OnSuccess both fire
    let result_success = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(result_success.len(), 2);
    let names_success: HashSet<&str> = result_success
        .iter()
        .map(|n| n.node_name.as_str())
        .collect();
    assert!(names_success.contains("b"));
    assert!(names_success.contains("c"));

    // Failure: only Always fires
    let result_failure = next_nodes(&NodeName("a".into()), StepOutcome::Failure, &def);
    assert_eq!(result_failure.len(), 1);
    assert_eq!(result_failure[0].node_name, NodeName("b".into()));
}

// B-57: next_nodes for linear chain returns next hop
#[test]
fn next_nodes_returns_next_hop_when_linear_chain_traversed() {
    let def = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![
            ("a", "b", EdgeCondition::Always),
            ("b", "c", EdgeCondition::Always),
        ],
    );
    let from_a = next_nodes(&NodeName("a".into()), StepOutcome::Success, &def);
    assert_eq!(from_a.len(), 1);
    assert_eq!(from_a[0].node_name, NodeName("b".into()));

    let from_b = next_nodes(&NodeName("b".into()), StepOutcome::Success, &def);
    assert_eq!(from_b.len(), 1);
    assert_eq!(from_b[0].node_name, NodeName("c".into()));
}

// ===================================================================
// WorkflowDefinitionError display
// ===================================================================

// B-58: DeserializationFailed display
#[test]
fn workflow_definition_error_deserialization_failed_displays_message_when_formatted(
) -> Result<(), Box<dyn std::error::Error>> {
    let source_err = serde_json::from_str::<serde_json::Value>("{{{").unwrap_err();
    let err = WorkflowDefinitionError::DeserializationFailed {
        message: source_err.to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("workflow definition deserialization failed"));
    Ok(())
}

// B-59: EmptyWorkflow display
#[test]
fn workflow_definition_error_empty_workflow_displays_message_when_formatted() {
    let err = WorkflowDefinitionError::EmptyWorkflow;
    let msg = err.to_string();
    assert!(msg.contains("workflow definition must contain at least one node"));
}

// B-60: CycleDetected display
#[test]
fn workflow_definition_error_cycle_detected_displays_node_names_when_formatted() {
    let err = WorkflowDefinitionError::CycleDetected {
        cycle_nodes: vec![NodeName("a".into()), NodeName("b".into())],
    };
    let msg = err.to_string();
    assert!(msg.contains("cycle"));
    assert!(msg.contains("a"));
    assert!(msg.contains("b"));
}

// B-61: UnknownNode display
#[test]
fn workflow_definition_error_unknown_node_displays_names_when_formatted() {
    let err = WorkflowDefinitionError::UnknownNode {
        edge_source: NodeName("a".into()),
        unknown_target: NodeName("ghost".into()),
    };
    let msg = err.to_string();
    assert!(msg.contains("a"));
    assert!(msg.contains("ghost"));
    assert!(msg.contains("unknown target node"));
}

// B-62: InvalidRetryPolicy display
#[test]
fn workflow_definition_error_invalid_retry_policy_displays_node_and_reason_when_formatted() {
    let err = WorkflowDefinitionError::InvalidRetryPolicy {
        node_name: NodeName("a".into()),
        reason: RetryPolicyError::ZeroAttempts,
    };
    let msg = err.to_string();
    assert!(msg.contains("a"));
    assert!(msg.contains("max_attempts"));
}

// ===================================================================
// Proptests
// ===================================================================
mod proptests {
    use super::*;

    proptest! {
        /// Invariant: For all valid RetryPolicy inputs, new() returns Ok
        /// and all fields are preserved.
        #[test]
        fn retry_policy_new_proptest_accepted_values_satisfy_invariants(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..1_000_000u64,
            backoff_multiplier in 1.0f32..1e10f32,
        ) {
            let policy = RetryPolicy::new(max_attempts, backoff_ms, backoff_multiplier)?;
            prop_assert_eq!(policy.max_attempts, max_attempts);
            prop_assert_eq!(policy.backoff_ms, backoff_ms);
            prop_assert_eq!(policy.backoff_multiplier, backoff_multiplier);
        }

        /// Anti-invariant: max_attempts = 0 always fails with ZeroAttempts
        #[test]
        fn retry_policy_new_proptest_zero_attempts_always_fails(
            backoff_ms in 0u64..1_000_000u64,
            backoff_multiplier in 1.0f32..100.0f32,
        ) {
            let result = RetryPolicy::new(0, backoff_ms, backoff_multiplier);
            prop_assert_eq!(result, Err(RetryPolicyError::ZeroAttempts));
        }

        /// Anti-invariant: backoff_multiplier < 1.0 always fails
        #[test]
        fn retry_policy_new_proptest_low_multiplier_always_fails(
            max_attempts in 1u8..=255u8,
            backoff_multiplier in -1e10f32..0.9999f32,
        ) {
            let result = RetryPolicy::new(max_attempts, 0, backoff_multiplier);
            let is_invalid = matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. }));
            prop_assert!(is_invalid);
        }

        /// Invariant: Edge serde round-trip
        #[test]
        fn edge_serde_round_trip_proptest(
            source in "[a-zA-Z0-9][a-zA-Z0-9_-]{0,126}[a-zA-Z0-9]",
            target in "[a-zA-Z0-9][a-zA-Z0-9_-]{0,126}[a-zA-Z0-9]",
            condition in edge_condition_strategy(),
        ) {
            let edge = Edge {
                source_node: NodeName(source),
                target_node: NodeName(target),
                condition,
            };
            let json = serde_json::to_value(&edge).expect("serialize");
            let restored: Edge = serde_json::from_value(json).expect("deserialize");
            prop_assert_eq!(restored.source_node, edge.source_node);
            prop_assert_eq!(restored.target_node, edge.target_node);
            prop_assert_eq!(restored.condition, edge.condition);
        }

        /// Invariant: RetryPolicy serde round-trip
        #[test]
        fn retry_policy_serde_round_trip_proptest(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..1_000_000u64,
            backoff_multiplier in 1.0f32..100.0f32,
        ) {
            let policy = RetryPolicy {
                max_attempts,
                backoff_ms,
                backoff_multiplier,
            };
            let json = serde_json::to_value(policy).expect("serialize");
            let restored: RetryPolicy = serde_json::from_value(json).expect("deserialize");
            prop_assert_eq!(restored, policy);
        }

        /// Invariant: next_nodes always returns nodes from def
        #[test]
        fn next_nodes_always_returns_nodes_from_def_proptest(
            outcome in step_outcome_strategy(),
        ) {
            let def = make_workflow(
                "test",
                vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0)],
                vec![("a", "b", EdgeCondition::Always)],
            );
            let result = next_nodes(&NodeName("a".into()), outcome, &def);
            let all_found = result.iter().all(|node| def.nodes.as_slice().iter().any(|n| n.node_name == node.node_name));
            prop_assert!(all_found, "next_nodes returned node not in def");
        }

        /// Invariant: next_nodes result matches edge targets
        #[test]
        fn next_nodes_matches_edge_targets_proptest(
            outcome in step_outcome_strategy(),
        ) {
            let def = make_workflow(
                "test",
                vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
                vec![
                    ("a", "b", EdgeCondition::Always),
                    ("a", "c", EdgeCondition::OnSuccess),
                ],
            );
            let result = next_nodes(&NodeName("a".into()), outcome, &def);
            let result_names: HashSet<String> =
                result.iter().map(|n| n.node_name.0.clone()).collect();

            let expected: HashSet<String> = def
                .edges
                .iter()
                .filter(|e| {
                    e.source_node == NodeName("a".into())
                        && edge_matches_outcome(&e.condition, &outcome)
                })
                .map(|e| e.target_node.0.clone())
                .collect();
            prop_assert_eq!(result_names, expected);
        }

        /// Invariant: For any valid acyclic workflow JSON, parse succeeds
        /// and re-parsing the re-serialized result yields an identical struct.
        #[test]
        fn workflow_definition_parse_serialize_round_trip_proptest(
            node_count in 1usize..=5usize,
            edge_seeds in proptest::collection::vec(0usize..=100usize, 0..=10usize),
            max_attempts in 1u8..=10u8,
            backoff_ms in 0u64..=10000u64,
            backoff_multiplier in 1.0f32..=100.0f32,
        ) {
            // Generate unique valid node names (identifiers: start with letter, then alnum/hyphen/underscore)
            let node_names: Vec<String> = (0..node_count)
                .map(|i| format!("node{}", i))
                .collect();

            // All possible acyclic edges: (lower_idx, higher_idx) guarantees no cycles
            let possible_edges: Vec<(usize, usize)> = if node_count > 1 {
                (0..node_count)
                    .flat_map(|i| (i + 1..node_count).map(move |j| (i, j)))
                    .collect()
            } else {
                vec![]
            };

            // Select a random subset of valid edges via seeds
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
                        "retry_policy": {
                            "max_attempts": max_attempts,
                            "backoff_ms": backoff_ms,
                            "backoff_multiplier": backoff_multiplier,
                        }
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
                "workflow_name": "proptest-workflow",
                "nodes": nodes_json,
                "edges": edges_json,
            });

            let bytes = serde_json::to_vec(&workflow_json)
                .expect("failed to serialize workflow JSON");

            // Step 1: parse should succeed for valid acyclic workflow
            let parsed = WorkflowDefinition::parse(&bytes)
                .expect("parse should succeed for valid acyclic workflow");

            // Step 2: re-serialize the parsed definition
            let reserialized_bytes = serde_json::to_vec(&parsed)
                .expect("failed to re-serialize parsed definition");

            // Step 3: re-parse should also succeed
            let reparsed = WorkflowDefinition::parse(&reserialized_bytes)
                .expect("re-parse should succeed");

            // Step 4: the two WorkflowDefinitions must be identical
            prop_assert_eq!(reparsed, parsed);
        }
    }
}

/// Helper for proptests: check if an EdgeCondition matches a StepOutcome
fn edge_matches_outcome(condition: &EdgeCondition, outcome: &StepOutcome) -> bool {
    match condition {
        EdgeCondition::Always => true,
        EdgeCondition::OnSuccess => matches!(outcome, StepOutcome::Success),
        EdgeCondition::OnFailure => matches!(outcome, StepOutcome::Failure),
    }
}
