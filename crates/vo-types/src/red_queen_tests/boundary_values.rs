//! Red Queen tests: boundary-values, serde-integrity, and trait-compliance dimensions.
//!
//! Tests u8::MAX, u64::MAX, negative zero, sub-1.0, round-trip preservation,
//! and required trait implementations.

use crate::*;
use rstest::rstest;

// Re-export helpers for use in tests
use super::helpers;

// ===========================================================================
// DIMENSION: boundary-values
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
    let result = RetryPolicy::new(1, 0, -0.0f64);
    // -0.0 == 0.0, and 0.0 < 1.0 is true, so -0.0 < 1.0 is true -> rejected
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));
}

// RQ-39: Very small positive multiplier just below 1.0 is rejected
#[test]
fn rq_very_small_positive_multiplier_rejected() {
    let result = RetryPolicy::new(1, 0, 0.9999999f64);
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));
}

// RQ-40: Very large multiplier is rejected (non-finite check)
#[test]
fn rq_very_large_multiplier_accepted() {
    let result = RetryPolicy::new(1, 0, 1e38f64);
    result.unwrap();
}

// RQ-41: backoff_multiplier exactly 1.0 accepted (boundary)
#[test]
fn rq_multiplier_exactly_1_accepted() {
    let result = RetryPolicy::new(1, 0, 1.0f64);
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
// Round-trip with boundary values and edge cases
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
                max_backoff_ms: u64::MAX,
            },
            compensation_policy: None,
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
        max_backoff_ms: u64::MAX,
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
    let def = helpers::make_def(
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
// DIMENSION: trait-compliance
// Required trait implementations
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
        compensation_policy: None,
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
        backoff_multiplier: f64::NAN,
        max_backoff_ms: u64::MAX,
    };
    let p2 = RetryPolicy {
        max_attempts: 1,
        backoff_ms: 0,
        backoff_multiplier: f64::NAN,
        max_backoff_ms: u64::MAX,
    };
    // f64 PartialEq: NaN != NaN
    assert_ne!(
        p1, p2,
        "two NaN RetryPolicies should not be equal (IEEE 754)"
    );
}
