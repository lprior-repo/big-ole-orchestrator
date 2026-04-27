#![cfg(feature = "proptest")]

use crate::string_types::{
    BinaryHash, IdempotencyKey, InstanceId, NodeName, SpawnId, StepId, TimerId, WorkflowName,
};
use crate::{
    CompensationPolicy, ConnectorResult, ConnectorState, ConnectorTransition, ConnectorTransitionError,
    DagNode, Edge, EdgeCondition, EffectIntent, EffectKind, EffectTransitionEvent, NonEmptyVec,
    ParseError, ReconcileAction, RetryPolicy, StepOutcome, WorkflowDefinition,
};
use proptest::prelude::*;
use std::num::NonZeroU64;

impl Arbitrary for InstanceId {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let valid_ulid_chars = "[0-7][0-9A-HJKMNP-TV-Z]";
        let ulid_pattern = format!("{}{{{}}}", valid_ulid_chars, 26);
        prop::string::string_regex(&ulid_pattern)
            .unwrap()
            .prop_map(|s| InstanceId::parse(&s).expect("valid ULID in range"))
            .boxed()
    }
}

impl Arbitrary for WorkflowName {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let valid_name = "[a-zA-Z0-9][a-zA-Z0-9_-]*[a-zA-Z0-9]|[a-zA-Z0-9]";
        prop::string::string_regex(valid_name)
            .unwrap()
            .prop_filter("no consecutive separators", |s| {
                !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-")
            })
            .prop_map(|s| WorkflowName::parse(&s).expect("valid WorkflowName"))
            .boxed()
    }
}

impl Arbitrary for NodeName {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let valid_name = "[a-zA-Z0-9][a-zA-Z0-9_-]*[a-zA-Z0-9]|[a-zA-Z0-9]";
        prop::string::string_regex(valid_name)
            .unwrap()
            .prop_filter("no consecutive separators", |s| {
                !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-")
            })
            .prop_map(|s| NodeName::parse(&s).expect("valid NodeName"))
            .boxed()
    }
}

impl Arbitrary for BinaryHash {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let byte_len = 4u32..128u32;
        byte_len.prop_map(|len| {
            let hex_len = (len * 2) as usize;
            let s: String = "0123456789abcdef"
                .chars()
                .cycle()
                .take(hex_len)
                .collect();
            BinaryHash::parse(&s).expect("valid hex in range")
        }).boxed()
    }
}

impl Arbitrary for TimerId {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        "\\S{1,256}".prop_map(|s| TimerId::parse(&s).expect("valid TimerId"))
            .boxed()
    }
}

impl Arbitrary for IdempotencyKey {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let valid_key = "[a-zA-Z0-9_-]{1,1024}";
        prop::string::string_regex(valid_key)
            .unwrap()
            .prop_map(|s| IdempotencyKey::parse(&s).expect("valid IdempotencyKey"))
            .boxed()
    }
}

impl Arbitrary for SpawnId {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let valid_id = "[a-zA-Z0-9][a-zA-Z0-9_-]*[a-zA-Z0-9]|[a-zA-Z0-9]";
        prop::string::string_regex(valid_id)
            .unwrap()
            .prop_map(|s| SpawnId::parse(&s).expect("valid SpawnId"))
            .boxed()
    }
}

impl Arbitrary for StepId {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let valid_id = "[a-zA-Z0-9]([a-zA-Z0-9_-]*[a-zA-Z0-9])?";
        prop::string::string_regex(valid_id)
            .unwrap()
            .prop_map(|s| StepId::parse(&s).expect("valid StepId"))
            .boxed()
    }
}

impl Arbitrary for ConnectorState {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(Self::all_variants().to_vec()).boxed()
    }
}

impl Arbitrary for ConnectorResult {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(Self::all_variants().to_vec()).boxed()
    }
}

impl Arbitrary for ReconcileAction {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(Self::all_variants().to_vec()).boxed()
    }
}

impl Arbitrary for ConnectorTransition {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(Self::all_variants().to_vec()).boxed()
    }
}

impl Arbitrary for ConnectorTransitionError {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![
            ConnectorTransitionError::TerminalStateTransition,
            ConnectorTransitionError::InvalidTransition,
        ]).boxed()
    }
}

impl Arbitrary for StepOutcome {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![StepOutcome::Success, StepOutcome::Failure]).boxed()
    }
}

impl Arbitrary for EdgeCondition {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![
            EdgeCondition::Always,
            EdgeCondition::OnSuccess,
            EdgeCondition::OnFailure,
        ]).boxed()
    }
}

impl Arbitrary for RetryPolicy {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        (1u8..=255u8, 0u64..=u64::MAX, 1.0f64..=10.0f64, 0u64..=u64::MAX)
            .prop_map(|(max_attempts, backoff_ms, backoff_multiplier, max_backoff_ms)| {
                RetryPolicy::with_max_backoff(max_attempts, backoff_ms, backoff_multiplier, max_backoff_ms)
                    .expect("valid retry policy")
            })
            .boxed()
    }
}

impl Arbitrary for DagNode {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<(NodeName, RetryPolicy)>()
            .prop_map(|(node_name, retry_policy)| DagNode {
                node_name,
                retry_policy,
                compensation_policy: None,
            })
            .boxed()
    }
}

impl Arbitrary for Edge {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<(NodeName, NodeName, EdgeCondition)>()
            .prop_map(|(source_node, target_node, condition)| Edge {
                source_node,
                target_node,
                condition,
            })
            .boxed()
    }
}

impl Arbitrary for WorkflowDefinition {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        let node_count = 1usize..=5usize;
        let edge_count = 0usize..=10usize;
        (any::<WorkflowName>(), node_count, edge_count)
            .prop_map(|(workflow_name, node_count, edge_count)| {
                let nodes: Vec<DagNode> = (0..node_count)
                    .map(|i| DagNode {
                        node_name: NodeName::parse(&format!("node{}", i)).expect("valid"),
                        retry_policy: RetryPolicy::new(1, 0, 1.0).expect("valid"),
                        compensation_policy: None,
                    })
                    .collect();

                let mut edges = Vec::new();
                for i in 0..node_count {
                    if i > 0 && edge_count > edges.len() {
                        edges.push(Edge {
                            source_node: NodeName::parse(&format!("node{}", i)).expect("valid"),
                            target_node: NodeName::parse(&format!("node{}", i - 1)).expect("valid"),
                            condition: EdgeCondition::OnSuccess,
                        });
                    }
                }

                WorkflowDefinition {
                    workflow_name,
                    nodes: NonEmptyVec::new(nodes).expect("non-empty"),
                    edges,
                }
            })
            .boxed()
    }
}

impl Arbitrary for EffectIntent {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![
            EffectIntent::Prepared,
            EffectIntent::Committed,
            EffectIntent::RolledBack,
        ]).boxed()
    }
}

impl Arbitrary for EffectKind {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ]).boxed()
    }
}

impl Arbitrary for CompensationPolicy {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![
            CompensationPolicy::None,
            CompensationPolicy::Manual,
            CompensationPolicy::Automatic,
        ]).boxed()
    }
}

impl Arbitrary for EffectTransitionEvent {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop::sample::select(vec![
            EffectTransitionEvent::Commit,
            EffectTransitionEvent::Rollback,
        ]).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::apply_connector_transition;

    proptest! {
        #[test]
        fn instance_id_serde_roundtrip(id in any::<InstanceId>()) {
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: InstanceId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn workflow_name_serde_roundtrip(name in any::<WorkflowName>()) {
            let json = serde_json::to_string(&name).expect("serialize");
            let restored: WorkflowName = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, name);
        }

        #[test]
        fn node_name_serde_roundtrip(name in any::<NodeName>()) {
            let json = serde_json::to_string(&name).expect("serialize");
            let restored: NodeName = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, name);
        }

        #[test]
        fn binary_hash_serde_roundtrip(hash in any::<BinaryHash>()) {
            let json = serde_json::to_string(&hash).expect("serialize");
            let restored: BinaryHash = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, hash);
        }

        #[test]
        fn timer_id_serde_roundtrip(id in any::<TimerId>()) {
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: TimerId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn idempotency_key_serde_roundtrip(key in any::<IdempotencyKey>()) {
            let json = serde_json::to_string(&key).expect("serialize");
            let restored: IdempotencyKey = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, key);
        }

        #[test]
        fn spawn_id_serde_roundtrip(id in any::<SpawnId>()) {
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: SpawnId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn step_id_serde_roundtrip(id in any::<StepId>()) {
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: StepId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn connector_state_serde_roundtrip(state in any::<ConnectorState>()) {
            let json = serde_json::to_string(&state).expect("serialize");
            let restored: ConnectorState = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, state);
        }

        #[test]
        fn connector_result_serde_roundtrip(result in any::<ConnectorResult>()) {
            let json = serde_json::to_string(&result).expect("serialize");
            let restored: ConnectorResult = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, result);
        }

        #[test]
        fn reconcile_action_serde_roundtrip(action in any::<ReconcileAction>()) {
            let json = serde_json::to_string(&action).expect("serialize");
            let restored: ReconcileAction = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, action);
        }

        #[test]
        fn connector_transition_serde_roundtrip(transition in any::<ConnectorTransition>()) {
            let json = serde_json::to_string(&transition).expect("serialize");
            let restored: ConnectorTransition = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, transition);
        }

        #[test]
        fn connector_transition_error_serde_roundtrip(error in any::<ConnectorTransitionError>()) {
            let json = serde_json::to_string(&error).expect("serialize");
            let restored: ConnectorTransitionError = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, error);
        }

        #[test]
        fn step_outcome_serde_roundtrip(outcome in any::<StepOutcome>()) {
            let json = serde_json::to_string(&outcome).expect("serialize");
            let restored: StepOutcome = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, outcome);
        }

        #[test]
        fn edge_condition_serde_roundtrip(condition in any::<EdgeCondition>()) {
            let json = serde_json::to_string(&condition).expect("serialize");
            let restored: EdgeCondition = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, condition);
        }

        #[test]
        fn retry_policy_serde_roundtrip(policy in any::<RetryPolicy>()) {
            let json = serde_json::to_string(&policy).expect("serialize");
            let restored: RetryPolicy = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, policy);
        }

        #[test]
        fn dag_node_serde_roundtrip(node in any::<DagNode>()) {
            let json = serde_json::to_string(&node).expect("serialize");
            let restored: DagNode = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, node);
        }

        #[test]
        fn edge_serde_roundtrip(edge in any::<Edge>()) {
            let json = serde_json::to_string(&edge).expect("serialize");
            let restored: Edge = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, edge);
        }

        #[test]
        fn workflow_definition_serde_roundtrip(def in any::<WorkflowDefinition>()) {
            let json = serde_json::to_string(&def).expect("serialize");
            let restored: WorkflowDefinition = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, def);
        }

        #[test]
        fn effect_intent_serde_roundtrip(intent in any::<EffectIntent>()) {
            let json = serde_json::to_string(&intent).expect("serialize");
            let restored: EffectIntent = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, intent);
        }

        #[test]
        fn effect_kind_serde_roundtrip(kind in any::<EffectKind>()) {
            let json = serde_json::to_string(&kind).expect("serialize");
            let restored: EffectKind = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, kind);
        }

        #[test]
        fn compensation_policy_serde_roundtrip(policy in any::<CompensationPolicy>()) {
            let json = serde_json::to_string(&policy).expect("serialize");
            let restored: CompensationPolicy = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, policy);
        }

        #[test]
        fn effect_transition_event_serde_roundtrip(event in any::<EffectTransitionEvent>()) {
            let json = serde_json::to_string(&event).expect("serialize");
            let restored: EffectTransitionEvent = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, event);
        }
    }

    proptest! {
        #[test]
        fn instance_id_display_parse_roundtrip(id in any::<InstanceId>()) {
            let display = id.to_string();
            let parsed = InstanceId::parse(&display).expect("valid ULID");
            prop_assert_eq!(parsed, id);
        }

        #[test]
        fn workflow_name_display_parse_roundtrip(name in any::<WorkflowName>()) {
            let display = name.to_string();
            let parsed = WorkflowName::parse(&display).expect("valid WorkflowName");
            prop_assert_eq!(parsed, name);
        }

        #[test]
        fn node_name_display_parse_roundtrip(name in any::<NodeName>()) {
            let display = name.to_string();
            let parsed = NodeName::parse(&display).expect("valid NodeName");
            prop_assert_eq!(parsed, name);
        }

        #[test]
        fn binary_hash_display_parse_roundtrip(hash in any::<BinaryHash>()) {
            let display = hash.to_string();
            let parsed = BinaryHash::parse(&display).expect("valid BinaryHash");
            prop_assert_eq!(parsed, hash);
        }

        #[test]
        fn timer_id_display_parse_roundtrip(id in any::<TimerId>()) {
            let display = id.to_string();
            let parsed = TimerId::parse(&display).expect("valid TimerId");
            prop_assert_eq!(parsed, id);
        }

        #[test]
        fn idempotency_key_display_parse_roundtrip(key in any::<IdempotencyKey>()) {
            let display = key.to_string();
            let parsed = IdempotencyKey::parse(&display).expect("valid IdempotencyKey");
            prop_assert_eq!(parsed, key);
        }

        #[test]
        fn spawn_id_display_parse_roundtrip(id in any::<SpawnId>()) {
            let display = id.to_string();
            let parsed = SpawnId::parse(&display).expect("valid SpawnId");
            prop_assert_eq!(parsed, id);
        }

        #[test]
        fn step_id_display_parse_roundtrip(id in any::<StepId>()) {
            let display = id.to_string();
            let parsed = StepId::parse(&display).expect("valid StepId");
            prop_assert_eq!(parsed, id);
        }
    }

    proptest! {
        #[test]
        fn connector_transition_never_panics(state in any::<ConnectorState>(), transition in any::<ConnectorTransition>()) {
            let _ = apply_connector_transition(state, transition);
        }

        #[test]
        fn terminal_states_reject_all_transitions(
            state in prop::sample::select(vec![ConnectorState::Succeeded, ConnectorState::Failed]),
            transition in any::<ConnectorTransition>()
        ) {
            let result = apply_connector_transition(state, transition);
            prop_assert!(result.is_err());
        }

        #[test]
        fn ambiguous_state_only_accepts_reconcile_events(event in any::<ConnectorTransition>()) {
            let result = apply_connector_transition(ConnectorState::Ambiguous, event);
            match event {
                ConnectorTransition::ReconcileSucceeded
                | ConnectorTransition::ReconcileFailed
                | ConnectorTransition::ReconcileRetry => {
                    prop_assert!(result.is_ok());
                }
                _ => {
                    prop_assert!(result.is_err());
                }
            }
        }
    }

    proptest! {
        #[test]
        fn instance_id_max_ulid_roundtrip() {
            let max_ulid = "7ZZZZZZZZZZZZZZZZZZZZZZZZ";
            let id = InstanceId::parse(max_ulid).expect("valid max ULID");
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: InstanceId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn workflow_name_max_length_roundtrip() {
            let max_name = "a".repeat(128);
            let name = WorkflowName::parse(&max_name).expect("valid max length");
            let json = serde_json::to_string(&name).expect("serialize");
            let restored: WorkflowName = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, name);
        }

        #[test]
        fn node_name_max_length_roundtrip() {
            let max_name = "a".repeat(128);
            let name = NodeName::parse(&max_name).expect("valid max length");
            let json = serde_json::to_string(&name).expect("serialize");
            let restored: NodeName = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, name);
        }

        #[test]
        fn idempotency_key_max_length_roundtrip() {
            let max_key = "a".repeat(1024);
            let key = IdempotencyKey::parse(&max_key).expect("valid max length");
            let json = serde_json::to_string(&key).expect("serialize");
            let restored: IdempotencyKey = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, key);
        }

        #[test]
        fn timer_id_max_length_roundtrip() {
            let max_id = "a".repeat(256);
            let id = TimerId::parse(&max_id).expect("valid max length");
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: TimerId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn timer_id_unicode_roundtrip() {
            let unicode_id = "timer-\u{00e9}\u{00f1}-\u{4e2d}\u{6587}";
            let id = TimerId::parse(unicode_id).expect("valid unicode");
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: TimerId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }
    }

    proptest! {
        #[test]
        fn instance_id_min_valid_roundtrip() {
            let min_ulid = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
            let id = InstanceId::parse(min_ulid).expect("valid min ULID");
            let json = serde_json::to_string(&id).expect("serialize");
            let restored: InstanceId = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, id);
        }

        #[test]
        fn binary_hash_min_length_roundtrip() {
            let min_hash = "01234567";
            let hash = BinaryHash::parse(min_hash).expect("valid min length");
            let json = serde_json::to_string(&hash).expect("serialize");
            let restored: BinaryHash = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, hash);
        }

        #[test]
        fn binary_hash_large_roundtrip() {
            let large_hash: String = "a".repeat(256);
            let hash = BinaryHash::parse(&large_hash).expect("valid large hash");
            let json = serde_json::to_string(&hash).expect("serialize");
            let restored: BinaryHash = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(restored, hash);
        }
    }
}
