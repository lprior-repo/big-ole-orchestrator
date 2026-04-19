//! Property tests for DAG execution engine correctness.
//!
//! bead_id: ve-nzmc1
//!
//! Verifies invariants of the DependencyGraphResolver and WorkflowDefinition
//! using randomly generated DAGs.

#![allow(clippy::unwrap_used)]

use proptest::prelude::*;

use crate::{
    DagNode, DependencyGraphResolver, Edge, EdgeCondition, NodeName, NonEmptyVec, RetryPolicy,
    WorkflowDefinition,
};

/// Strategy: generate a random acyclic DAG.
/// Nodes are named n0..n{N-1}. Edges only go from lower to higher index.
fn random_acyclic_dag() -> impl Strategy<Value = (Vec<String>, Vec<(usize, usize)>)> {
    let node_count = 2usize..=30;
    node_count.prop_flat_map(|nc| {
        let nodes = proptest::collection::vec(
            proptest::string::string_regex("[a-z][a-z0-9]{0,9}").unwrap(),
            nc,
        );
        let edges = proptest::collection::vec(
            (0usize..nc, 0usize..nc).prop_filter_map("src < target", |(src, tgt)| {
                if src < tgt {
                    Some((src, tgt))
                } else {
                    None
                }
            }),
            0..nc * 3,
        );
        (nodes, edges)
    })
}

fn build_workflow(
    node_names: &[String],
    edges: &[(usize, usize)],
    condition: EdgeCondition,
) -> WorkflowDefinition {
    let nodes: Vec<DagNode> = node_names
        .iter()
        .map(|name| DagNode {
            node_name: NodeName(name.clone()),
            retry_policy: RetryPolicy {
                max_attempts: 1,
                backoff_ms: 0,
                backoff_multiplier: 1.0,
                max_backoff_ms: u64::MAX,
            },
            compensation_policy: None,
        })
        .collect();

    let edges: Vec<Edge> = edges
        .iter()
        .map(|(src, tgt)| Edge {
            source_node: NodeName(node_names[*src].clone()),
            target_node: NodeName(node_names[*tgt].clone()),
            condition,
        })
        .collect();

    WorkflowDefinition {
        workflow_name: crate::WorkflowName("proptest-dag".into()),
        nodes: NonEmptyVec::new_unchecked(nodes),
        edges,
    }
}

// ============================================================================
// Property 1: execution_layers produces a valid topological ordering
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]
    #[test]
    fn execution_layers_produces_valid_topological_order(
        (node_names, edges) in random_acyclic_dag()
    ) {
        prop_assume!(node_names.len() >= 2);
        // Ensure unique node names (proptest string strategy can produce duplicates)
        let unique_count = {
            let mut seen = std::collections::HashSet::new();
            node_names.iter().filter(|n| seen.insert(n.as_str())).count()
        };
        prop_assume!(unique_count == node_names.len());

        let conditions = [EdgeCondition::Always, EdgeCondition::OnSuccess, EdgeCondition::OnFailure];
        for &condition in &conditions {
            let wf = build_workflow(&node_names, &edges, condition);
            let layers = DependencyGraphResolver::execution_layers(&wf);

            // Invariant 1: All nodes appear exactly once
            let all_nodes: Vec<&NodeName> = layers.iter().flatten().collect();
            let unique: std::collections::HashSet<&NodeName> = all_nodes.iter().copied().collect();
            prop_assert_eq!(unique.len(), node_names.len(),
                "All nodes must appear exactly once across layers");

            // Invariant 2: No duplicates
            prop_assert_eq!(all_nodes.len(), node_names.len(),
                "Total node count across layers must equal workflow node count");

            // Invariant 3: No node appears in a layer before all its dependencies
            let mut node_layer: std::collections::HashMap<NodeName, usize> = std::collections::HashMap::new();
            for (layer_idx, layer) in layers.iter().enumerate() {
                for node in layer {
                    node_layer.insert(node.clone(), layer_idx);
                }
            }

            for edge in &wf.edges {
                if let (Some(&src_layer), Some(&tgt_layer)) =
                    (node_layer.get(&edge.source_node), node_layer.get(&edge.target_node))
                {
                    prop_assert!(src_layer < tgt_layer,
                        "Edge {:?} -> {:?} violates topological order: src in layer {}, tgt in layer {}",
                        edge.source_node, edge.target_node, src_layer, tgt_layer);
                }
            }
        }
    }
}

// ============================================================================
// Property 2: execution_layers has no intra-layer dependencies
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]
    #[test]
    fn execution_layers_no_intra_layer_dependencies(
        (node_names, edges) in random_acyclic_dag()
    ) {
        prop_assume!(node_names.len() >= 2);
        let unique_count = {
            let mut seen = std::collections::HashSet::new();
            node_names.iter().filter(|n| seen.insert(n.as_str())).count()
        };
        prop_assume!(unique_count == node_names.len());

        let wf = build_workflow(&node_names, &edges, EdgeCondition::Always);
        let layers = DependencyGraphResolver::execution_layers(&wf);

        for (layer_idx, layer) in layers.iter().enumerate() {
            let layer_set: std::collections::HashSet<&NodeName> = layer.iter().collect();

            for node in layer {
                let deps = DependencyGraphResolver::dependencies(&wf, node);
                for dep in &deps {
                    prop_assert!(!layer_set.contains(dep),
                        "Node {:?} in layer {} has dependency {:?} in same layer",
                        node, layer_idx, dep);
                }
            }
        }
    }
}

// ============================================================================
// Property 3: ready_nodes is consistent with execution_layers layer-by-layer
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]
    #[test]
    fn ready_nodes_matches_execution_layers_incremental(
        (node_names, edges) in random_acyclic_dag()
    ) {
        prop_assume!(node_names.len() >= 2);
        let unique_count = {
            let mut seen = std::collections::HashSet::new();
            node_names.iter().filter(|n| seen.insert(n.as_str())).count()
        };
        prop_assume!(unique_count == node_names.len());

        let wf = build_workflow(&node_names, &edges, EdgeCondition::Always);
        let layers = DependencyGraphResolver::execution_layers(&wf);

        let mut completed: Vec<NodeName> = Vec::new();
        for (layer_idx, layer) in layers.iter().enumerate() {
            let ready = DependencyGraphResolver::ready_nodes(&wf, &completed);

            let ready_set: std::collections::HashSet<&NodeName> = ready.iter().collect();
            let layer_set: std::collections::HashSet<&NodeName> = layer.iter().collect();

            prop_assert_eq!(ready_set, layer_set,
                "At layer {}, ready nodes != expected layer", layer_idx);

            completed.extend(layer.iter().cloned());
        }

        let ready = DependencyGraphResolver::ready_nodes(&wf, &completed);
        prop_assert!(ready.is_empty(),
            "After all layers completed, ready nodes should be empty");
    }
}
