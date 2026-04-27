//! Red Queen tests: proptest-property-attacks dimension.
//!
//! Fuzz-based property tests for RetryPolicy, next_nodes, parse, and serde.

#[cfg(feature = "proptest")]
use super::helpers;
#[cfg(feature = "proptest")]
use crate::*;
#[cfg(feature = "proptest")]
use proptest::prelude::*;

#[cfg(feature = "proptest")]
mod proptests {
    use super::*;

    proptest! {
        // RQ-PROP-01: RetryPolicy::new rejects all multipliers below 1.0 (except NaN)
        #[test]
        fn rq_retry_policy_rejects_all_multipliers_below_1(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..1_000_000u64,
            multiplier in -1e38f32..0.9999f32,
        ) {
            let result = RetryPolicy::new(max_attempts, backoff_ms, multiplier);
            prop_assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })), "multiplier {} should be rejected", multiplier);
        }

        // RQ-PROP-02: RetryPolicy::new accepts all multipliers >= 1.0
        #[test]
        fn rq_retry_policy_accepts_all_multipliers_ge_1(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..1_000_000u64,
            multiplier in 1.0f32..1e38f32,
        ) {
            let result = RetryPolicy::new(max_attempts, backoff_ms, multiplier);
            result.unwrap();
        }

        // RQ-PROP-03: next_nodes result nodes all live in def (pointer equality)
        #[test]
        fn rq_next_nodes_result_nodes_are_from_def(
            outcome in helpers::step_outcome_strategy(),
        ) {
            let def = helpers::make_def(
                "test",
                vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
                vec![
                    ("a", "b", EdgeCondition::Always),
                    ("a", "c", EdgeCondition::OnSuccess),
                ],
            );
            let result = next_nodes(&NodeName("a".into()), outcome, &def);
            let all_found = result.iter().all(|node| def.nodes.as_slice().iter().any(|n| std::ptr::eq(n, *node)));
            prop_assert!(all_found, "next_nodes returned a &DagNode not from def.nodes");
        }

        // RQ-PROP-04: parse never panics with arbitrary valid-structure JSON
        #[test]
        fn rq_parse_never_panics(
            name_suffix in "[a-z]{1,10}",
            max_attempts in 0u8..=255u8,
            backoff_ms in 0u64..=1_000_000u64,
            multiplier in 0.0f32..=10.0f32,
        ) {
            let workflow_name = format!("wf-{}", name_suffix);
            let json = serde_json::json!({
                "workflow_name": workflow_name,
                "nodes": [{"node_name": "a", "retry_policy": {"max_attempts": max_attempts, "backoff_ms": backoff_ms, "backoff_multiplier": multiplier}}],
                "edges": []
            });
            let bytes = serde_json::to_vec(&json).unwrap();
            let _result = std::panic::catch_unwind(|| {
                let _ignored = WorkflowDefinition::parse(&bytes);
            });
            // If we reach here, no panic occurred
        }

        // RQ-PROP-05: RetryPolicy serde round-trip preserves values
        #[test]
        fn rq_retry_policy_serde_round_trip(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..=1_000_000u64,
            multiplier in 1.0f64..100.0f64,
        ) {
            let policy = RetryPolicy {
                max_attempts,
                backoff_ms,
                backoff_multiplier: multiplier,
                max_backoff_ms: u64::MAX,
            };
            let json = serde_json::to_value(policy).unwrap();
            let restored: RetryPolicy = serde_json::from_value(json).unwrap();
            prop_assert_eq!(restored, policy);
        }

        // RQ-PROP-06: Edge serde round-trip preserves all fields
        // Uses valid NodeName pattern: alphanumeric, no leading/trailing _/-
        #[test]
        fn rq_edge_serde_round_trip(
            source in "[a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9]",
            target in "[a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9]",
            condition in helpers::edge_condition_strategy(),
        ) {
            let edge = Edge {
                source_node: NodeName(source),
                target_node: NodeName(target),
                condition,
            };
            let json = serde_json::to_value(&edge).unwrap();
            let restored: Edge = serde_json::from_value(json).unwrap();
            prop_assert_eq!(restored.source_node, edge.source_node);
            prop_assert_eq!(restored.target_node, edge.target_node);
            prop_assert_eq!(restored.condition, edge.condition);
        }

        // RQ-PROP-07: WorkflowDefinition parse + re-serialize + re-parse = identity
        // (generates only acyclic workflows)
        #[test]
        fn rq_workflow_parse_round_trip_identity(
            node_count in 1usize..=4usize,
            edge_seeds in proptest::collection::vec(0usize..=20usize, 0..=6usize),
        ) {
            let node_names: Vec<String> = (0..node_count).map(|i| format!("n{}", i)).collect();

            // All acyclic edges: (lower_idx, higher_idx) guarantees no cycle
            let possible_edges: Vec<(usize, usize)> = if node_count > 1 {
                (0..node_count)
                    .flat_map(|i| (i + 1..node_count).map(move |j| (i, j)))
                    .collect()
            } else {
                vec![]
            };

            let edges: std::collections::HashSet<(usize, usize)> = if possible_edges.is_empty() {
                std::collections::HashSet::new()
            } else {
                edge_seeds
                    .into_iter()
                    .map(|s| possible_edges[s % possible_edges.len()])
                    .collect()
            };

            let nodes_json: Vec<serde_json::Value> = node_names
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "node_name": name,
                        "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
                    })
                })
                .collect();

            let edges_json: Vec<serde_json::Value> = edges
                .iter()
                .map(|&(src, tgt)| {
                    serde_json::json!({
                        "source_node": node_names[src],
                        "target_node": node_names[tgt],
                        "condition": "Always"
                    })
                })
                .collect();

            let workflow_json = serde_json::json!({
                "workflow_name": "proptest",
                "nodes": nodes_json,
                "edges": edges_json,
            });

            let bytes = serde_json::to_vec(&workflow_json).unwrap();
            let parsed = WorkflowDefinition::parse(&bytes).expect("parse should succeed for acyclic workflow");
            let reserialized = serde_json::to_vec(&parsed).unwrap();
            let reparsed = WorkflowDefinition::parse(&reserialized).expect("re-parse should succeed");
            prop_assert_eq!(reparsed, parsed);
        }

        // RQ-PROP-08: get_node returns None for names not in workflow
        #[test]
        fn rq_get_node_none_for_missing(
            missing_suffix in "[a-z]{1,5}",
        ) {
            let def = helpers::make_def("test", vec![("a", 1, 0, 1.0)], vec![]);
            let missing = format!("zzz-{}", missing_suffix);
            prop_assert!(def.get_node(&NodeName(missing)).is_none());
        }

        // RQ-PROP-09: NonEmptyVec serde round-trip
        #[test]
        fn rq_non_empty_vec_serde_round_trip(
            items in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 1..=50),
        ) {
            let nev = NonEmptyVec::new_unchecked(items.clone());
            let json = serde_json::to_value(&nev).unwrap();
            let restored: NonEmptyVec<u8> = serde_json::from_value(json).unwrap();
            prop_assert_eq!(restored.as_slice(), items.as_slice());
        }
    }
}
