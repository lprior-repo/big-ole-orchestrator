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
