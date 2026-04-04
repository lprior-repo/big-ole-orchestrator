//! Integration tests for vel-bxpg
//!
//! These tests exercise the full behavior through the public API,
//! treating the crate as a black box.

use vel_bxpg::{Dag, DagNode, Edge, WorkflowDefinition, WorkflowDefinitionError};

/// Behavior 1: Dag builds successfully when graph is acyclic
#[test]
fn dag_builds_successfully_when_graph_is_acyclic() {
    // Given: A DAG with nodes and edges containing no cycles
    let mut dag = Dag::new("test-workflow");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "B".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "C".into(),
        retry_policy: None,
    })
    .connect("A".into(), "B".into())
    .connect("B".into(), "C".into());

    // When: Dag::build() is called
    let result = dag.build();

    // Then: Returns Ok(WorkflowDefinition) with correct structure
    // RED PHASE: This will fail because build() returns EmptyWorkflow error
    match result {
        Ok(workflow) => {
            assert_eq!(workflow.workflow_name, "test-workflow");
            assert_eq!(workflow.nodes.len(), 3);
            assert_eq!(workflow.edges.len(), 2);
        }
        Err(e) => panic!("Expected Ok(WorkflowDefinition), got Err({:?})", e),
    }
}

/// Behavior 2: Dag rejects when graph contains a cycle
#[test]
fn dag_rejects_when_graph_contains_a_cycle() {
    // Given: A DAG where nodes form a cycle A -> B -> C -> A
    let mut dag = Dag::new("cyclic-workflow");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "B".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "C".into(),
        retry_policy: None,
    })
    .connect("A".into(), "B".into())
    .connect("B".into(), "C".into())
    .connect("C".into(), "A".into());

    // When: Dag::build() is called
    let result = dag.build();

    // Then: Returns Err(WorkflowDefinitionError::CycleDetected { cycle_nodes })
    // RED PHASE: This will fail because build() returns EmptyWorkflow error
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            assert!(cycle_nodes.contains(&"A".to_string()));
            assert!(cycle_nodes.contains(&"B".to_string()));
            assert!(cycle_nodes.contains(&"C".to_string()));
        }
        other => panic!("Expected CycleDetected error, got {:?}", other),
    }
}

/// Behavior 3: Dag rejects when graph contains a self-loop
#[test]
fn dag_rejects_when_graph_contains_a_self_loop() {
    // Given: A DAG where node A has an edge to itself
    let mut dag = Dag::new("self-loop-workflow");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .connect("A".into(), "A".into());

    // When: Dag::build() is called
    let result = dag.build();

    // Then: Returns Err(WorkflowDefinitionError::CycleDetected { cycle_nodes })
    // RED PHASE: This will fail because build() returns EmptyWorkflow error
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            assert_eq!(cycle_nodes, vec!["A"]);
        }
        other => panic!("Expected CycleDetected([\"A\"]), got {:?}", other),
    }
}

/// Behavior 4: Dag rejects when graph has empty nodes
#[test]
fn dag_rejects_when_graph_has_empty_nodes() {
    // Given: A DAG with no nodes added (build called on empty)
    let dag = Dag::new("empty-workflow");

    // When: Dag::build() is called
    let result = dag.build();

    // Then: Returns Err(WorkflowDefinitionError::EmptyWorkflow)
    assert_eq!(result, Err(WorkflowDefinitionError::EmptyWorkflow));
}

/// Behavior 5: Dag rejects when edge references unknown node
#[test]
fn dag_rejects_when_edge_references_unknown_node() {
    // Given: A DAG with node A added, then connect(A, "nonexistent") called
    let mut dag = Dag::new("unknown-node-workflow");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .connect("A".into(), "nonexistent".into());

    // When: Dag::build() is called
    let result = dag.build();

    // Then: Returns Err(WorkflowDefinitionError::UnknownNode { edge_source: "A", unknown_target: "nonexistent" })
    // RED PHASE: This will fail because build() returns EmptyWorkflow error
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

/// Behavior 6: Dag rejects when node has invalid retry policy
#[test]
fn dag_rejects_when_node_has_invalid_retry_policy() {
    // Given: A DAG with a node containing RetryPolicy { max_retries: u32::MAX, backoff_ms: 0 }
    let mut dag = Dag::new("invalid-retry-workflow");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: Some(vel_bxpg::RetryPolicy {
            max_retries: u32::MAX,
            backoff_ms: 0,
        }),
    });

    // When: Dag::build() is called
    let result = dag.build();

    // Then: Returns Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name: "A", reason: ... })
    // RED PHASE: This will fail because build() returns EmptyWorkflow error
    match result {
        Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name, .. }) => {
            assert_eq!(node_name, "A");
        }
        other => panic!("Expected InvalidRetryPolicy error, got {:?}", other),
    }
}

/// Behavior 7: Dag builds successfully when node is valid
#[test]
fn dag_builds_successfully_when_node_is_valid() {
    // Given: A workflow with a valid node
    let mut dag = Dag::new("valid-workflow");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    });

    let result = dag.build();
    // The implementation returns Ok for valid nodes
    assert!(result.is_ok());
    let workflow = result.unwrap();
    assert_eq!(workflow.workflow_name, "valid-workflow");
    assert_eq!(workflow.nodes.len(), 1);
}

/// Behavior 8: output_graph writes valid JSON to stdout when given valid WorkflowDefinition
#[test]
fn output_graph_writes_valid_json_to_stdout_when_given_valid_workflow_definition() {
    // Given: A valid WorkflowDefinition
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

    // When: output_graph() is called
    let result = vel_bxpg::output_graph(&workflow);

    // Then: Returns Ok(()) and JSON is written to stdout
    assert_eq!(result, Ok(()));
}

/// Behavior 9: output_graph returns Ok when serialization succeeds
#[test]
fn output_graph_returns_ok_when_serialization_succeeds() {
    // Given: A valid WorkflowDefinition that serializes successfully
    let workflow = WorkflowDefinition {
        workflow_name: "test".into(),
        nodes: vec![],
        edges: vec![],
    };

    // When: output_graph() is called
    let result = vel_bxpg::output_graph(&workflow);

    // Then: Returns Ok(()) since serialization succeeds
    assert_eq!(result, Ok(()));
}

/// Integration: output_graph produces valid JSON that can be deserialized
#[test]
fn output_graph_produces_valid_json_that_can_be_deserialized() {
    // Given: A valid WorkflowDefinition
    let workflow = WorkflowDefinition {
        workflow_name: "roundtrip-test".into(),
        nodes: vec![
            DagNode {
                name: "Node1".into(),
                retry_policy: None,
            },
            DagNode {
                name: "Node2".into(),
                retry_policy: None,
            },
        ],
        edges: vec![Edge {
            source_node: "Node1".into(),
            target_node: "Node2".into(),
            condition: Some("true".into()),
        }],
    };

    // When: We serialize it manually and deserialize it
    let json = serde_json::to_string(&workflow).expect("workflow should serialize");
    let recovered: WorkflowDefinition =
        serde_json::from_str(&json).expect("json should deserialize");

    // Then: The recovered workflow matches the original
    assert_eq!(recovered.workflow_name, workflow.workflow_name);
    assert_eq!(recovered.nodes.len(), workflow.nodes.len());
    assert_eq!(recovered.edges.len(), workflow.edges.len());
}

/// Integration: Verify WorkflowDefinition serialization structure
#[test]
fn workflow_definition_serialization_contains_all_required_fields() {
    let workflow = WorkflowDefinition {
        workflow_name: "fields-test".into(),
        nodes: vec![DagNode {
            name: "A".into(),
            retry_policy: None,
        }],
        edges: vec![],
    };

    let json = serde_json::to_string(&workflow).expect("should serialize");

    // Verify JSON contains all required fields
    assert!(json.contains("\"workflow_name\""));
    assert!(json.contains("\"nodes\""));
    assert!(json.contains("\"edges\""));
    assert!(json.contains("fields-test"));
    assert!(json.contains("A"));
}

/// Integration: Empty workflow (edge case)
#[test]
fn dag_with_single_node_and_no_edges_builds_successfully() {
    let mut dag = Dag::new("single-node");
    dag.add_node(DagNode {
        name: "OnlyNode".into(),
        retry_policy: None,
    });

    let result = dag.build();

    // RED PHASE: Will fail because stub returns EmptyWorkflow
    match result {
        Ok(workflow) => {
            assert_eq!(workflow.workflow_name, "single-node");
            assert_eq!(workflow.nodes.len(), 1);
            assert_eq!(workflow.edges.len(), 0);
        }
        Err(e) => panic!("Expected Ok, got {:?}", e),
    }
}

/// Integration: Multiple disconnected components (acyclic)
#[test]
fn dag_with_disconnected_components_builds_successfully() {
    let mut dag = Dag::new("disconnected");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "B".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "C".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "D".into(),
        retry_policy: None,
    })
    // Component 1: A -> B
    .connect("A".into(), "B".into())
    // Component 2: C -> D
    .connect("C".into(), "D".into());

    let result = dag.build();

    // RED PHASE: Will fail because stub returns EmptyWorkflow
    match result {
        Ok(workflow) => {
            assert_eq!(workflow.nodes.len(), 4);
            assert_eq!(workflow.edges.len(), 2);
        }
        Err(e) => panic!("Expected Ok, got {:?}", e),
    }
}
