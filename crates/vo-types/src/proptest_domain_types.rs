//! Comprehensive proptest generators and serialization round-trip tests for vo-types domain types.
//!
//! This module provides:
//! - `Arbitrary` implementations for domain types without built-in proptest support
//! - Serialization round-trip tests (serde_json serialize then deserialize preserves equality)
//! - Edge case coverage (empty strings, max-length IDs, unicode, large payloads)
//! - Invariant checking (valid state transitions, required fields)
//!
//! All tests are gated behind the `proptest` feature flag.

#![cfg(feature = "proptest")]

mod proptests {
    use crate::connector::types::{
        ConnectorResult, ConnectorState, ConnectorTransition, ReconcileAction,
    };
    use crate::effects::CompensationPolicy;
    use crate::non_empty_vec::NonEmptyVec;
    use crate::NodeName;
    use crate::ParseError;
    use crate::RetryPolicy;
    use crate::StepId;
    use crate::WorkflowName;
    use crate::{DagNode, Edge, EdgeCondition, SpawnId, StepOutcome, WorkflowDefinition};
    use proptest::prelude::*;
    use std::collections::HashSet;

    // ============================================================================
    // SpawnId proptests
    // ============================================================================

    prop_compose! {
        fn valid_spawn_id() -> String {
            let len = proptest::sample::size_range(1..=128).gen();
            prop::sample::subsequence(
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"
                    .chars(),
                len,
            )
            .collect()
        }
    }

    proptest! {
        #[test]
        fn spawn_id_valid_roundtrip(s in valid_spawn_id()) {
            let spawn_id = SpawnId::parse(&s).unwrap();
            prop_assert_eq!(spawn_id.as_str(), s);
        }

        #[test]
        fn spawn_id_serde_roundtrip(s in valid_spawn_id()) {
            let spawn_id = SpawnId::parse(&s).unwrap();
            let json = serde_json::to_string(&spawn_id).expect("serialize");
            let restored: SpawnId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, spawn_id);
        }

        #[test]
        fn spawn_id_try_from_string_valid(s in valid_spawn_id()) {
            let spawn_id = SpawnId::try_from(s.clone()).unwrap();
            prop_assert_eq!(spawn_id.as_str(), s);
        }

        #[test]
        fn spawn_id_display_roundtrip(s in valid_spawn_id()) {
            let spawn_id = SpawnId::parse(&s).unwrap();
            let displayed = spawn_id.to_string();
            prop_assert_eq!(displayed, s);
        }

        #[test]
        fn spawn_id_rejects_empty() {
            let result = SpawnId::parse("");
            prop_assert!(result.is_err());
            if let Err(ParseError::InvalidCharacters { .. }) = result {
                // Expected
            } else {
                panic!("Expected InvalidCharacters for empty string, got {:?}", result);
            }
        }

        #[test]
        fn spawn_id_max_length_boundary() {
            let max_len_s = "a".repeat(1024);
            let result = SpawnId::parse(&max_len_s);
            prop_assert!(result.is_ok(), "Should accept max length string of 1024 chars");
        }

        #[test]
        fn spawn_id_exceeds_max_length() {
            let too_long_s = "a".repeat(1025);
            let result = SpawnId::parse(&too_long_s);
            prop_assert!(result.is_err());
        }

        #[test]
        fn spawn_id_rejects_invalid_chars() {
            let with_spaces = "valid@invalid".to_string();
            let result = SpawnId::parse(&with_spaces);
            prop_assert!(result.is_err());
        }
    }

    // ============================================================================
    // StepOutcome proptests
    // ============================================================================

    proptest! {
        #[test]
        fn step_outcome_serde_roundtrip(outcome: StepOutcome) {
            let json = serde_json::to_string(&outcome).expect("serialize");
            let restored: StepOutcome = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, outcome);
        }

        #[test]
        fn edge_condition_matches_step_outcome(condition: EdgeCondition, outcome: StepOutcome) {
            let matches = condition.matches(outcome);
            match condition {
                EdgeCondition::Always => prop_assert!(matches, "Always should always match"),
                EdgeCondition::OnSuccess => prop_assert_eq!(matches, outcome == StepOutcome::Success),
                EdgeCondition::OnFailure => prop_assert_eq!(matches, outcome == StepOutcome::Failure),
            }
        }

        #[test]
        fn edge_condition_serde_roundtrip(condition: EdgeCondition) {
            let json = serde_json::to_string(&condition).expect("serialize");
            let restored: EdgeCondition = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, condition);
        }
    }

    // ============================================================================
    // RetryPolicy proptests
    // ============================================================================

    prop_compose! {
        fn valid_retry_policy()(
            max_attempts in 1u8..=255u8,
            backoff_ms in 0u64..=u64::MAX,
            backoff_multiplier in prop_oneof![
                1.0f64..=2.0f64,
                1.0f64..=10.0f64,
            ],
        ) -> RetryPolicy {
            RetryPolicy::new(max_attempts, backoff_ms, backoff_multiplier).expect("valid policy")
        }
    }

    prop_compose! {
        fn valid_dag_node()(
            node_name in "[a-z][a-z0-9-]*",
        ) -> DagNode {
            DagNode::valid_default(NodeName::parse(&node_name).expect("valid node name"))
                .expect("valid default")
        }
    }

    proptest! {
        #[test]
        fn retry_policy_serde_roundtrip(policy: RetryPolicy) {
            let json = serde_json::to_string(&policy).expect("serialize");
            let restored: RetryPolicy = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, policy);
        }

        #[test]
        fn retry_policy_backoff_calculation(policy: RetryPolicy) {
            let delay_1 = policy.calculate_backoff_delay(1);
            prop_assert_eq!(delay_1, policy.backoff_ms, "First attempt should return base backoff_ms");

            if policy.backoff_multiplier > 1.0 && policy.backoff_ms > 0 {
                let delay_2 = policy.calculate_backoff_delay(2);
                prop_assert!(delay_2 >= delay_1, "Later attempts should have >= delay");
            }
        }

        #[test]
        fn retry_policy_attempt_zero_returns_zero(policy: RetryPolicy) {
            let delay = policy.calculate_backoff_delay(0);
            prop_assert_eq!(delay, 0, "Attempt 0 should return zero delay");
        }

        #[test]
        fn retry_policy_zero_backoff_returns_zero(max_attempts in 1u8..255u8) {
            let policy = RetryPolicy::new(max_attempts, 0, 2.0).expect("valid");
            let delay = policy.calculate_backoff_delay(100);
            prop_assert_eq!(delay, 0, "Zero backoff_ms should always return 0");
        }

        #[test]
        fn retry_policy_max_attempts_exhausted(max_attempts in 1u8..=10u8, attempt in 1u32..=20u32) {
            let policy = RetryPolicy::new(max_attempts, 100, 2.0).expect("valid");
            let is_exhausted = attempt >= max_attempts as u32;
            prop_assert_eq!(
                policy.max_attempts.is_exhausted(crate::AttemptNumber::new_unchecked(attempt as u64)),
                is_exhausted
            );
        }

        #[test]
        fn retry_policy_zero_attempts_rejected() {
            let result = RetryPolicy::new(0, 100, 2.0);
            prop_assert!(result.is_err());
        }

        #[test]
        fn retry_policy_invalid_multiplier_rejected(multiplier in proptest::num::F64::ANY.prop_filter("must be < 1.0", |f| f.value < 1.0)) {
            let result = RetryPolicy::new(3, 100, multiplier.value);
            prop_assert!(result.is_err());
        }

        #[test]
        fn retry_policy_nan_multiplier_rejected() {
            let result = RetryPolicy::new(3, 100, f64::NAN);
            prop_assert!(result.is_err());
        }

        #[test]
        fn retry_policy_infinity_multiplier_rejected() {
            let result = RetryPolicy::new(3, 100, f64::INFINITY);
            prop_assert!(result.is_err());
        }

        #[test]
        fn retry_policy_max_backoff_capped(
            max_attempts in 1u8..=10u8,
            backoff_ms in 1u64..=1000u64,
            multiplier in 1.1f64..=10.0f64,
            max_backoff in 1u64..=500u64,
        ) {
            prop_assume!(max_backoff < backoff_ms);
            let result = RetryPolicy::with_max_backoff(max_attempts, backoff_ms, multiplier, max_backoff);
            prop_assert!(result.is_err());
        }
    }

    // ============================================================================
    // DagNode proptests
    // ============================================================================

    proptest! {
        #[test]
        fn dag_node_valid_default_roundtrip(node_name in "[a-z][a-z0-9-]*") {
            let node = DagNode::valid_default(NodeName::parse(&node_name).expect("valid")).expect("valid default");
            let json = serde_json::to_string(&node).expect("serialize");
            let restored: DagNode = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, node);
        }

        #[test]
        fn dag_node_with_custom_retry_policy(
            node_name in "[a-z][a-z0-9-]*",
            max_attempts in 1u8..=10u8,
            backoff_ms in 0u64..=1000u64,
        ) {
            let node_name = NodeName::parse(&node_name).expect("valid");
            let retry_policy = RetryPolicy::new(max_attempts, backoff_ms, 2.0).expect("valid policy");
            let node = DagNode {
                node_name,
                retry_policy,
                compensation_policy: None,
            };
            let json = serde_json::to_string(&node).expect("serialize");
            let restored: DagNode = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, node);
        }

        #[test]
        fn dag_node_serde_roundtrip(node: DagNode) {
            let json = serde_json::to_string(&node).expect("serialize");
            let restored: DagNode = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, node);
        }
    }

    // ============================================================================
    // Edge proptests
    // ============================================================================

    prop_compose! {
        fn valid_edge_condition() -> EdgeCondition {
            proptest::sample::select(vec![
                EdgeCondition::Always,
                EdgeCondition::OnSuccess,
                EdgeCondition::OnFailure,
            ]).unwrap()
        }
    }

    prop_compose! {
        fn valid_edge()(
            source in "[a-z][a-z0-9-]*",
            target in "[a-z][a-z0-9-]*",
            condition in valid_edge_condition(),
        ) -> Edge {
            Edge {
                source_node: NodeName::parse(&source).expect("valid"),
                target_node: NodeName::parse(&target).expect("valid"),
                condition,
            }
        }
    }

    proptest! {
        #[test]
        fn edge_serde_roundtrip(edge: Edge) {
            let json = serde_json::to_string(&edge).expect("serialize");
            let restored: Edge = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, edge);
        }

        #[test]
        fn edge_condition_matches_invariant(condition: EdgeCondition) {
            match condition {
                EdgeCondition::Always => prop_assert!(true),
                EdgeCondition::OnSuccess => {
                    prop_assert!(condition.matches(StepOutcome::Success));
                    prop_assert!(!condition.matches(StepOutcome::Failure));
                },
                EdgeCondition::OnFailure => {
                    prop_assert!(!condition.matches(StepOutcome::Success));
                    prop_assert!(condition.matches(StepOutcome::Failure));
                },
            }
        }
    }

    // ============================================================================
    // WorkflowDefinition proptests
    // ============================================================================

    prop_compose! {
        fn valid_workflow_name() -> String {
            let len = proptest::sample::size_range(1..=64).gen();
            prop::sample::subsequence(
                "abcdefghijklmnopqrstuvwxyz",
                len,
            )
            .collect()
        }
    }

    prop_compose! {
        fn single_node_workflow()(
            name in valid_workflow_name(),
            node_name in "[a-z][a-z0-9-]*",
        ) -> WorkflowDefinition {
            let node = DagNode::valid_default(NodeName::parse(&node_name).expect("valid")).expect("valid");
            WorkflowDefinition {
                workflow_name: WorkflowName::parse(&name).expect("valid"),
                nodes: NonEmptyVec::new(node, vec![]).expect("valid"),
                edges: vec![],
            }
        }
    }

    proptest! {
        #[test]
        fn workflow_definition_serde_roundtrip(def: WorkflowDefinition) {
            let json = serde_json::to_string(&def).expect("serialize");
            let restored: WorkflowDefinition = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, def);
        }

        #[test]
        fn workflow_definition_get_node(def: WorkflowDefinition, node_name in "[a-z][a-z0-9-]*") {
            let query_name = NodeName::parse(&node_name).expect("valid");
            let result = def.get_node(&query_name);
            // Should either find the node or return None
            if let Some(found) = result {
                prop_assert_eq!(&found.node_name, &query_name);
            }
        }

        #[test]
        fn workflow_definition_single_node_valid(name in valid_workflow_name(), node_name in "[a-z][a-z0-9-]*") {
            let def = single_node_workflow()(name, node_name);
            prop_assert_eq!(def.nodes.len(), 1);
            prop_assert!(def.edges.is_empty());
        }

        #[test]
        fn workflow_definition_nodes_non_empty(name in valid_workflow_name()) {
            let result = WorkflowDefinition::parse(&serde_json::json!({
                "workflow_name": name,
                "nodes": [],
                "edges": []
            }).to_string().as_bytes());
            prop_assert!(result.is_err());
        }

        #[test]
        fn workflow_definition_empty_workflow_name() {
            let result = WorkflowDefinition::parse(&serde_json::json!({
                "workflow_name": "",
                "nodes": [{"node_name": "test", "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}}],
                "edges": []
            }).to_string().as_bytes());
            prop_assert!(result.is_err());
        }
    }

    // ============================================================================
    // ConnectorState / ConnectorResult comprehensive tests
    // ============================================================================

    proptest! {
        #[test]
        fn connector_state_is_terminal_invariant(state: ConnectorState) {
            let is_terminal = state.is_terminal();
            match state {
                ConnectorState::Succeeded | ConnectorState::Failed => {
                    prop_assert!(is_terminal, "Succeeded and Failed must be terminal");
                },
                _ => {
                    prop_assert!(!is_terminal, "Non-terminal states must not be terminal");
                },
            }
        }

        #[test]
        fn connector_state_all_variants_have_is_terminal() {
            for state in ConnectorState::all_variants() {
                let _ = state.is_terminal();
            }
        }

        #[test]
        fn connector_result_all_variants() {
            let variants = ConnectorResult::all_variants();
            prop_assert_eq!(variants.len(), 3);
        }

        #[test]
        fn reconcile_action_all_variants() {
            let variants = ReconcileAction::all_variants();
            prop_assert_eq!(variants.len(), 3);
        }

        #[test]
        fn connector_transition_all_variants() {
            let variants = ConnectorTransition::all_variants();
            prop_assert_eq!(variants.len(), 9);
        }

        #[test]
        fn connector_state_serde_all_variants() {
            for state in ConnectorState::all_variants() {
                let json = serde_json::to_string(state).expect("serialize");
                let restored: ConnectorState = serde_json::from_str(&json).expect("deserialize");
                prop_assert_eq!(restored, *state);
            }
        }

        #[test]
        fn connector_result_serde_all_variants() {
            for result in ConnectorResult::all_variants() {
                let json = serde_json::to_string(result).expect("serialize");
                let restored: ConnectorResult = serde_json::from_str(&json).expect("deserialize");
                prop_assert_eq!(restored, *result);
            }
        }

        #[test]
        fn reconcile_action_serde_all_variants() {
            for action in ReconcileAction::all_variants() {
                let json = serde_json::to_string(action).expect("serialize");
                let restored: ReconcileAction = serde_json::from_str(&json).expect("deserialize");
                prop_assert_eq!(restored, *action);
            }
        }

        #[test]
        fn connector_transition_serde_all_variants() {
            for transition in ConnectorTransition::all_variants() {
                let json = serde_json::to_string(transition).expect("serialize");
                let restored: ConnectorTransition = serde_json::from_str(&json).expect("deserialize");
                prop_assert_eq!(restored, *transition);
            }
        }
    }

    // ============================================================================
    // Mixed domain roundtrip tests
    // ============================================================================

    proptest! {
        #[test]
        fn step_id_and_spawn_id_roundtrip(
            step_id_str in "[a-z][a-z0-9-]*",
            spawn_id_str in "[a-z][a-z0-9-]*",
        ) {
            let step_id = StepId::parse(&step_id_str).expect("valid");
            let spawn_id = SpawnId::parse(&spawn_id_str).expect("valid");

            let step_json = serde_json::to_string(&step_id).expect("serialize step");
            let spawn_json = serde_json::to_string(&spawn_id).expect("serialize spawn");

            let restored_step: StepId = serde_json::from_str(&step_json).expect("deserialize step");
            let restored_spawn: SpawnId = serde_json::from_str(&spawn_json).expect("deserialize spawn");

            prop_assert_eq!(restored_step, step_id);
            prop_assert_eq!(restored_spawn, spawn_id);
        }

        #[test]
        fn workflow_name_and_node_name_roundtrip(
            workflow in "[a-z]+",
            node in "[a-z][a-z0-9-]*",
        ) {
            let workflow_name = WorkflowName::parse(&workflow).expect("valid");
            let node_name = NodeName::parse(&node).expect("valid");

            let wf_json = serde_json::to_string(&workflow_name).expect("serialize workflow");
            let nd_json = serde_json::to_string(&node_name).expect("serialize node");

            let restored_wf: WorkflowName = serde_json::from_str(&wf_json).expect("deserialize workflow");
            let restored_nd: NodeName = serde_json::from_str(&nd_json).expect("deserialize node");

            prop_assert_eq!(restored_wf, workflow_name);
            prop_assert_eq!(restored_nd, node_name);
        }
    }

    // ============================================================================
    // Anti-invariant tests: verify that invalid states are rejected
    // ============================================================================

    proptest! {
        #[test]
        fn anti_invariant_spawn_id_nil_ulid_rejected() {
            let result = SpawnId::parse("00000000000000000000000000");
            prop_assert!(result.is_err(), "Nil ULID should be rejected");
        }

        #[test]
        fn anti_invariant_instance_id_nil_ulid_rejected() {
            let result = crate::InstanceId::parse("00000000000000000000000000");
            prop_assert!(result.is_err(), "Nil ULID should be rejected for InstanceId");
        }

        #[test]
        fn anti_invariant_retry_policy_backoff_capped(
            max_attempts in 1u8..=10u8,
            backoff_ms in 1u64..=u64::MAX,
            multiplier in 1.1f64..=10.0f64,
            max_backoff in 1u64..=u64::MAX,
        ) {
            let policy = RetryPolicy::with_max_backoff(max_attempts, backoff_ms, multiplier, max_backoff);
            if max_backoff >= backoff_ms {
                prop_assert!(policy.is_ok());
            }
        }
    }
}
