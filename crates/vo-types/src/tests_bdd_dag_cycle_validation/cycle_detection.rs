//! Scenarios 1-3: Cycle detection with full path reporting, 2-node cycles, and self-loops.

use super::*;

// ============================================================================
// Scenario 1: Simple cycle detected with full path A→B→C→A
// ============================================================================

#[test]
fn given_workflow_with_edges_a_b_c_a_when_dag_validated_then_cycle_detected_with_full_path(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given a workflow with edges A→B→C→A forming a cycle
    let json = serde_json::json!({
        "workflow_name": "cycle-abc",
        "nodes": [node_json("A"), node_json("B"), node_json("C")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("B", "C", "Always"),
            edge_json("C", "A", "Always"),
        ]
    });

    // When the workflow DAG is validated
    let result = parse_workflow(json);

    // Then a cycle is detected and the full cycle path A→B→C→A is reported
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::CycleDetected {
            cycle_nodes: vec![
                NodeName("A".into()),
                NodeName("B".into()),
                NodeName("C".into()),
                NodeName("A".into()),
            ],
        })
    );
    Ok(())
}

#[test]
fn given_2_node_cycle_a_b_a_when_dag_validated_then_cycle_path_a_b_a_reported(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given a workflow with edges A→B→A
    let json = serde_json::json!({
        "workflow_name": "cycle-2",
        "nodes": [node_json("A"), node_json("B")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("B", "A", "Always"),
        ]
    });

    // When validated
    let result = parse_workflow(json);

    // Then cycle path A→B→A is reported
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::CycleDetected {
            cycle_nodes: vec![
                NodeName("A".into()),
                NodeName("B".into()),
                NodeName("A".into()),
            ],
        })
    );
    Ok(())
}

// ============================================================================
// Scenario 3: Self-loop detected
// ============================================================================

#[test]
fn given_self_loop_edge_a_to_a_when_dag_validated_then_cycle_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    // Given a workflow with a self-loop edge A→A
    let json = serde_json::json!({
        "workflow_name": "self-loop",
        "nodes": [node_json("A")],
        "edges": [edge_json("A", "A", "Always")]
    });

    // When the DAG is validated
    let result = parse_workflow(json);

    // Then a cycle is detected (self-loop) and reported
    assert_eq!(
        result,
        Err(WorkflowDefinitionError::CycleDetected {
            cycle_nodes: vec![NodeName("A".into()), NodeName("A".into())],
        })
    );
    Ok(())
}

#[test]
fn given_self_loop_in_multi_node_graph_when_dag_validated_then_cycle_detected(
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::json!({
        "workflow_name": "self-loop-multi",
        "nodes": [node_json("A"), node_json("B"), node_json("C")],
        "edges": [
            edge_json("A", "B", "Always"),
            edge_json("B", "C", "Always"),
            edge_json("B", "B", "Always"),
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
            vec![NodeName("B".into()), NodeName("B".into())]
        );
    }
    Ok(())
}
