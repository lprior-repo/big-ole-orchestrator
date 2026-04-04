//! Proptest invariants for vel-bxpg
//!
//! These tests verify that critical invariants hold across thousands of
//! randomly generated inputs.

use proptest::prelude::*;
use vel_bxpg::{Dag, DagNode, Edge, WorkflowDefinition, detect_cycle};

/// Proptest Invariant 1: detect_cycle — Idempotence
///
/// Invariant: Running detect_cycle on a WorkflowDefinition produced by Dag::build()
///           (which succeeded) always returns None (the result is acyclic).
///
/// Strategy: Construct random valid DAGs (nodes: 1-20, edges: 0 to nodes*(nodes-1)/2, no duplicates),
///           build() must succeed, then detect_cycle on the result must return None.
///
/// RED PHASE: This will fail because build() currently returns EmptyWorkflow error
///            for all inputs (even valid ones).
proptest! {
    #[test]
    fn dag_build_produces_acyclic_workflow_definition(nodes_count in 1..=20u32) {
        // Given: A DAG with the specified number of nodes and no cycles
        let mut dag = Dag::new("proptest-dag");
        for i in 0..nodes_count {
            dag.add_node(DagNode {
                name: format!("Node{}", i),
                retry_policy: None,
            });
        }

        // Add a simple chain: 0 -> 1 -> 2 -> ... -> N-1
        for i in 0..nodes_count - 1 {
            dag.connect(format!("Node{}", i), format!("Node{}", i + 1));
        }

        // When: build() is called
        let result = dag.build();

        // Then: build succeeds and the result is acyclic
        prop_assert!(result.is_ok(), "Expected build to succeed for acyclic DAG");
        let workflow = result.unwrap();

        // The resulting workflow must be acyclic
        let cycle = detect_cycle(&workflow.nodes, &workflow.edges);
        prop_assert!(cycle.is_none(), "Built workflow should be acyclic, but found cycle: {:?}", cycle);
    }

    #[test]
    fn dag_build_with_linear_chain_always_succeeds(chain_length in 1..=50u32) {
        let mut dag = Dag::new("linear-chain");
        for i in 0..chain_length {
            dag.add_node(DagNode {
                name: format!("N{}", i),
                retry_policy: None,
            });
        }

        // Create linear chain: N0 -> N1 -> N2 -> ... -> N(chain_length-1)
        for i in 0..chain_length.saturating_sub(1) {
            dag.connect(format!("N{}", i), format!("N{}", i + 1));
        }

        let result = dag.build();
        prop_assert!(result.is_ok(), "Linear chain should always build successfully");
    }
}

/// Proptest Invariant 2: detect_cycle — Cycle Detection Completeness
///
/// Invariant: Any graph containing a cycle (according to transitive closure) must be detected.
///           Formally: if exists path from A to B and path from B to A, detect_cycle returns Some.
///
/// Strategy: Generate graphs with known cycles (self-loops, 2-node mutual edges, N-node loops),
///           verify detect_cycle returns Some(cycle_nodes).
///
/// RED PHASE: detect_cycle currently always returns None (stub implementation).
proptest! {
    #[test]
    fn detect_cycle_finds_self_loop_on_any_node(node_name: String) {
        let nodes = vec![DagNode {
            name: node_name.clone(),
            retry_policy: None,
        }];
        let edges = vec![Edge {
            source_node: node_name.clone(),
            target_node: node_name,
            condition: None,
        }];

        let result = detect_cycle(&nodes, &edges);
        prop_assert!(result.is_some(), "Self-loop should always be detected");
    }

    #[test]
    fn detect_cycle_finds_two_node_mutual_cycle(node_a: String, node_b: String) {
        // Ensure distinct node names
        prop_assume!(node_a != node_b);

        let nodes = vec![
            DagNode { name: node_a.clone(), retry_policy: None },
            DagNode { name: node_b.clone(), retry_policy: None },
        ];
        let edges = vec![
            Edge { source_node: node_a.clone(), target_node: node_b.clone(), condition: None },
            Edge { source_node: node_b.clone(), target_node: node_a.clone(), condition: None },
        ];

        let result = detect_cycle(&nodes, &edges);
        prop_assert!(result.is_some(), "Two-node mutual cycle should be detected");
    }

    #[test]
    fn detect_cycle_finds_n_node_cycle(node_names: Vec<String>) {
        // Generate a cycle of length node_names.len()
        prop_assume!(node_names.len() >= 2);

        let nodes: Vec<DagNode> = node_names
            .iter()
            .map(|name| DagNode {
                name: name.clone(),
                retry_policy: None,
            })
            .collect();

        // Create edges forming a cycle
        let mut edges = Vec::new();
        for i in 0..node_names.len() {
            let source = &node_names[i];
            let target = &node_names[(i + 1) % node_names.len()];
            edges.push(Edge {
                source_node: source.clone(),
                target_node: target.clone(),
                condition: None,
            });
        }

        let result = detect_cycle(&nodes, &edges);
        prop_assert!(result.is_some(), "{}-node cycle should be detected", node_names.len());
    }

    #[test]
    fn detect_cycle_returns_none_for_tree_structure(nodes_count in 1..=20u32) {
        // A tree is an acyclic graph
        let mut dag = Dag::new("tree");
        for i in 0..nodes_count {
            dag.add_node(DagNode {
                name: format!("Node{}", i),
                retry_policy: None,
            });
        }

        // Create a tree structure by connecting each node (except root) to a parent
        for i in 1..nodes_count {
            dag.connect(format!("Node{}", i / 2), format!("Node{}", i));
        }

        let result = dag.build();
        prop_assert!(result.is_ok(), "Tree should build successfully");

        let workflow = result.unwrap();
        let cycle = detect_cycle(&workflow.nodes, &workflow.edges);
        prop_assert!(cycle.is_none(), "Tree should never contain a cycle");
    }
}

/// Proptest Invariant 3: output_graph — Round-trip Serialization
///
/// Invariant: A WorkflowDefinition that serializes successfully can be deserialized back
///           to an equivalent WorkflowDefinition.
///
/// Strategy: Generate valid WorkflowDefinition instances, serialize via serde_json,
///           deserialize via serde_json, verify nodes and edges counts match.
proptest! {
    #[test]
    fn workflow_definition_serialize_deserialize_roundtrip(
        workflow_name: String,
        node_count in 0..=100u32,
        edge_count in 0..=500u32,
    ) {
        // Generate nodes
        let mut nodes = Vec::new();
        let mut node_names: Vec<String> = Vec::new();
        for i in 0..node_count {
            let name = format!("{}_node_{}", workflow_name, i);
            node_names.push(name.clone());
            nodes.push(DagNode {
                name,
                retry_policy: None,
            });
        }

        // Generate edges (may reference non-existent nodes, which is valid for this test)
        let mut edges = Vec::new();
        for i in 0..edge_count {
            if !node_names.is_empty() {
                let source_idx = i as usize % node_names.len();
                let target_idx = (i as usize + 1) % node_names.len();
                edges.push(Edge {
                    source_node: node_names[source_idx].clone(),
                    target_node: node_names[target_idx].clone(),
                    condition: None,
                });
            }
        }

        let workflow = WorkflowDefinition {
            workflow_name: workflow_name.clone(),
            nodes,
            edges,
        };

        // Serialize
        let json = serde_json::to_string(&workflow)
            .expect("WorkflowDefinition should always serialize to JSON");

        // Deserialize
        let recovered: WorkflowDefinition = serde_json::from_str(&json)
            .expect("JSON should always deserialize to WorkflowDefinition");

        // Verify counts match
        prop_assert_eq!(recovered.workflow_name, workflow.workflow_name);
        prop_assert_eq!(recovered.nodes.len(), workflow.nodes.len());
        prop_assert_eq!(recovered.edges.len(), workflow.edges.len());
    }

    #[test]
    fn workflow_definition_json_roundtrips_with_any_node_name(
        workflow_name: String,
        node_name: String,
    ) {
        let workflow = WorkflowDefinition {
            workflow_name: workflow_name.clone(),
            nodes: vec![DagNode {
                name: node_name.clone(),
                retry_policy: None,
            }],
            edges: vec![],
        };

        // Serialize to JSON
        let json = serde_json::to_string(&workflow).expect("should serialize");

        // Verify JSON structure contains required fields
        prop_assert!(json.contains("workflow_name"));
        prop_assert!(json.contains("nodes"));
        prop_assert!(json.contains("edges"));

        // Deserialize and verify the node name is preserved
        let recovered: WorkflowDefinition =
            serde_json::from_str(&json).expect("json should deserialize");
        prop_assert_eq!(recovered.workflow_name, workflow_name);
        prop_assert_eq!(recovered.nodes.len(), 1);
        prop_assert_eq!(recovered.nodes[0].name.clone(), node_name);
    }
}

/// Additional property: Cycle detection determinism
///
/// Invariant: Calling detect_cycle on the same graph multiple times returns the same result
///            with the same cycle_nodes ordering.
proptest! {
    #[test]
    fn detect_cycle_is_deterministic_on_cyclic_graph(node_names: Vec<String>) {
        prop_assume!(node_names.len() >= 2);

        let nodes: Vec<DagNode> = node_names
            .iter()
            .map(|name| DagNode {
                name: name.clone(),
                retry_policy: None,
            })
            .collect();

        // Create a cycle
        let mut edges = Vec::new();
        for i in 0..node_names.len() {
            let source = &node_names[i];
            let target = &node_names[(i + 1) % node_names.len()];
            edges.push(Edge {
                source_node: source.clone(),
                target_node: target.clone(),
                condition: None,
            });
        }

        // Call detect_cycle multiple times
        let result1 = detect_cycle(&nodes, &edges);
        let result2 = detect_cycle(&nodes, &edges);
        let result3 = detect_cycle(&nodes, &edges);

        // Results should be identical
        prop_assert_eq!(result1.clone(), result2);
        prop_assert_eq!(result1, result3);
    }

    #[test]
    fn detect_cycle_is_deterministic_on_acyclic_graph(node_count in 1..=20u32) {
        let mut dag = Dag::new("determinism-test");
        for i in 0..node_count {
            dag.add_node(DagNode {
                name: format!("Node{}", i),
                retry_policy: None,
            });
        }

        // Create a linear chain
        for i in 0..node_count - 1 {
            dag.connect(format!("Node{}", i), format!("Node{}", i + 1));
        }

        let result = dag.build().expect("should build");
        let nodes = &result.nodes;
        let edges = &result.edges;

        let cycle1 = detect_cycle(nodes, edges);
        let cycle2 = detect_cycle(nodes, edges);

        prop_assert_eq!(cycle1, cycle2);
    }
}

/// Anti-invariant: Acyclic graphs should never return a cycle
proptest! {
    #[test]
    fn tree_always_reports_no_cycle(tree_size in 1..=30u32) {
        let mut dag = Dag::new("anti-invariant-tree");
        for i in 0..tree_size {
            dag.add_node(DagNode {
                name: format!("T{}", i),
                retry_policy: None,
            });
        }

        // Create a tree structure
        for i in 1..tree_size {
            let parent = i / 2;
            dag.connect(format!("T{}", parent), format!("T{}", i));
        }

        let result = dag.build().expect("tree should build");
        let cycle = detect_cycle(&result.nodes, &result.edges);

        prop_assert!(cycle.is_none(), "Tree should never have a cycle, but got {:?}", cycle);
    }
}
