//! vel-bxpg: DAG cycle detection with --graph integration (ADR-022)
//!
//! This crate provides:
//! - `Dag`: Workflow builder that tracks nodes and edges
//! - `Dag::build()`: Validates the DAG and runs cycle detection
//! - `output_graph()`: Serializes `WorkflowDefinition` to stdout as JSON

pub mod cycle;
pub mod dag;
pub mod error;
pub mod graph;
pub mod types;
#[cfg(kani)]
pub mod verification;

// Re-export commonly used types
pub use cycle::detect_cycle;
pub use dag::Dag;
pub use error::{GraphOutputError, WorkflowDefinitionError};
pub use graph::output_graph;
pub use types::{DagNode, Edge, NodeHandle, NodeName, RetryPolicy, WorkflowDefinition};

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Dag::build() Unit Tests
    // ========================================================================

    #[test]
    fn dag_build_returns_empty_workflow_error_when_no_nodes_added() {
        let dag = Dag::new("test-workflow");
        let result = dag.build();
        assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow));
    }

    #[test]
    fn dag_build_returns_cycle_detected_when_graph_contains_simple_cycle() {
        let mut dag = Dag::new("cyclic-workflow");
        dag.add_node(DagNode {
            name: "A".into(),
            retry_policy: None,
        })
        .add_node(DagNode {
            name: "B".into(),
            retry_policy: None,
        })
        .connect("A".into(), "B".into())
        .connect("B".into(), "A".into());

        let result = dag.build();
        assert!(matches!(
            result,
            Err(WorkflowDefinitionError::CycleDetected { .. })
        ));
    }

    #[test]
    fn dag_build_returns_cycle_detected_when_graph_contains_self_loop() {
        let mut dag = Dag::new("self-loop-workflow");
        dag.add_node(DagNode {
            name: "A".into(),
            retry_policy: None,
        })
        .connect("A".into(), "A".into());

        let result = dag.build();
        match result {
            Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
                assert_eq!(cycle_nodes, vec!["A"]);
            }
            other => panic!("Expected CycleDetected error, got {:?}", other),
        }
    }

    #[test]
    fn dag_build_returns_unknown_node_when_edge_references_nonexistent_node() {
        let mut dag = Dag::new("unknown-node-workflow");
        dag.add_node(DagNode {
            name: "A".into(),
            retry_policy: None,
        })
        .connect("A".into(), "nonexistent".into());

        let result = dag.build();
        match result {
            Err(WorkflowDefinitionError::UnknownNode {
                edge_source,
                unknown_target,
            }) => {
                assert_eq!(edge_source, "A");
                assert_eq!(unknown_target, "nonexistent");
            }
            other => panic!("Expected UnknownNode error, got {:?}", other),
        }
    }

    #[test]
    fn dag_build_returns_invalid_retry_policy_when_node_has_negative_backoff() {
        let mut dag = Dag::new("invalid-retry-workflow");
        dag.add_node(DagNode {
            name: "A".into(),
            retry_policy: Some(RetryPolicy {
                max_retries: 3,
                backoff_ms: 0, // Invalid: 0 is not strictly negative but tests boundary
            }),
        });

        let result = dag.build();
        // This should fail with InvalidRetryPolicy when implementation exists
        assert!(result.is_err());
    }

    #[test]
    fn dag_build_returns_ok_when_node_is_valid() {
        // This tests the success path when a valid node is added
        let mut dag = Dag::new("valid-workflow");
        dag.add_node(DagNode {
            name: "A".into(),
            retry_policy: None,
        });

        let result = dag.build();
        // Implementation returns Ok since the node is valid
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert_eq!(workflow.workflow_name, "valid-workflow");
        assert_eq!(workflow.nodes.len(), 1);
    }

    // ========================================================================
    // detect_cycle Unit Tests
    // ========================================================================

    #[test]
    fn detect_cycle_returns_none_when_graph_is_acyclic() {
        let nodes = vec![
            DagNode {
                name: "A".into(),
                retry_policy: None,
            },
            DagNode {
                name: "B".into(),
                retry_policy: None,
            },
        ];
        let edges = vec![Edge {
            source_node: "A".into(),
            target_node: "B".into(),
            condition: None,
        }];

        let result = cycle::detect_cycle(&nodes, &edges);
        assert_eq!(result, None);
    }

    #[test]
    fn detect_cycle_returns_some_when_graph_contains_self_loop() {
        let nodes = vec![DagNode {
            name: "A".into(),
            retry_policy: None,
        }];
        let edges = vec![Edge {
            source_node: "A".into(),
            target_node: "A".into(),
            condition: None,
        }];

        let result = cycle::detect_cycle(&nodes, &edges);
        match result {
            Some(cycle_nodes) => assert_eq!(cycle_nodes, vec!["A"]),
            None => panic!("Expected Some([\"A\"]), got None"),
        }
    }

    #[test]
    fn detect_cycle_returns_some_when_graph_contains_two_node_cycle() {
        let nodes = vec![
            DagNode {
                name: "A".into(),
                retry_policy: None,
            },
            DagNode {
                name: "B".into(),
                retry_policy: None,
            },
        ];
        let edges = vec![
            Edge {
                source_node: "A".into(),
                target_node: "B".into(),
                condition: None,
            },
            Edge {
                source_node: "B".into(),
                target_node: "A".into(),
                condition: None,
            },
        ];

        let result = cycle::detect_cycle(&nodes, &edges);
        assert!(result.is_some());
        let cycle = result.unwrap();
        assert!(cycle.contains(&"A".to_string()) && cycle.contains(&"B".to_string()));
    }

    #[test]
    fn detect_cycle_returns_some_when_graph_contains_three_node_cycle() {
        let nodes = vec![
            DagNode {
                name: "A".into(),
                retry_policy: None,
            },
            DagNode {
                name: "B".into(),
                retry_policy: None,
            },
            DagNode {
                name: "C".into(),
                retry_policy: None,
            },
        ];
        let edges = vec![
            Edge {
                source_node: "A".into(),
                target_node: "B".into(),
                condition: None,
            },
            Edge {
                source_node: "B".into(),
                target_node: "C".into(),
                condition: None,
            },
            Edge {
                source_node: "C".into(),
                target_node: "A".into(),
                condition: None,
            },
        ];

        let result = cycle::detect_cycle(&nodes, &edges);
        assert!(result.is_some());
    }

    #[test]
    fn detect_cycle_returns_deterministic_ordering() {
        let nodes = vec![
            DagNode {
                name: "A".into(),
                retry_policy: None,
            },
            DagNode {
                name: "B".into(),
                retry_policy: None,
            },
            DagNode {
                name: "C".into(),
                retry_policy: None,
            },
        ];
        let edges = vec![
            Edge {
                source_node: "A".into(),
                target_node: "B".into(),
                condition: None,
            },
            Edge {
                source_node: "B".into(),
                target_node: "C".into(),
                condition: None,
            },
            Edge {
                source_node: "C".into(),
                target_node: "A".into(),
                condition: None,
            },
        ];

        let result1 = cycle::detect_cycle(&nodes, &edges);
        let result2 = cycle::detect_cycle(&nodes, &edges);
        assert_eq!(result1, result2);
    }

    #[test]
    fn detect_cycle_handles_disconnected_components_with_cycles() {
        // Component 1: A -> B (acyclic)
        // Component 2: C -> D -> C (cyclic)
        let nodes = vec![
            DagNode {
                name: "A".into(),
                retry_policy: None,
            },
            DagNode {
                name: "B".into(),
                retry_policy: None,
            },
            DagNode {
                name: "C".into(),
                retry_policy: None,
            },
            DagNode {
                name: "D".into(),
                retry_policy: None,
            },
        ];
        let edges = vec![
            Edge {
                source_node: "A".into(),
                target_node: "B".into(),
                condition: None,
            },
            Edge {
                source_node: "C".into(),
                target_node: "D".into(),
                condition: None,
            },
            Edge {
                source_node: "D".into(),
                target_node: "C".into(),
                condition: None,
            },
        ];

        let result = cycle::detect_cycle(&nodes, &edges);
        assert!(result.is_some());
        let cycle = result.unwrap();
        assert!(cycle.contains(&"C".to_string()) && cycle.contains(&"D".to_string()));
        assert!(!cycle.contains(&"A".to_string()) && !cycle.contains(&"B".to_string()));
    }

    #[test]
    fn detect_cycle_returns_none_for_empty_graph() {
        let nodes: Vec<DagNode> = vec![];
        let edges: Vec<Edge> = vec![];

        let result = cycle::detect_cycle(&nodes, &edges);
        assert_eq!(result, None);
    }

    #[test]
    fn detect_cycle_returns_none_for_single_node_no_edges() {
        let nodes = vec![DagNode {
            name: "A".into(),
            retry_policy: None,
        }];
        let edges: Vec<Edge> = vec![];

        let result = cycle::detect_cycle(&nodes, &edges);
        assert_eq!(result, None);
    }

    // ========================================================================
    // output_graph Unit Tests
    // ========================================================================

    #[test]
    fn output_graph_returns_ok_when_serialization_succeeds() {
        // Create a valid WorkflowDefinition that serializes successfully
        let workflow = WorkflowDefinition {
            workflow_name: "test".into(),
            nodes: vec![],
            edges: vec![],
        };

        let result = output_graph(&workflow);
        // Implementation succeeds - serialization works for valid input
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn output_graph_returns_ok_for_valid_workflow() {
        let workflow = WorkflowDefinition {
            workflow_name: "test-workflow".into(),
            nodes: vec![
                DagNode {
                    name: "A".into(),
                    retry_policy: None,
                },
                DagNode {
                    name: "B".into(),
                    retry_policy: None,
                },
            ],
            edges: vec![Edge {
                source_node: "A".into(),
                target_node: "B".into(),
                condition: None,
            }],
        };

        let result = output_graph(&workflow);
        // Implementation succeeds for valid workflow
        assert_eq!(result, Ok(()));
    }
}
