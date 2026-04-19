//! Topology tests

use std::collections::HashMap;

use crate::compensation_order::{
    compute_compensation_order, detect_cycle, filter_compensatable, validate_dependencies,
    CompensationNode, CompensationOrderResult, CompensationPolicy, OrderingError,
};

fn node(id: &str, deps: &[&str]) -> CompensationNode {
    CompensationNode {
        effect_id: id.to_string(),
        dependencies: deps.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn empty_input_returns_empty() {
    let result = compute_compensation_order(vec![]).expect("empty input should not error");
    assert_eq!(result, CompensationOrderResult::Empty);
}

#[test]
fn single_node_returns_ordered() {
    let nodes = vec![node("a", &[])];
    let result = compute_compensation_order(nodes).expect("single node should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            assert_eq!(execution_order, vec!["a"]);
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

#[test]
fn linear_chain_a_to_b_to_c() {
    let nodes = vec![node("c", &["b"]), node("b", &["a"]), node("a", &[])];
    let result = compute_compensation_order(nodes).expect("linear chain should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            let pos_a = execution_order
                .iter()
                .position(|x| x == "a")
                .expect("a missing");
            let pos_b = execution_order
                .iter()
                .position(|x| x == "b")
                .expect("b missing");
            let pos_c = execution_order
                .iter()
                .position(|x| x == "c")
                .expect("c missing");
            assert!(pos_c < pos_b, "c must come before b in compensation order");
            assert!(pos_b < pos_a, "b must come before a in compensation order");
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

#[test]
fn diamond_dependency() {
    let nodes = vec![
        node("d", &[]),
        node("c", &["d"]),
        node("b", &["d"]),
        node("a", &["b", "c"]),
    ];
    let result = compute_compensation_order(nodes).expect("diamond should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            let pos =
                |id: &str| -> usize { execution_order.iter().position(|x| x == id).expect(id) };
            assert!(pos("a") < pos("b"), "a must come before b");
            assert!(pos("a") < pos("c"), "a must come before c");
            assert!(pos("b") < pos("d"), "b must come before d");
            assert!(pos("c") < pos("d"), "c must come before d");
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

#[test]
fn cycle_detected() {
    let nodes = vec![node("a", &["b"]), node("b", &["c"]), node("c", &["a"])];
    let result = compute_compensation_order(nodes);
    match result {
        Err(OrderingError::CycleDetected { cycle_members }) => {
            assert!(
                !cycle_members.is_empty(),
                "cycle members should not be empty"
            );
        }
        Ok(CompensationOrderResult::CycleDetected { cycle_members }) => {
            assert!(!cycle_members.is_empty());
        }
        other => panic!("expected cycle detection, got {:?}", other),
    }
}

#[test]
fn self_referencing_is_cycle() {
    let nodes = vec![node("a", &["a"])];
    let result = compute_compensation_order(nodes);
    match result {
        Err(OrderingError::CycleDetected { cycle_members }) => {
            assert!(cycle_members.contains(&"a".to_string()));
        }
        Ok(CompensationOrderResult::CycleDetected { cycle_members }) => {
            assert!(cycle_members.contains(&"a".to_string()));
        }
        other => panic!("expected cycle detection for self-ref, got {:?}", other),
    }
}

#[test]
fn detect_cycle_returns_some_for_cyclic_graph() {
    let nodes = vec![node("x", &["y"]), node("y", &["x"])];
    let cycle = detect_cycle(&nodes);
    assert!(
        cycle.is_some(),
        "detect_cycle should return Some for cyclic graph"
    );
}

#[test]
fn detect_cycle_returns_none_for_dag() {
    let nodes = vec![node("a", &[]), node("b", &["a"])];
    let cycle = detect_cycle(&nodes);
    assert!(cycle.is_none(), "detect_cycle should return None for DAG");
}

#[test]
fn validate_dependencies_rejects_unknown() {
    let nodes = vec![node("a", &["nonexistent"])];
    let result = validate_dependencies(&nodes);
    assert!(
        matches!(result, Err(OrderingError::UnknownDependency { .. })),
        "expected UnknownDependency error, got {:?}",
        result
    );
}

#[test]
fn validate_dependencies_rejects_duplicates() {
    let nodes = vec![node("a", &[]), node("a", &[])];
    let result = validate_dependencies(&nodes);
    assert!(
        matches!(result, Err(OrderingError::DuplicateEffectId { .. })),
        "expected DuplicateEffectId error, got {:?}",
        result
    );
}

#[test]
fn two_independent_nodes_any_order() {
    let nodes = vec![node("a", &[]), node("b", &[])];
    let result = compute_compensation_order(nodes).expect("independent nodes should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            assert_eq!(execution_order.len(), 2);
            assert!(execution_order.contains(&"a".to_string()));
            assert!(execution_order.contains(&"b".to_string()));
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

#[test]
fn filter_compensatable_removes_not_needed() {
    let nodes = vec![node("a", &[]), node("b", &[]), node("c", &[])];
    let mut policies = HashMap::new();
    policies.insert("a".to_string(), CompensationPolicy::Required);
    policies.insert("b".to_string(), CompensationPolicy::NotNeeded);
    policies.insert("c".to_string(), CompensationPolicy::BestEffort);

    let filtered = filter_compensatable(&nodes, &policies);
    let ids: Vec<&str> = filtered.iter().map(|n| n.effect_id.as_str()).collect();
    assert!(ids.contains(&"a"), "required should be kept");
    assert!(!ids.contains(&"b"), "not-needed should be removed");
    assert!(ids.contains(&"c"), "best-effort should be kept");
}

// ========================================================================
// Saga Failure Compensation Tests (ve-2rmt5)
// ========================================================================
// When a saga step fails, all previous (completed) steps must compensate
// in reverse dependency order. The topological sort produces this order.

/// Given: A 3-step saga (charge → reserve → ship) where ship fails
/// When: Compensation order is computed for the first 2 steps
/// Then: Compensation runs in reverse: reserve, then charge
#[test]
fn saga_simple_failure_compensates_in_reverse() {
    // Forward execution: charge -> reserve -> ship
    // Ship fails, so compensate: reserve, charge (reverse order)
    let completed_steps = vec![
        node("charge", &[]),
        node("reserve", &["charge"]),
    ];
    let result = compute_compensation_order(completed_steps).expect("should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            // Reverse dependency order: reserve first, then charge
            let pos = |id: &str| -> usize {
                execution_order.iter().position(|x| x == id).expect(id)
            };
            assert!(
                pos("reserve") < pos("charge"),
                "reserve must compensate before charge (it depended on charge)"
            );
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

/// Given: A 5-step saga where step 4 fails
/// When: Compensation order is computed for steps 1-3
/// Then: All compensate in reverse dependency order (last first)
#[test]
fn saga_multi_step_failure_compensates_all_previous() {
    // Forward: step1 -> step2 -> step3 -> step4(fails)
    // Compensate: step3, step2, step1
    let completed_steps = vec![
        node("step1", &[]),
        node("step2", &["step1"]),
        node("step3", &["step2"]),
    ];
    let result = compute_compensation_order(completed_steps).expect("should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            let pos = |id: &str| -> usize {
                execution_order.iter().position(|x| x == id).expect(id)
            };
            assert!(pos("step3") < pos("step2"), "step3 compensates before step2");
            assert!(pos("step2") < pos("step1"), "step2 compensates before step1");
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

/// Given: A saga with diamond-shaped dependencies where a leaf fails
/// When: Compensation is computed for the diamond
/// Then: Both branches compensate before the root
#[test]
fn saga_diamond_failure_compensates_branches_before_root() {
    // Forward: charge -> [reserve, warehouse] -> ship(fails)
    // Compensate: reserve, warehouse (any order), then charge
    let completed_steps = vec![
        node("charge", &[]),
        node("reserve", &["charge"]),
        node("warehouse", &["charge"]),
    ];
    let result = compute_compensation_order(completed_steps).expect("should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            let pos = |id: &str| -> usize {
                execution_order.iter().position(|x| x == id).expect(id)
            };
            // Both reserve and warehouse must compensate before charge
            assert!(pos("reserve") < pos("charge"), "reserve before charge");
            assert!(pos("warehouse") < pos("charge"), "warehouse before charge");
            // All 3 must be present
            assert_eq!(execution_order.len(), 3);
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

/// Given: A saga where only the first step succeeded before failure
/// When: Compensation order is computed for that single step
/// Then: Only that step needs compensation
#[test]
fn saga_failure_at_second_step_compensates_only_first() {
    let completed_steps = vec![node("step1", &[])];
    let result = compute_compensation_order(completed_steps).expect("should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            assert_eq!(execution_order, vec!["step1"]);
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}

/// Given: A saga with mixed policies where some steps don't need compensation
/// When: Step fails and compensation is filtered
/// Then: Only Required/BestEffort steps compensate, NotNeeded skipped
#[test]
fn saga_failure_skips_not_needed_compensation() {
    let steps = vec![
        node("charge", &[]),
        node("reserve", &["charge"]),
        node("notify", &[]),  // notification doesn't need compensation
    ];
    let mut policies = HashMap::new();
    policies.insert("charge".to_string(), CompensationPolicy::Required);
    policies.insert("reserve".to_string(), CompensationPolicy::Required);
    policies.insert("notify".to_string(), CompensationPolicy::NotNeeded);

    let compensatable = filter_compensatable(&steps, &policies);
    let result = compute_compensation_order(compensatable).expect("should not error");
    match result {
        CompensationOrderResult::Ordered { execution_order } => {
            assert!(execution_order.contains(&"charge".to_string()), "charge must compensate");
            assert!(execution_order.contains(&"reserve".to_string()), "reserve must compensate");
            assert!(!execution_order.contains(&"notify".to_string()), "notify should be skipped");
        }
        other => panic!("expected Ordered, got {:?}", other),
    }
}
