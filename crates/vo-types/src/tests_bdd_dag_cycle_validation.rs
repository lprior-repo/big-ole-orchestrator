//! BDD tests for DAG Cycle Validation & Edge Integrity.
//!
//! bead_id: ve-ttqfg
//!
//! Given/When/Then scenarios covering:
//! 1. Cycle detection with full path reporting
//! 2. Diamond DAG validity
//! 3. Self-loop detection
//! 4. Disconnected subgraph handling
//! 5. Mutually exclusive conditional edges
//! 6. Exhaustive conditional coverage
//! 7. Empty condition defaults to unconditional
//! 8. Large DAG performance (<100ms for 100 nodes)
//! 9. Parallel fan-in validity
//! 10. Terminal-to-active edge rejection

use crate::{
    DagNode, DependencyGraphResolver, Edge, EdgeCondition, NodeName, NonEmptyVec, RetryPolicy,
    StepOutcome, WorkflowDefinition, WorkflowDefinitionError, WorkflowName,
};

/// Helper: construct a valid WorkflowDefinition directly (bypasses parse).
fn make_workflow(
    name: &str,
    nodes: Vec<(&str, u8, u64, f64)>,
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
                        max_backoff_ms: u64::MAX,
                    },
                    compensation_policy: None,
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

/// Helper: parse a JSON workflow definition.
fn parse_workflow(json: serde_json::Value) -> Result<WorkflowDefinition, WorkflowDefinitionError> {
    let bytes = serde_json::to_vec(&json).expect("serialize");
    WorkflowDefinition::parse(&bytes)
}

/// Helper: make a single-node JSON fragment.
fn node_json(name: &str) -> serde_json::Value {
    serde_json::json!({
        "node_name": name,
        "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
    })
}

/// Helper: make an edge JSON fragment.
fn edge_json(from: &str, to: &str, condition: &str) -> serde_json::Value {
    serde_json::json!({
        "source_node": from,
        "target_node": to,
        "condition": condition
    })
}

// ============================================================================
// Scenario 1: Simple cycle detected with full path A→B→C→A
// ============================================================================

mod scenario_1_simple_cycle_with_path {
    use super::*;

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
}

// ============================================================================
// Scenario 2: Diamond DAG is valid
// ============================================================================

mod scenario_2_diamond_dag_valid {
    use super::*;

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
}

// ============================================================================
// Scenario 3: Self-loop detected
// ============================================================================

mod scenario_3_self_loop {
    use super::*;

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
}

// ============================================================================
// Scenario 4: Disconnected subgraph (orphan) produces error
// ============================================================================

mod scenario_4_disconnected_subgraph {
    use super::*;

    #[test]
    fn given_disconnected_node_c_when_dag_validated_then_orphan_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "workflow_name": "disconnected",
            "nodes": [node_json("A"), node_json("B"), node_json("C")],
            "edges": [edge_json("A", "B", "Always")]
        });
        let result = parse_workflow(json);
        assert!(matches!(result, Err(WorkflowDefinitionError::OrphanNodes { .. })));
        if let Err(WorkflowDefinitionError::OrphanNodes { orphan_nodes }) = result {
            assert!(orphan_nodes.contains(&NodeName("C".into())));
        }
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
}

// ============================================================================
// Scenario 5: Mutually exclusive conditional edges valid
// ============================================================================

mod scenario_5_mutually_exclusive_conditions {
    use super::*;

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
}

// ============================================================================
// Scenario 6: Exhaustive conditional coverage reaches all terminals
// ============================================================================

mod scenario_6_exhaustive_conditional_coverage {
    use super::*;

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
}

// ============================================================================
// Scenario 7: Empty condition defaults to unconditional (Always)
// ============================================================================

mod scenario_7_empty_condition_default {
    use super::*;

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
}

// ============================================================================
// Scenario 8: Large DAG performance (<100ms for 100 nodes)
// ============================================================================

mod scenario_8_large_dag_performance {
    use super::*;
    use std::time::Instant;

    fn build_large_dag_json(node_count: usize) -> serde_json::Value {
        let nodes: Vec<serde_json::Value> = (0..node_count)
            .map(|i| node_json(&format!("node{}", i)))
            .collect();
        let edges: Vec<serde_json::Value> = (0..node_count.saturating_sub(1))
            .map(|i| edge_json(&format!("node{}", i), &format!("node{}", i + 1), "Always"))
            .collect();
        serde_json::json!({
            "workflow_name": "large-chain",
            "nodes": nodes,
            "edges": edges,
        })
    }

    #[test]
    fn given_100_node_chain_when_dag_validated_then_completes_under_100ms(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = build_large_dag_json(100);
        let start = Instant::now();
        let result = parse_workflow(json);
        let elapsed = start.elapsed();
        let def = result.expect("100-node chain should be valid");
        assert_eq!(def.nodes.len(), 100);
        assert!(
            elapsed.as_millis() < 100,
            "Validation took {}ms, expected < 100ms",
            elapsed.as_millis()
        );
        Ok(())
    }

    #[test]
    fn given_100_node_chain_when_execution_layers_computed_then_completes_under_100ms() {
        let nodes: Vec<DagNode> = (0..100)
            .map(|i| DagNode {
                node_name: NodeName(format!("node{}", i).into()),
                retry_policy: RetryPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                    backoff_multiplier: 1.0,
                    max_backoff_ms: u64::MAX,
                },
                compensation_policy: None,
            })
            .collect();
        let edges: Vec<Edge> = (0..99)
            .map(|i| Edge {
                source_node: NodeName(format!("node{}", i).into()),
                target_node: NodeName(format!("node{}", i + 1).into()),
                condition: EdgeCondition::Always,
            })
            .collect();
        let def = WorkflowDefinition {
            workflow_name: WorkflowName("large-chain".into()),
            nodes: NonEmptyVec::new_unchecked(nodes),
            edges,
        };
        let start = Instant::now();
        let layers = DependencyGraphResolver::execution_layers(&def);
        let elapsed = start.elapsed();
        assert_eq!(layers.len(), 100);
        for layer in &layers {
            assert_eq!(layer.len(), 1, "chain should have 1 node per layer");
        }
        assert!(
            elapsed.as_millis() < 100,
            "Execution layers took {}ms, expected < 100ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn given_100_node_diamond_structure_when_dag_validated_then_completes_under_100ms(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let nodes: Vec<serde_json::Value> =
            (0..100).map(|i| node_json(&format!("node{}", i))).collect();
        let mut edges: Vec<serde_json::Value> = Vec::new();
        for i in 1..50 {
            edges.push(edge_json("node0", &format!("node{}", i), "Always"));
        }
        for i in 1..50 {
            edges.push(edge_json(&format!("node{}", i), "node50", "Always"));
        }
        for i in 50..99 {
            edges.push(edge_json(
                &format!("node{}", i),
                &format!("node{}", i + 1),
                "Always",
            ));
        }
        let json = serde_json::json!({
            "workflow_name": "large-diamond",
            "nodes": nodes,
            "edges": edges,
        });
        let start = Instant::now();
        let result = parse_workflow(json);
        let elapsed = start.elapsed();
        let def = result.expect("100-node diamond should be valid");
        assert_eq!(def.nodes.len(), 100);
        assert!(
            elapsed.as_millis() < 100,
            "Diamond validation took {}ms, expected < 100ms",
            elapsed.as_millis()
        );
        Ok(())
    }
}

// ============================================================================
// Scenario 9: Parallel fan-in is valid
// ============================================================================

mod scenario_9_parallel_fan_in {
    use super::*;

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
}

// ============================================================================
// Scenario 10: Edge from terminal to active node — cycle detection
// ============================================================================

mod scenario_10_terminal_to_active_edge {
    use super::*;

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
}

// ============================================================================
// Edge integrity: unknown node references
// ============================================================================

mod edge_integrity {
    use super::*;

    #[test]
    fn given_edge_with_unknown_source_when_validated_then_unknown_node_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "workflow_name": "bad-source",
            "nodes": [node_json("A")],
            "edges": [edge_json("ghost", "A", "Always")]
        });
        let result = parse_workflow(json);
        assert_eq!(
            result,
            Err(WorkflowDefinitionError::UnknownNode {
                edge_source: NodeName("ghost".into()),
                unknown_target: NodeName("ghost".into()),
            })
        );
        Ok(())
    }

    #[test]
    fn given_edge_with_unknown_target_when_validated_then_unknown_node_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "workflow_name": "bad-target",
            "nodes": [node_json("A")],
            "edges": [edge_json("A", "ghost", "Always")]
        });
        let result = parse_workflow(json);
        assert_eq!(
            result,
            Err(WorkflowDefinitionError::UnknownNode {
                edge_source: NodeName("A".into()),
                unknown_target: NodeName("ghost".into()),
            })
        );
        Ok(())
    }

    #[test]
    fn given_edge_with_both_nodes_unknown_when_validated_then_unknown_node_error(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "workflow_name": "both-unknown",
            "nodes": [node_json("A")],
            "edges": [edge_json("phantom", "specter", "Always")]
        });
        let result = parse_workflow(json);
        assert!(matches!(
            result,
            Err(WorkflowDefinitionError::UnknownNode { .. })
        ));
        Ok(())
    }
}
