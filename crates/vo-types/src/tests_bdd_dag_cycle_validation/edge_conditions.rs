//! Scenarios 5-7: Mutually exclusive conditional edges, exhaustive coverage,
//! and empty condition defaulting to Always.

use super::*;

// ============================================================================
// Scenario 5: Mutually exclusive conditional edges valid
// ============================================================================

#[test]
fn given_onsuccess_and_onfailure_edges_when_dag_validated_then_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "exclusive-conditions",
        "nodes": [node_json("check"), node_json("handle_ok"), node_json("handle_err")],
        "edges": [
            edge_json("check", "handle_ok", "OnSuccess"),
            edge_json("check", "handle_err", "OnFailure"),
        ]
    });
    let result = parse_workflow(json);
    let def = result.expect("mutually exclusive conditions should be valid");
    assert_eq!(def.nodes.len(), 3);
    assert_eq!(def.edges.len(), 2);
    Ok(())
}

#[test]
fn given_onsuccess_and_onfailure_when_success_outcome_then_only_success_path_taken() {
    let def = make_workflow(
        "exclusive",
        vec![
            ("check", 1, 0, 1.0),
            ("handle_ok", 1, 0, 1.0),
            ("handle_err", 1, 0, 1.0),
        ],
        vec![
            ("check", "handle_ok", EdgeCondition::OnSuccess),
            ("check", "handle_err", EdgeCondition::OnFailure),
        ],
    );
    let next = crate::next_nodes(&NodeName("check".into()), StepOutcome::Success, &def);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].node_name, NodeName("handle_ok".into()));
}

#[test]
fn given_onsuccess_and_onfailure_when_failure_outcome_then_only_failure_path_taken() {
    let def = make_workflow(
        "exclusive",
        vec![
            ("check", 1, 0, 1.0),
            ("handle_ok", 1, 0, 1.0),
            ("handle_err", 1, 0, 1.0),
        ],
        vec![
            ("check", "handle_ok", EdgeCondition::OnSuccess),
            ("check", "handle_err", EdgeCondition::OnFailure),
        ],
    );
    let next = crate::next_nodes(&NodeName("check".into()), StepOutcome::Failure, &def);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].node_name, NodeName("handle_err".into()));
}

// ============================================================================
// Scenario 6: Exhaustive conditional coverage reaches all terminals
// ============================================================================

#[test]
fn given_onsuccess_onfailure_and_always_edges_when_dag_validated_then_all_reachable(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "exhaustive",
        "nodes": [
            node_json("check"),
            node_json("ok"),
            node_json("err"),
            node_json("audit"),
        ],
        "edges": [
            edge_json("check", "ok", "OnSuccess"),
            edge_json("check", "err", "OnFailure"),
            edge_json("check", "audit", "Always"),
        ]
    });
    let result = parse_workflow(json);
    let def = result.expect("exhaustive conditional coverage should be valid");
    assert_eq!(def.nodes.len(), 4);
    assert_eq!(def.edges.len(), 3);

    let on_success = crate::next_nodes(&NodeName("check".into()), StepOutcome::Success, &def);
    let on_failure = crate::next_nodes(&NodeName("check".into()), StepOutcome::Failure, &def);

    let success_names: std::collections::HashSet<&str> =
        on_success.iter().map(|n| n.node_name.as_str()).collect();
    let failure_names: std::collections::HashSet<&str> =
        on_failure.iter().map(|n| n.node_name.as_str()).collect();

    assert!(success_names.contains("ok"));
    assert!(success_names.contains("audit"));
    assert!(!success_names.contains("err"));

    assert!(failure_names.contains("err"));
    assert!(failure_names.contains("audit"));
    assert!(!failure_names.contains("ok"));
    Ok(())
}

// ============================================================================
// Scenario 7: Empty condition defaults to unconditional (Always)
// ============================================================================

#[test]
fn given_edge_with_always_condition_when_dag_validated_then_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "always-edge",
        "nodes": [node_json("A"), node_json("B")],
        "edges": [edge_json("A", "B", "Always")]
    });
    let result = parse_workflow(json);
    let def = result.expect("Always edge should be valid");
    assert_eq!(def.edges[0].condition, EdgeCondition::Always);
    Ok(())
}

#[test]
fn given_always_edge_when_success_outcome_then_edge_traversed() {
    let def = make_workflow(
        "test",
        vec![("A", 1, 0, 1.0), ("B", 1, 0, 1.0)],
        vec![("A", "B", EdgeCondition::Always)],
    );
    let next = crate::next_nodes(&NodeName("A".into()), StepOutcome::Success, &def);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].node_name, NodeName("B".into()));
}

#[test]
fn given_always_edge_when_failure_outcome_then_edge_still_traversed() {
    let def = make_workflow(
        "test",
        vec![("A", 1, 0, 1.0), ("B", 1, 0, 1.0)],
        vec![("A", "B", EdgeCondition::Always)],
    );
    let next = crate::next_nodes(&NodeName("A".into()), StepOutcome::Failure, &def);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].node_name, NodeName("B".into()));
}
