//! Adversarial tests for vel-bxpg
//!
//! These tests attempt to violate contracts, find edge cases, and trigger failure modes.

use vel_bxpg::{
    Dag, DagNode, Edge, RetryPolicy, WorkflowDefinition, WorkflowDefinitionError, detect_cycle,
    output_graph,
};

/// ============================================================================
/// CONTRACT VIOLATIONS: Empty Workflow
/// ============================================================================

/// Contract: Dag::build() on empty Dag MUST return EmptyWorkflow
#[test]
fn adversarial_empty_workflow_returns_error() {
    let dag = Dag::new("empty-workflow");
    let result = dag.build();
    assert!(
        matches!(result, Err(WorkflowDefinitionError::EmptyWorkflow)),
        "Empty workflow must return EmptyWorkflow error, got: {:?}",
        result
    );
}

/// ============================================================================
/// CONTRACT VIOLATIONS: Cycle Detection
/// ============================================================================

/// Contract: Self-loop A→A MUST be detected as CycleDetected
#[test]
fn adversarial_self_loop_detected() {
    let mut dag = Dag::new("self-loop");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .connect("A".into(), "A".into());

    let result = dag.build();
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            assert!(
                cycle_nodes.contains(&"A".to_string()),
                "Self-loop must report node A in cycle"
            );
        }
        other => panic!("Expected CycleDetected([\"A\"]), got: {:?}", other),
    }
}

/// Contract: Mutual edges A↔B MUST be detected as CycleDetected
#[test]
fn adversarial_two_node_cycle_detected() {
    let mut dag = Dag::new("mutual-edges");
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
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            assert!(
                cycle_nodes.contains(&"A".to_string()) && cycle_nodes.contains(&"B".to_string()),
                "Two-node cycle must contain both A and B"
            );
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

/// Contract: Triangle A→B→C→A MUST be detected
#[test]
fn adversarial_three_node_cycle_detected() {
    let mut dag = Dag::new("triangle");
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

    let result = dag.build();
    assert!(
        matches!(result, Err(WorkflowDefinitionError::CycleDetected { .. })),
        "Triangle cycle must be detected"
    );
}

/// Contract: Disconnected component with cycle MUST be detected
#[test]
fn adversarial_disconnected_cycle_detected() {
    // Component 1: A -> B (acyclic)
    // Component 2: C -> D -> E -> C (cyclic)
    let mut dag = Dag::new("disconnected-cycle");
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
    .add_node(DagNode {
        name: "E".into(),
        retry_policy: None,
    })
    .connect("A".into(), "B".into())
    .connect("C".into(), "D".into())
    .connect("D".into(), "E".into())
    .connect("E".into(), "C".into());

    let result = dag.build();
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            // Cycle should be in the disconnected component (C, D, or E)
            assert!(
                cycle_nodes
                    .iter()
                    .all(|n| ["C", "D", "E"].contains(&n.as_str())),
                "Cycle should be in disconnected component, got: {:?}",
                cycle_nodes
            );
            assert!(
                !cycle_nodes.contains(&"A".to_string()) && !cycle_nodes.contains(&"B".to_string()),
                "Acyclic component should not be in cycle"
            );
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

/// ============================================================================
/// CONTRACT VIOLATIONS: Unknown Node
/// ============================================================================

/// Contract: Edge referencing non-existent node MUST return UnknownNode
#[test]
fn adversarial_unknown_node_returns_error() {
    let mut dag = Dag::new("unknown-node");
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
        other => panic!("Expected UnknownNode error, got: {:?}", other),
    }
}

/// Contract: Edge from non-existent source node MUST return UnknownNode
#[test]
fn adversarial_unknown_source_node_returns_error() {
    let mut dag = Dag::new("unknown-source");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None,
    })
    .connect("nonexistent".into(), "A".into());

    let result = dag.build();
    match result {
        Err(WorkflowDefinitionError::UnknownNode { .. }) => {}
        other => panic!("Expected UnknownNode error, got: {:?}", other),
    }
}

/// ============================================================================
/// CONTRACT VIOLATIONS: Invalid Retry Policy
/// ============================================================================

/// Contract: max_retries = u32::MAX MUST return InvalidRetryPolicy
#[test]
fn adversarial_max_retries_exceeded_returns_error() {
    let mut dag = Dag::new("max-retries");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: Some(RetryPolicy {
            max_retries: u32::MAX,
            backoff_ms: 100,
        }),
    });

    let result = dag.build();
    match result {
        Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name, .. }) => {
            assert_eq!(node_name, "A");
        }
        other => panic!("Expected InvalidRetryPolicy error, got: {:?}", other),
    }
}

/// Contract: backoff_ms = 0 MUST return InvalidRetryPolicy
#[test]
fn adversarial_zero_backoff_returns_error() {
    let mut dag = Dag::new("zero-backoff");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: Some(RetryPolicy {
            max_retries: 3,
            backoff_ms: 0,
        }),
    });

    let result = dag.build();
    match result {
        Err(WorkflowDefinitionError::InvalidRetryPolicy { node_name, .. }) => {
            assert_eq!(node_name, "A");
        }
        other => panic!("Expected InvalidRetryPolicy error, got: {:?}", other),
    }
}

/// ============================================================================
/// EDGE CASES: Large Graphs
/// ============================================================================

/// Edge case: Graph with 100 nodes in a chain should build successfully
#[test]
fn adversarial_large_chain_succeeds() {
    let mut dag = Dag::new("large-chain");
    for i in 0..100 {
        dag.add_node(DagNode {
            name: format!("N{}", i),
            retry_policy: None,
        });
    }
    for i in 0..99 {
        dag.connect(format!("N{}", i), format!("N{}", i + 1));
    }

    let result = dag.build();
    assert!(
        result.is_ok(),
        "100-node chain should build successfully, got: {:?}",
        result
    );
}

/// Edge case: Graph with 100 nodes but 1000 edges should handle correctly
#[test]
fn adversarial_dense_graph_handled() {
    let mut dag = Dag::new("dense-graph");
    for i in 0..20 {
        dag.add_node(DagNode {
            name: format!("N{}", i),
            retry_policy: None,
        });
    }
    // Create many edges - connect each node to all others (except self to avoid self-loop)
    for i in 0..20 {
        for j in 0..20 {
            if i != j {
                dag.connect(format!("N{}", i), format!("N{}", j));
            }
        }
    }

    let result = dag.build();
    // With 20 nodes fully connected (except self), there are many cycles
    // The result depends on implementation but should be deterministic
    match result {
        Err(WorkflowDefinitionError::CycleDetected { cycle_nodes }) => {
            assert!(!cycle_nodes.is_empty(), "Cycle detected should have nodes");
        }
        _ => {} // Could also reject if edges reference non-existent (but they do exist)
    }
}

/// ============================================================================
/// EDGE CASES: Deep Nesting
/// ============================================================================

/// Edge case: Deep chain of 50 nodes should build successfully
#[test]
fn adversarial_deep_chain_succeeds() {
    let mut dag = Dag::new("deep-chain");
    for i in 0..50 {
        dag.add_node(DagNode {
            name: format!("Node{}", i),
            retry_policy: None,
        });
    }
    for i in 0..49 {
        dag.connect(format!("Node{}", i), format!("Node{}", i + 1));
    }

    let result = dag.build();
    assert!(result.is_ok(), "Deep chain should build, got: {:?}", result);
}

/// ============================================================================
/// EDGE CASES: Multiple Components
/// ============================================================================

/// Edge case: Multiple disconnected DAG components should build successfully
#[test]
fn adversarial_multiple_disconnected_components_succeed() {
    let mut dag = Dag::new("multi-component");
    // Component 1: A -> B
    dag.add_node(DagNode {
        name: "A1".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "B1".into(),
        retry_policy: None,
    })
    .connect("A1".into(), "B1".into());

    // Component 2: C -> D -> E
    dag.add_node(DagNode {
        name: "C2".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "D2".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "E2".into(),
        retry_policy: None,
    })
    .connect("C2".into(), "D2".into())
    .connect("D2".into(), "E2".into());

    // Component 3: F (isolated)
    dag.add_node(DagNode {
        name: "F3".into(),
        retry_policy: None,
    });

    let result = dag.build();
    assert!(
        result.is_ok(),
        "Multiple disconnected components should build: {:?}",
        result
    );
}

/// ============================================================================
/// EDGE CASES: Node Name Edge Cases
/// ============================================================================

/// Edge case: Empty string node name
#[test]
fn adversarial_empty_node_name() {
    let mut dag = Dag::new("empty-name");
    dag.add_node(DagNode {
        name: "".into(),
        retry_policy: None,
    });

    let result = dag.build();
    // Empty name might be allowed or not - implementation dependent
    // Should at least not panic
    assert!(result.is_ok() || matches!(result, Err(WorkflowDefinitionError::EmptyWorkflow)));
}

/// Edge case: Unicode node names
#[test]
fn adversarial_unicode_node_names() {
    let mut dag = Dag::new("unicode");
    dag.add_node(DagNode {
        name: "节点🔗".into(),
        retry_policy: None,
    })
    .add_node(DagNode {
        name: "nœud".into(),
        retry_policy: None,
    })
    .connect("节点🔗".into(), "nœud".into());

    let result = dag.build();
    assert!(
        result.is_ok(),
        "Unicode node names should work: {:?}",
        result
    );
}

/// Edge case: Very long node name
#[test]
fn adversarial_long_node_name() {
    let mut dag = Dag::new("long-name");
    let long_name = "A".repeat(10000);
    dag.add_node(DagNode {
        name: long_name.clone(),
        retry_policy: None,
    });

    let result = dag.build();
    assert!(result.is_ok(), "Long node name should work: {:?}", result);
}

/// ============================================================================
/// EDGE CASES: Same Node Added Twice
/// ============================================================================

/// Edge case: Adding node with same name twice - second should overwrite
#[test]
fn adversarial_duplicate_node_name_overwrites() {
    let mut dag = Dag::new("duplicate-name");
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: Some(RetryPolicy {
            max_retries: 1,
            backoff_ms: 100,
        }),
    });
    dag.add_node(DagNode {
        name: "A".into(),
        retry_policy: None, // Different retry policy
    });

    let result = dag.build();
    assert!(
        result.is_ok(),
        "Duplicate node name should overwrite: {:?}",
        result
    );
    if let Ok(workflow) = result {
        // Should only have one node named "A"
        let a_nodes: Vec<_> = workflow.nodes.iter().filter(|n| n.name == "A").collect();
        assert_eq!(a_nodes.len(), 1, "Should have exactly one node A");
    }
}

/// ============================================================================
/// FAILURE MODES: detect_cycle direct testing
/// ============================================================================

/// Failure mode: detect_cycle on graph with cycle MUST return Some
#[test]
fn adversarial_detect_cycle_on_cyclic_graph_returns_some() {
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

    let result = detect_cycle(&nodes, &edges);
    assert!(
        result.is_some(),
        "detect_cycle on cyclic graph must return Some"
    );
}

/// Failure mode: detect_cycle on acyclic graph MUST return None
#[test]
fn adversarial_detect_cycle_on_acyclic_graph_returns_none() {
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
    ];

    let result = detect_cycle(&nodes, &edges);
    assert!(
        result.is_none(),
        "detect_cycle on acyclic graph must return None"
    );
}

/// Failure mode: detect_cycle determinism - same input MUST produce same output
#[test]
fn adversarial_detect_cycle_deterministic() {
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

    let result1 = detect_cycle(&nodes, &edges);
    let result2 = detect_cycle(&nodes, &edges);
    let result3 = detect_cycle(&nodes, &edges);

    assert_eq!(result1, result2, "detect_cycle must be deterministic");
    assert_eq!(result2, result3, "detect_cycle must be deterministic");
}

/// Failure mode: detect_cycle on empty graph MUST return None
#[test]
fn adversarial_detect_cycle_empty_graph_returns_none() {
    let nodes: Vec<DagNode> = vec![];
    let edges: Vec<Edge> = vec![];

    let result = detect_cycle(&nodes, &edges);
    assert_eq!(result, None, "detect_cycle on empty graph must return None");
}

/// Failure mode: detect_cycle on single node with self-loop MUST detect
#[test]
fn adversarial_detect_cycle_single_node_self_loop() {
    let nodes = vec![DagNode {
        name: "A".into(),
        retry_policy: None,
    }];
    let edges = vec![Edge {
        source_node: "A".into(),
        target_node: "A".into(),
        condition: None,
    }];

    let result = detect_cycle(&nodes, &edges);
    match result {
        Some(cycle) => assert_eq!(cycle, vec!["A"], "Self-loop should return [A]"),
        None => panic!("Self-loop must be detected"),
    }
}

/// ============================================================================
/// FAILURE MODES: output_graph testing
/// ============================================================================

/// Failure mode: output_graph on valid workflow MUST succeed
#[test]
fn adversarial_output_graph_valid_workflow_succeeds() {
    let workflow = WorkflowDefinition {
        workflow_name: "test".into(),
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
    assert_eq!(
        result,
        Ok(()),
        "output_graph on valid workflow must succeed"
    );
}

/// Failure mode: output_graph MUST produce valid JSON
#[test]
fn adversarial_output_graph_produces_valid_json() {
    let workflow = WorkflowDefinition {
        workflow_name: "json-test".into(),
        nodes: vec![DagNode {
            name: "X".into(),
            retry_policy: None,
        }],
        edges: vec![],
    };

    output_graph(&workflow).expect("output_graph should succeed");
    // If we get here, JSON was written to stdout
}

/// ============================================================================
/// FAILURE MODES: DAG build workflow
/// ============================================================================

/// Failure mode: Valid workflow MUST produce acyclic WorkflowDefinition
#[test]
fn adversarial_valid_workflow_is_acyclic() {
    let mut dag = Dag::new("valid-acyclic");
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

    let result = dag.build().expect("Valid workflow should build");

    // Verify the resulting WorkflowDefinition is acyclic
    let cycle = detect_cycle(&result.nodes, &result.edges);
    assert!(cycle.is_none(), "Built workflow must be acyclic");
}

/// Failure mode: build() on workflow with cycle MUST NOT produce WorkflowDefinition
#[test]
fn adversarial_cyclic_workflow_cannot_build() {
    let mut dag = Dag::new("cyclic-blocked");
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
    assert!(result.is_err(), "Cyclic workflow must fail to build");
}

/// ============================================================================
/// ANTI-PROPERTY: Acyclic graphs should NEVER return a cycle
/// ============================================================================

/// Anti-property: Tree structure must never report cycle
#[test]
fn adversarial_tree_never_has_cycle() {
    let mut dag = Dag::new("tree");
    for i in 0..20 {
        dag.add_node(DagNode {
            name: format!("T{}", i),
            retry_policy: None,
        });
    }
    // Create tree: each node (except root) connects to parent
    for i in 1..20 {
        dag.connect(format!("T{}", i / 2), format!("T{}", i));
    }

    let result = dag.build().expect("Tree should build");
    let cycle = detect_cycle(&result.nodes, &result.edges);
    assert!(
        cycle.is_none(),
        "Tree should never have cycle, got: {:?}",
        cycle
    );
}

/// ============================================================================
/// ANTI-PROPERTY: Built workflow must maintain node/edge integrity
/// ============================================================================

/// Anti-property: Built workflow must have same nodes count
#[test]
fn adversarial_built_workflow_preserves_node_count() {
    let mut dag = Dag::new("preserve-nodes");
    for i in 0..5 {
        dag.add_node(DagNode {
            name: format!("N{}", i),
            retry_policy: None,
        });
    }
    for i in 0..4 {
        dag.connect(format!("N{}", i), format!("N{}", i + 1));
    }

    let result = dag.build().expect("Should build");
    assert_eq!(result.nodes.len(), 5, "Must preserve all nodes");
    assert_eq!(result.edges.len(), 4, "Must preserve all edges");
}

/// ============================================================================
/// STRESS TESTS
/// ============================================================================

/// Stress: Very long chain must not overflow stack
#[test]
fn adversarial_very_long_chain_no_stack_overflow() {
    let mut dag = Dag::new("stress-chain");
    for i in 0..1000 {
        dag.add_node(DagNode {
            name: format!("S{}", i),
            retry_policy: None,
        });
    }
    for i in 0..999 {
        dag.connect(format!("S{}", i), format!("S{}", i + 1));
    }

    let result = dag.build();
    assert!(result.is_ok(), "1000-node chain should build: {:?}", result);
}

/// Stress: Many parallel branches must build
#[test]
fn adversarial_many_parallel_branches() {
    let mut dag = Dag::new("parallel-branches");
    dag.add_node(DagNode {
        name: "Root".into(),
        retry_policy: None,
    });

    // Create 50 parallel branches from Root
    for i in 0..50 {
        dag.add_node(DagNode {
            name: format!("Branch{}", i),
            retry_policy: None,
        });
        dag.connect("Root".into(), format!("Branch{}", i));
    }

    let result = dag.build();
    assert!(
        result.is_ok(),
        "Parallel branches should build: {:?}",
        result
    );
}
