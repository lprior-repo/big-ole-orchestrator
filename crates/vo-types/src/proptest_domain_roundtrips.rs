#![cfg(feature = "proptest")]

use crate::{
    CompensationPolicy, ConnectorResult, ConnectorState, ConnectorTransition, EffectIntent, EffectKind,
    EventEnvelope, ExternalReceipt, IdempotencyKey, InstanceId, NodeName, ReconcileAction,
    SpawnId, StepId, StepOutcome, TimerId, WorkflowName, BinaryHash, EffectRecord,
};
use crate::events::{EventMetadata, RoutingProjection};
use crate::events::payload::EventPayload;
use crate::workflow::{DagNode, Edge, EdgeCondition, RetryPolicy};
use proptest::prelude::*;

fn valid_ulid() -> impl Strategy<Value = String> {
    use ulid::Ulid;
    any::<[u8; 16]>().prop_map(|bytes| {
        Ulid::from_bytes(bytes).to_string()
    })
}

fn valid_workflow_name() -> impl Strategy<Value = String> {
    r"[a-z][a-z0-9_-]{0,126}".prop_filter("cannot have double hyphen or underscore", |s| {
        !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-")
    })
}

fn valid_node_name() -> impl Strategy<Value = String> {
    r"[a-z][a-z0-9_-]{0,127}".prop_filter("cannot have double hyphen or underscore", |s| {
        !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-")
    })
}

fn valid_binary_hash() -> impl Strategy<Value = String> {
    r"[a-f0-9]{8,64}".prop_map(|s| {
        if s.len() % 2 != 0 {
            format!("{}0", s)
        } else {
            s
        }
    })
}

fn valid_id_or_timer_id() -> impl Strategy<Value = String> {
    r"[a-zA-Z0-9_-]{1,255}"
}

fn valid_idempotency_key() -> impl Strategy<Value = String> {
    r"[a-zA-Z0-9_-]{1,1024}".prop_filter("must not be empty", |s| !s.is_empty())
}

proptest! {
    // ========================================================================
    // InstanceId Tests
    // ========================================================================

    #[test]
    fn instance_id_ulid_roundtrip_serde(ulid_str in valid_ulid()) {
        let id = InstanceId::parse(&ulid_str).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let restored: InstanceId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, id);
    }

    #[test]
    fn instance_id_display_roundtrip(ulid_str in valid_ulid()) {
        let id = InstanceId::parse(&ulid_str).unwrap();
        let displayed = id.to_string();
        let restored = InstanceId::parse(&displayed).unwrap();
        prop_assert_eq!(restored, id);
    }

    #[test]
    fn instance_id_rejects_empty() {
        let result = InstanceId::parse("");
        prop_assert!(result.is_err());
    }

    #[test]
    fn instance_id_rejects_wrong_length(s in "[a-zA-Z0-9]{1,50}".prop_filter("not 26 chars", |s| s.len() != 26)) {
        let result = InstanceId::parse(&s);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // WorkflowName Tests
    // ========================================================================

    #[test]
    fn workflow_name_roundtrip_serde(name in valid_workflow_name()) {
        let wn = WorkflowName::parse(&name).unwrap();
        let json = serde_json::to_string(&wn).unwrap();
        let restored: WorkflowName = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, wn);
    }

    #[test]
    fn workflow_name_rejects_empty() {
        let result = WorkflowName::parse("");
        prop_assert!(result.is_err());
    }

    #[test]
    fn workflow_name_rejects_consecutive_hyphens(name in "[a-z]{1,10}".prop_map(|s| format!("{}--test", s))) {
        let result = WorkflowName::parse(&name);
        prop_assert!(result.is_err());
    }

    #[test]
    fn workflow_name_rejects_uppercase(name in "[A-Z][a-z0-9_-]{0,10}") {
        let result = WorkflowName::parse(&name);
        prop_assert!(result.is_err());
    }

    #[test]
    fn workflow_name_max_length_edge_case() {
        let max_len_name = "a".repeat(128);
        let result = WorkflowName::parse(&max_len_name);
        prop_assert!(result.is_ok(), "128 char name should be valid");
    }

    #[test]
    fn workflow_name_exceeds_max_length() {
        let too_long_name = "a".repeat(129);
        let result = WorkflowName::parse(&too_long_name);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // NodeName Tests
    // ========================================================================

    #[test]
    fn node_name_roundtrip_serde(name in valid_node_name()) {
        let nn = NodeName::parse(&name).unwrap();
        let json = serde_json::to_string(&nn).unwrap();
        let restored: NodeName = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, nn);
    }

    #[test]
    fn node_name_rejects_empty() {
        let result = NodeName::parse("");
        prop_assert!(result.is_err());
    }

    #[test]
    fn node_name_rejects_consecutive_underscores(name in "[a-z]{1,10}".prop_map(|s| format!("{}__test", s))) {
        let result = NodeName::parse(&name);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // BinaryHash Tests
    // ========================================================================

    #[test]
    fn binary_hash_roundtrip_serde(hash in valid_binary_hash()) {
        let bh = BinaryHash::parse(&hash).unwrap();
        let json = serde_json::to_string(&bh).unwrap();
        let restored: BinaryHash = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, bh);
    }

    #[test]
    fn binary_hash_rejects_empty() {
        let result = BinaryHash::parse("");
        prop_assert!(result.is_err());
    }

    #[test]
    fn binary_hash_rejects_odd_length(s in proptest::collection::vec("[a-f0-9]", 1..20).prop_map(|v| v.join(""))) {
        prop_assume!(s.len() % 2 != 0);
        let result = BinaryHash::parse(&s);
        prop_assert!(result.is_err());
    }

    #[test]
    fn binary_hash_rejects_uppercase(s in "[A-F0-9]{16}") {
        let result = BinaryHash::parse(&s);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // TimerId Tests
    // ========================================================================

    #[test]
    fn timer_id_roundtrip_serde(id in valid_id_or_timer_id()) {
        let tid = TimerId::parse(&id).unwrap();
        let json = serde_json::to_string(&tid).unwrap();
        let restored: TimerId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, tid);
    }

    #[test]
    fn timer_id_rejects_empty() {
        let result = TimerId::parse("");
        prop_assert!(result.is_err());
    }

    #[test]
    fn timer_id_max_length_edge_case() {
        let max_len = "a".repeat(256);
        let result = TimerId::parse(&max_len);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn timer_id_exceeds_max_length() {
        let too_long = "a".repeat(257);
        let result = TimerId::parse(&too_long);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // IdempotencyKey Tests
    // ========================================================================

    #[test]
    fn idempotency_key_roundtrip_serde(key in valid_idempotency_key()) {
        let ik = IdempotencyKey::parse(&key).unwrap();
        let json = serde_json::to_string(&ik).unwrap();
        let restored: IdempotencyKey = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, ik);
    }

    #[test]
    fn idempotency_key_rejects_empty() {
        let result = IdempotencyKey::parse("");
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // SpawnId Tests
    // ========================================================================

    #[test]
    fn spawn_id_roundtrip_serde(id in valid_id_or_timer_id()) {
        let sid = SpawnId::parse(&id).unwrap();
        let json = serde_json::to_string(&sid).unwrap();
        let restored: SpawnId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, sid);
    }

    #[test]
    fn spawn_id_rejects_empty() {
        let result = SpawnId::parse("");
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // StepId Tests
    // ========================================================================

    #[test]
    fn step_id_roundtrip_serde(id in valid_id_or_timer_id()) {
        let sid = StepId::parse(&id).unwrap();
        let json = serde_json::to_string(&sid).unwrap();
        let restored: StepId = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, sid);
    }

    #[test]
    fn step_id_rejects_empty() {
        let result = StepId::parse("");
        prop_assert!(result.is_err());
    }

    #[test]
    fn step_id_rejects_underscore_start(name in "_[a-z][a-z0-9_-]*") {
        let result = StepId::parse(&name);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // StepOutcome Tests
    // ========================================================================

    #[test]
    fn step_outcome_serde_roundtrip(outcome in any::<StepOutcome>()) {
        let json = serde_json::to_string(&outcome).unwrap();
        let restored: StepOutcome = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, outcome);
    }

    // ========================================================================
    // EdgeCondition Tests
    // ========================================================================

    #[test]
    fn edge_condition_serde_roundtrip(condition in any::<EdgeCondition>()) {
        let json = serde_json::to_string(&condition).unwrap();
        let restored: EdgeCondition = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, condition);
    }

    // ========================================================================
    // CompensationPolicy Tests
    // ========================================================================

    #[test]
    fn compensation_policy_serde_roundtrip(policy in any::<CompensationPolicy>()) {
        let json = serde_json::to_string(&policy).unwrap();
        let restored: CompensationPolicy = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, policy);
    }

    // ========================================================================
    // ConnectorState Tests
    // ========================================================================

    #[test]
    fn connector_state_serde_roundtrip(state in any::<ConnectorState>()) {
        let json = serde_json::to_string(&state).unwrap();
        let restored: ConnectorState = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, state);
    }

    // ========================================================================
    // ConnectorResult Tests
    // ========================================================================

    #[test]
    fn connector_result_serde_roundtrip(result in any::<ConnectorResult>()) {
        let json = serde_json::to_string(&result).unwrap();
        let restored: ConnectorResult = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, result);
    }

    // ========================================================================
    // ReconcileAction Tests
    // ========================================================================

    #[test]
    fn reconcile_action_serde_roundtrip(action in any::<ReconcileAction>()) {
        let json = serde_json::to_string(&action).unwrap();
        let restored: ReconcileAction = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, action);
    }

    // ========================================================================
    // EffectIntent Tests
    // ========================================================================

    #[test]
    fn effect_intent_serde_roundtrip(intent in any::<EffectIntent>()) {
        let json = serde_json::to_string(&intent).unwrap();
        let restored: EffectIntent = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, intent);
    }

    // ========================================================================
    // EffectKind Tests
    // ========================================================================

    #[test]
    fn effect_kind_serde_roundtrip(kind in any::<EffectKind>()) {
        let json = serde_json::to_string(&kind).unwrap();
        let restored: EffectKind = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, kind);
    }

    // ========================================================================
    // RoutingProjection Tests
    // ========================================================================

    #[test]
    fn routing_projection_serde_roundtrip() {
        let rp = RoutingProjection {};
        let json = serde_json::to_string(&rp).unwrap();
        let restored: RoutingProjection = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, rp);
    }

    // ========================================================================
    // EventPayload Edge Cases
    // ========================================================================

    #[test]
    fn event_payload_workflow_started_unicode(workflow_id in "[a-z]{1,20}", hash in "[a-f0-9]{16}") {
        let payload = EventPayload::WorkflowStarted {
            workflow_id: workflow_id.clone(),
            dag_topology: serde_json::json!({"nodes": [], "edges": []}),
            binary_hash: hash.to_string(),
            workflow_version_hash: hash.to_string(),
            dedupe_key_hash: Some(format!("{} with unicode émoji 🎉", workflow_id)),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    #[test]
    fn event_payload_workflow_started_empty_dedupe(workflow_id in "[a-z]{1,20}", hash in "[a-f0-9]{16}") {
        let payload = EventPayload::WorkflowStarted {
            workflow_id: workflow_id.clone(),
            dag_topology: serde_json::json!({}),
            binary_hash: hash.to_string(),
            workflow_version_hash: hash.to_string(),
            dedupe_key_hash: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    #[test]
    fn event_payload_large_output(workflow_id in "[a-z]{1,20}", step_id in "[a-z]{1,20}") {
        let large_obj = serde_json::json!({
            "data": "x".repeat(10000),
            "nested": {"a": [1, 2, 3]},
            "unicode": "héllo wörld 😀"
        });
        let payload = EventPayload::StepCompleted {
            workflow_id,
            step_id,
            completed_at_ms: 1_700_000_000_000,
            attempt: 1,
            fence: 42,
            routing_projection: None,
            output_ref: None,
            output_hash: None,
            output: large_obj,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    #[test]
    fn event_payload_timer_set_and_fired(workflow_id in "[a-z]{1,20}", timer_id in "[a-z0-9_-]{1,50}") {
        let timer_set = EventPayload::TimerSet {
            workflow_id: workflow_id.clone(),
            timer_id: timer_id.clone(),
            fire_at_ms: 1_700_000_000_000,
        };
        let json_set = serde_json::to_string(&timer_set).unwrap();
        let restored_set: EventPayload = serde_json::from_str(&json_set).unwrap();
        prop_assert_eq!(restored_set, timer_set);

        let timer_fired = EventPayload::TimerFired {
            workflow_id,
            timer_id,
            fired_at_ms: 1_700_000_001_000,
        };
        let json_fired = serde_json::to_string(&timer_fired).unwrap();
        let restored_fired: EventPayload = serde_json::from_str(&json_fired).unwrap();
        prop_assert_eq!(restored_fired, timer_fired);
    }

    #[test]
    fn event_payload_effect_prepared_and_committed(
        workflow_id in "[a-z]{1,20}",
        step_id in "[a-z]{1,20}",
        effect_id in "[a-z0-9_-]{1,50}",
        sink_kind in "[a-z]{1,20}",
        payload_hash in "[a-f0-9]{16}",
    ) {
        let prepared = EventPayload::EffectPrepared {
            workflow_id: workflow_id.clone(),
            step_id: step_id.clone(),
            effect_id: effect_id.clone(),
            sink_kind: sink_kind.to_string(),
            payload_hash: payload_hash.to_string(),
            fence: 100,
        };
        let json_prepared = serde_json::to_string(&prepared).unwrap();
        let restored_prepared: EventPayload = serde_json::from_str(&json_prepared).unwrap();
        prop_assert_eq!(restored_prepared, prepared);

        let receipt_payload = serde_json::json!({"receipt": "data", "timestamp": 42});
        let external_receipt = ExternalReceipt::new(
            "connector-1".to_string(),
            "1.0.0".to_string(),
            EffectKind::HttpCall,
            receipt_payload,
        ).unwrap();

        let committed = EventPayload::EffectCommitted {
            workflow_id,
            step_id,
            effect_id,
            external_receipt,
            fence: 100,
        };
        let json_committed = serde_json::to_string(&committed).unwrap();
        let restored_committed: EventPayload = serde_json::from_str(&json_committed).unwrap();
        prop_assert_eq!(restored_committed, committed);
    }

    #[test]
    fn event_payload_workflow_cancelled(workflow_id in "[a-z]{1,20}", cancelled_by in "[a-z_-]{1,50}") {
        let payload = EventPayload::WorkflowCancelled {
            workflow_id,
            cancelled_by,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    #[test]
    fn event_payload_instance_resumed(workflow_id in "[a-z]{1,20}") {
        let payload = EventPayload::InstanceResumed {
            workflow_id,
            resumed_at_ms: 1_700_000_000_000,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    #[test]
    fn event_payload_continued_as_new(
        workflow_id in "[a-z]{1,20}",
        lineage_id in "[a-z0-9_-]{1,50}",
    ) {
        let payload = EventPayload::ContinuedAsNew {
            workflow_id,
            lineage_id,
            old_epoch: 1,
            new_epoch: 2,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    #[test]
    fn event_payload_workflow_quarantined(workflow_id in "[a-z]{1,20}") {
        let payload = EventPayload::WorkflowQuarantined {
            workflow_id,
            failure_count: 5,
            failure_window_seconds: 300,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EventPayload = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, payload);
    }

    // ========================================================================
    // EventEnvelope Tests
    // ========================================================================

    #[test]
    fn event_envelope_roundtrip(
        instance_id in valid_ulid(),
        sequence in 1u64..,
        timestamp_ms in 0u64..,
        payload in any::<serde_json::Value>(),
    ) {
        let envelope = EventEnvelope {
            schema_version: 1,
            instance_id,
            sequence,
            timestamp_ms,
            payload,
            metadata: EventMetadata::default(),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let restored: EventEnvelope = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, envelope);
    }

    #[test]
    fn event_envelope_with_metadata_roundtrip(
        instance_id in valid_ulid(),
        sequence in 1u64..,
        timestamp_ms in 0u64..,
    ) {
        let metadata = EventMetadata {
            command_metadata: None,
            annotations: serde_json::json!({
                "key1": "value1",
                "key2": 42,
                "nested": {"a": "b"}
            }),
        };
        let envelope = EventEnvelope {
            schema_version: 1,
            instance_id,
            sequence,
            timestamp_ms,
            payload: serde_json::json!({"test": "payload"}),
            metadata,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let restored: EventEnvelope = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, envelope);
    }

    // ========================================================================
    // EventMetadata Tests
    // ========================================================================

    #[test]
    fn event_metadata_roundtrip() {
        let metadata = EventMetadata {
            command_metadata: None,
            annotations: serde_json::json!({"foo": "bar"}),
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let restored: EventMetadata = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, metadata);
    }

    // ========================================================================
    // RetryPolicy Tests
    // ========================================================================

    #[test]
    fn retry_policy_serde_roundtrip(
        max_attempts in 1u8..=255,
        backoff_ms in 0u64..1_000_000,
        multiplier in 1.0f64..10.0,
    ) {
        let policy = RetryPolicy::new(max_attempts, backoff_ms, multiplier).unwrap();
        let json = serde_json::to_string(&policy).unwrap();
        let restored: RetryPolicy = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored.max_attempts, policy.max_attempts);
        prop_assert_eq!(restored.backoff_ms, policy.backoff_ms);
        prop_assert!((restored.backoff_multiplier - policy.backoff_multiplier).abs() < 0.0001);
    }

    #[test]
    fn retry_policy_rejects_zero_attempts() {
        let result = RetryPolicy::new(0, 100, 2.0);
        prop_assert!(result.is_err());
    }

    #[test]
    fn retry_policy_rejects_multiplier_less_than_one() {
        let result = RetryPolicy::new(3, 100, 0.5);
        prop_assert!(result.is_err());
    }

    #[test]
    fn retry_policy_rejects_nan_multiplier() {
        let result = RetryPolicy::new(3, 100, f64::NAN);
        prop_assert!(result.is_err());
    }

    // ========================================================================
    // EdgeCondition.matches() Invariant Tests
    // ========================================================================

    #[test]
    fn edge_condition_matches_success() {
        prop_assert!(EdgeCondition::OnSuccess.matches(StepOutcome::Success));
        prop_assert!(!EdgeCondition::OnSuccess.matches(StepOutcome::Failure));
    }

    #[test]
    fn edge_condition_matches_failure() {
        prop_assert!(EdgeCondition::OnFailure.matches(StepOutcome::Failure));
        prop_assert!(!EdgeCondition::OnFailure.matches(StepOutcome::Success));
    }

    #[test]
    fn edge_condition_always_matches() {
        prop_assert!(EdgeCondition::Always.matches(StepOutcome::Success));
        prop_assert!(EdgeCondition::Always.matches(StepOutcome::Failure));
    }

    // ========================================================================
    // StepOutcome Invariant Tests
    // ========================================================================

    #[test]
    fn step_outcome_is_binary() {
        let outcome = any::<StepOutcome>();
        prop_assert!(outcome == StepOutcome::Success || outcome == StepOutcome::Failure);
    }

    // ========================================================================
    // ConnectorState Invariant Tests
    // ========================================================================

    #[test]
    fn connector_state_is_terminal_for_succeeded_and_failed(state in any::<ConnectorState>()) {
        match state {
            ConnectorState::Succeeded | ConnectorState::Failed => {
                prop_assert!(state.is_terminal());
            }
            _ => {
                prop_assert!(!state.is_terminal());
            }
        }
    }

    #[test]
    fn connector_state_all_variants_length() {
        let variants = ConnectorState::all_variants();
        prop_assert_eq!(variants.len(), 7);
    }

    // ========================================================================
    // EffectRecord Tests
    // ========================================================================

    #[test]
    fn effect_record_roundtrip(
        id in "[a-zA-Z0-9_-]{1,100}",
        kind_idx in 0usize..3,
        status_idx in 0usize..3,
    ) {
        let kinds = [EffectKind::HttpCall, EffectKind::SqlQuery, EffectKind::BlobWrite];
        let statuses = [EffectIntent::Prepared, EffectIntent::Committed, EffectIntent::RolledBack];
        let kind = kinds[kind_idx];
        let status = statuses[status_idx];
        let params = serde_json::json!({"key": "value"});
        let ts = crate::TimestampMs(42);

        let record = EffectRecord::new(id.clone(), kind, params.clone(), status, Some(ts));
        prop_assert!(record.is_some());
        let r = record.unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let restored: EffectRecord = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored.intent_id(), r.intent_id());
        prop_assert_eq!(restored.kind(), r.kind());
        prop_assert_eq!(restored.status(), r.status());
    }

    #[test]
    fn effect_record_rejects_empty_id() {
        let result = EffectRecord::new(
            String::new(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        );
        prop_assert!(result.is_none());
    }

    // ========================================================================
    // ExternalReceipt Tests
    // ========================================================================

    #[test]
    fn external_receipt_roundtrip() {
        let receipt = ExternalReceipt::new(
            "connector-abc".to_string(),
            "1.0.0".to_string(),
            EffectKind::BlobWrite,
            serde_json::json!({"receipt_id": "123", "timestamp": 999}),
        ).unwrap();
        let json = serde_json::to_string(&receipt).unwrap();
        let restored: ExternalReceipt = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored.connector_id(), receipt.connector_id());
        prop_assert_eq!(restored.connector_version(), receipt.connector_version());
        prop_assert_eq!(restored.sink_kind(), receipt.sink_kind());
    }

    #[test]
    fn external_receipt_rejects_empty_connector_id() {
        let result = ExternalReceipt::new(
            String::new(),
            "1.0.0".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
        );
        prop_assert!(result.is_none());
    }
}