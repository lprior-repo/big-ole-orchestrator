//! Scenarios 2, 4, 9, 10: Diamond DAG validity, disconnected subgraphs, parallel fan-in,
//! and terminal-to-active edge rejection.

use super::*;

// ============================================================================
// Scenario 2: Diamond DAG is valid
// ============================================================================

#[test]
fn given_diamond_edges_a_b_a_c_b_d_c_d_when_dag_validated_then_no_cycle(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given a workflow with edges A→B, A→C, B→D, C→D (diamond pattern)
    let json = serde_json::json!({
        "workflow_name": "diamond",
        "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("A", "C", "Always"),
            edge_json("B", "D", "Always"),
            edge_json("C", "D", "Always"),
        ]
    });

    // When the DAG is validated
    let result = parse_workflow(json);

    // Then no cycle is detected; the DAG is valid
    let def = result.expect("diamond DAG should be valid");
    assert_eq!(def.nodes.len(), 4);
    assert_eq!(def.edges.len(), 4);
    Ok(())
}

#[test]
fn given_diamond_dag_when_execution_layers_computed_then_correct_parallel_grouping() {
    // Given a diamond DAG
    let def = make_workflow(
        "diamond",
        vec![
            ("A", 1, 0, 1.0),
            ("B", 1, 0, 1.0),
            ("C", 1, 0, 1.0),
            ("D", 1, 0, 1.0),
        ],
        vec![
            ("A", "B", EdgeCondition::Always),
            ("A", "C", EdgeCondition::Always),
            ("B", "D", EdgeCondition::Always),
            ("C", "D", EdgeCondition::Always),
        ],
    );

    // When execution layers are computed
    let layers = DependencyGraphResolver::execution_layers(&def);

    // Then B and C are in the same layer (parallel), D is in the next layer
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].len(), 1); // A
    assert_eq!(layers[1].len(), 2); // B, C (parallel)
    assert_eq!(layers[2].len(), 1); // D
}

// ============================================================================
// Scenario 4: Disconnected subgraph produces warning (parses OK, no error)
// ============================================================================

#[test]
fn given_disconnected_node_c_when_dag_validated_then_dag_still_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "disconnected",
        "nodes": [node_json("A"), node_json("B"), node_json("C")],
        "edges": [edge_json("A", "B", "Always")]
    });
    let result = parse_workflow(json);
    let def = result.expect("disconnected DAG should parse OK");
    assert_eq!(def.nodes.len(), 3);
    assert_eq!(def.edges.len(), 1);
    Ok(())
}

#[test]
fn given_disconnected_component_with_cycle_when_dag_validated_then_cycle_still_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "disconnected-with-cycle",
        "nodes": [node_json("isolated"), node_json("X"), node_json("Y")],
        "edges": [
            edge_json("X", "Y", "Always"),
            edge_json("Y", "X", "Always"),
        ]
    });
    let result = parse_workflow(json);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
    Ok(())
}

#[test]
fn given_two_separate_acyclic_components_when_dag_validated_then_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "two-components",
        "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("C", "D", "Always"),
        ]
    });
    let result = parse_workflow(json);
    let def = result.expect("two disconnected acyclic components should parse OK");
    assert_eq!(def.nodes.len(), 4);
    assert_eq!(def.edges.len(), 2);
    Ok(())
}

// ============================================================================
// Scenario 9: Parallel fan-in is valid
// ============================================================================

#[test]
fn given_parallel_branches_merge_at_d_when_dag_validated_then_fan_in_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "fan-in",
        "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("A", "C", "Always"),
            edge_json("B", "D", "Always"),
            edge_json("C", "D", "Always"),
        ]
    });
    let result = parse_workflow(json);
    let def = result.expect("fan-in DAG should be valid");
    assert_eq!(def.nodes.len(), 4);
    assert_eq!(def.edges.len(), 4);
    Ok(())
}

#[test]
fn given_wide_fan_in_10_to_1_when_dag_validated_then_valid(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut nodes = vec![node_json("entry")];
    for i in 0..10 {
        nodes.push(node_json(&format!("worker{}", i)));
    }
    nodes.push(node_json("merge"));
    let mut edges: Vec<serde_json::Value> = Vec::new();
    for i in 0..10 {
        edges.push(edge_json("entry", &format!("worker{}", i), "Always"));
        edges.push(edge_json(&format!("worker{}", i), "merge", "Always"));
    }
    let json = serde_json::json!({
        "workflow_name": "wide-fan-in",
        "nodes": nodes,
        "edges": edges,
    });
    let result = parse_workflow(json);
    let def = result.expect("wide fan-in should be valid");
    assert_eq!(def.nodes.len(), 12);
    assert_eq!(def.edges.len(), 20);
    Ok(())
}

#[test]
fn given_fan_in_dag_when_execution_layers_computed_then_parallel_layer_correct() {
    let def = make_workflow(
        "fan-in",
        vec![
            ("A", 1, 0, 1.0),
            ("B", 1, 0, 1.0),
            ("C", 1, 0, 1.0),
            ("D", 1, 0, 1.0),
        ],
        vec![
            ("A", "B", EdgeCondition::Always),
            ("A", "C", EdgeCondition::Always),
            ("B", "D", EdgeCondition::Always),
            ("C", "D", EdgeCondition::Always),
        ],
    );
    let layers = DependencyGraphResolver::execution_layers(&def);
    assert_eq!(layers.len(), 3);
    let parallel_layer = &layers[1];
    assert_eq!(parallel_layer.len(), 2);
    assert!(parallel_layer.contains(&NodeName("B".into())));
    assert!(parallel_layer.contains(&NodeName("C".into())));
}

// ============================================================================
// Scenario 10: Edge from terminal to active node — cycle detection
// ============================================================================

#[test]
fn given_edge_from_leaf_back_to_root_when_dag_validated_then_cycle_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "terminal-to-active",
        "nodes": [node_json("A"), node_json("B"), node_json("D")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("B", "D", "Always"),
            edge_json("D", "A", "Always"),
        ]
    });
    let result = parse_workflow(json);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
    if let Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) = result {
        assert!(cycle_nodes.len() >= 3);
        assert_eq!(cycle_nodes.first(), Some(&NodeName("A".into())));
        assert_eq!(cycle_nodes.last(), Some(&NodeName("A".into())));
    }
    Ok(())
}

#[test]
fn given_leaf_pointing_to_mid_node_when_dag_validated_then_cycle_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "leaf-to-mid",
        "nodes": [node_json("A"), node_json("B"), node_json("C")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("B", "C", "Always"),
            edge_json("C", "B", "Always"),
        ]
    });
    let result = parse_workflow(json);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
    if let Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) = result {
        assert_eq!(
            cycle_nodes,
            vec![
                NodeName("B".into()),
                NodeName("C".into()),
                NodeName("B".into()),
            ]
        );
    }
    Ok(())
}

#[test]
fn given_complex_dag_with_back_edge_when_dag_validated_then_cycle_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "back-edge-complex",
        "nodes": [node_json("A"), node_json("B"), node_json("C"), node_json("D")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("A", "C", "Always"),
            edge_json("B", "D", "Always"),
            edge_json("C", "D", "Always"),
            edge_json("D", "A", "Always"),
        ]
    });
    let result = parse_workflow(json);
    assert!(matches!(
        result,
        Err(WorkflowDefinitionError::CycleDetected { .. })
    ));
    Ok(())
}
