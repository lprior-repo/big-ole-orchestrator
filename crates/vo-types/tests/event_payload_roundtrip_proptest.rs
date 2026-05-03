//! Proptest tests for EventPayload serialization roundtrip via try_from_json.
//! Covers all 18 variants of the EventPayload enum.
//! Bead: ve-bh0

use proptest::prelude::*;
use vo_types::events::payload::{EventPayload, SinkKind, StepOutput};
use vo_types::events::EventEnvelope;

fn valid_workflow_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_step_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_timer_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_effect_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_hash() -> impl Strategy<Value = String> { "[a-f0-9]{8,64}".prop_filter("even length", |s| s.len() % 2 == 0) }
fn valid_reason() -> impl Strategy<Value = String> { "[a-zA-Z0-9 .,_-]{1,500}" }
fn valid_actor() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_signal_name() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_lineage_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_instance_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }
fn valid_execution_id() -> impl Strategy<Value = String> { "[a-zA-Z0-9_-]{1,255}" }

fn simple_json_value() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(|v| serde_json::Value::Number(v.into())),
        any::<String>().prop_map(serde_json::Value::String),
    ]
}

fn payload_json(payload_type: &str, fields: Vec<(&str, serde_json::Value)>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), serde_json::Value::String(payload_type.to_string()));
    map.insert("version".to_string(), serde_json::Value::Number(0u64.into()));
    for (k, v) in fields {
        map.insert(k.to_string(), v);
    }
    serde_json::Value::Object(map)
}

// 1. WorkflowStarted
proptest! {
    #[test]
    fn roundtrip_workflow_started(
        workflow_id in valid_workflow_id(),
        binary_hash in valid_hash(),
        version_hash in valid_hash(),
        dedupe_present in any::<bool>(),
        dedupe_val in valid_hash(),
    ) {
        let topology = serde_json::json!({"nodes": [], "edges": []});
        let json = payload_json("WorkflowStarted", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("dag_topology", topology.clone()),
            ("binary_hash", serde_json::Value::String(binary_hash.clone())),
            ("workflow_version_hash", serde_json::Value::String(version_hash.clone())),
            ("dedupe_key_hash", if dedupe_present { serde_json::Value::String(dedupe_val.clone()) } else { serde_json::Value::Null }),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        let expected = EventPayload::WorkflowStarted {
            workflow_id,
            dag_topology: topology,
            binary_hash,
            workflow_version_hash: vo_types::WorkflowVersionHash::try_from(version_hash).unwrap(),
            dedupe_key_hash: if dedupe_present { Some(dedupe_val) } else { None },
        };
        prop_assert_eq!(parsed, expected);
    }
}

// 2. WorkflowCompleted
proptest! {
    #[test]
    fn roundtrip_workflow_completed(workflow_id in valid_workflow_id(), completion_time_ms in any::<u64>()) {
        let json = payload_json("WorkflowCompleted", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("completion_time_ms", serde_json::Value::Number(completion_time_ms.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::WorkflowCompleted { workflow_id, completion_time_ms });
    }
}

// 3. WorkflowFailed
proptest! {
    #[test]
    fn roundtrip_workflow_failed(workflow_id in valid_workflow_id(), failure_reason in valid_reason()) {
        let json = payload_json("WorkflowFailed", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("failure_reason", serde_json::Value::String(failure_reason.clone())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::WorkflowFailed { workflow_id, failure_reason });
    }
}

// 4. WorkflowCancelled
proptest! {
    #[test]
    fn roundtrip_workflow_cancelled(workflow_id in valid_workflow_id(), cancelled_by in valid_actor()) {
        let json = payload_json("WorkflowCancelled", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("cancelled_by", serde_json::Value::String(cancelled_by.clone())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::WorkflowCancelled { workflow_id, cancelled_by });
    }
}

// 5. StepScheduled
proptest! {
    #[test]
    fn roundtrip_step_scheduled(
        workflow_id in valid_workflow_id(),
        step_id in valid_step_id(),
        attempt in 1u32..=100u32,
        fence in any::<u64>(),
        execution_id in valid_execution_id(),
    ) {
        let json = payload_json("StepScheduled", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("step_id", serde_json::Value::String(step_id.clone())),
            ("attempt", serde_json::Value::Number((attempt as u64).into())),
            ("fence", serde_json::Value::Number(fence.into())),
            ("execution_id", serde_json::Value::String(execution_id.clone())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::StepScheduled {
            workflow_id, step_id, attempt, fence, execution_id,
        });
    }
}

// 6. StepStarted
proptest! {
    #[test]
    fn roundtrip_step_started(
        workflow_id in valid_workflow_id(),
        step_id in valid_step_id(),
        started_at_ms in any::<u64>(),
    ) {
        let json = payload_json("StepStarted", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("step_id", serde_json::Value::String(step_id.clone())),
            ("started_at_ms", serde_json::Value::Number(started_at_ms.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::StepStarted { workflow_id, step_id, started_at_ms });
    }
}

// 7. StepCompleted
proptest! {
    #[test]
    fn roundtrip_step_completed(
        workflow_id in valid_workflow_id(),
        step_id in valid_step_id(),
        completed_at_ms in any::<u64>(),
        attempt in 1u32..=100u32,
        fence in any::<u64>(),
        output_val in simple_json_value(),
        output_ref_present in any::<bool>(),
        output_ref_val in valid_hash(),
        output_hash_present in any::<bool>(),
        output_hash_val in valid_hash(),
        routing in simple_json_value(),
    ) {
        let json = payload_json("StepCompleted", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("step_id", serde_json::Value::String(step_id.clone())),
            ("completed_at_ms", serde_json::Value::Number(completed_at_ms.into())),
            ("attempt", serde_json::Value::Number((attempt as u64).into())),
            ("fence", serde_json::Value::Number(fence.into())),
            ("routing_projection", routing.clone()),
            ("output_ref", if output_ref_present { serde_json::Value::String(output_ref_val.clone()) } else { serde_json::Value::Null }),
            ("output_hash", if output_hash_present { serde_json::Value::String(output_hash_val.clone()) } else { serde_json::Value::Null }),
            ("output", output_val.clone()),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        let expected_output = if output_val.is_null() { StepOutput::Null } else { StepOutput::Inline(output_val) };
        prop_assert_eq!(parsed, EventPayload::StepCompleted {
            workflow_id, step_id, completed_at_ms, attempt, fence,
            routing_projection: routing,
            output_ref: if output_ref_present { Some(output_ref_val) } else { None },
            output_hash: if output_hash_present { Some(output_hash_val) } else { None },
            output: expected_output,
        });
    }
}

// 8. StepFailed
proptest! {
    #[test]
    fn roundtrip_step_failed(
        workflow_id in valid_workflow_id(),
        step_id in valid_step_id(),
        failure_reason in valid_reason(),
        attempt in 1u32..=100u32,
        fence in any::<u64>(),
    ) {
        let json = payload_json("StepFailed", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("step_id", serde_json::Value::String(step_id.clone())),
            ("failure_reason", serde_json::Value::String(failure_reason.clone())),
            ("attempt", serde_json::Value::Number((attempt as u64).into())),
            ("fence", serde_json::Value::Number(fence.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::StepFailed {
            workflow_id, step_id, failure_reason, attempt, fence,
        });
    }
}

// 9. EffectPrepared (all 3 sink kinds)
proptest! {
    #[test]
    fn roundtrip_effect_prepared(
        workflow_id in valid_workflow_id(),
        step_id in valid_step_id(),
        effect_id in valid_effect_id(),
        sink_idx in 0usize..3,
        payload_hash in valid_hash(),
        fence in any::<u64>(),
    ) {
        let sinks = ["BlobWrite", "TimerWrite", "SignalWrite"];
        let sink_str = sinks[sink_idx];
        let json = payload_json("EffectPrepared", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("step_id", serde_json::Value::String(step_id.clone())),
            ("effect_id", serde_json::Value::String(effect_id.clone())),
            ("sink_kind", serde_json::Value::String(sink_str.to_string())),
            ("payload_hash", serde_json::Value::String(payload_hash.clone())),
            ("fence", serde_json::Value::Number(fence.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        let expected_sink = match sink_str {
            "BlobWrite" => SinkKind::BlobWrite,
            "TimerWrite" => SinkKind::TimerWrite,
            _ => SinkKind::SignalWrite,
        };
        prop_assert_eq!(parsed, EventPayload::EffectPrepared {
            workflow_id, step_id, effect_id, sink_kind: expected_sink, payload_hash, fence,
        });
    }
}

// 10. EffectCommitted
proptest! {
    #[test]
    fn roundtrip_effect_committed(
        workflow_id in valid_workflow_id(),
        step_id in valid_step_id(),
        effect_id in valid_effect_id(),
        receipt in simple_json_value(),
        fence in any::<u64>(),
    ) {
        let json = payload_json("EffectCommitted", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("step_id", serde_json::Value::String(step_id.clone())),
            ("effect_id", serde_json::Value::String(effect_id.clone())),
            ("external_receipt", receipt.clone()),
            ("fence", serde_json::Value::Number(fence.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::EffectCommitted {
            workflow_id, step_id, effect_id, external_receipt: receipt, fence,
        });
    }
}

// 11. TimerSet
proptest! {
    #[test]
    fn roundtrip_timer_set(workflow_id in valid_workflow_id(), timer_id in valid_timer_id(), fire_at_ms in any::<u64>()) {
        let json = payload_json("TimerSet", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("timer_id", serde_json::Value::String(timer_id.clone())),
            ("fire_at_ms", serde_json::Value::Number(fire_at_ms.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::TimerSet { workflow_id, timer_id, fire_at_ms });
    }
}

// 12. TimerScheduled
proptest! {
    #[test]
    fn roundtrip_timer_scheduled(
        workflow_id in valid_workflow_id(),
        timer_id in valid_timer_id(),
        fire_at_ms in any::<u64>(),
        instance_id in valid_instance_id(),
    ) {
        let json = payload_json("TimerScheduled", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("timer_id", serde_json::Value::String(timer_id.clone())),
            ("fire_at_ms", serde_json::Value::Number(fire_at_ms.into())),
            ("instance_id", serde_json::Value::String(instance_id.clone())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::TimerScheduled { workflow_id, timer_id, fire_at_ms, instance_id });
    }
}

// 13. TimerFired
proptest! {
    #[test]
    fn roundtrip_timer_fired(workflow_id in valid_workflow_id(), timer_id in valid_timer_id(), fired_at_ms in any::<u64>()) {
        let json = payload_json("TimerFired", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("timer_id", serde_json::Value::String(timer_id.clone())),
            ("fired_at_ms", serde_json::Value::Number(fired_at_ms.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::TimerFired { workflow_id, timer_id, fired_at_ms });
    }
}

// 14. CancelRequested
proptest! {
    #[test]
    fn roundtrip_cancel_requested(workflow_id in valid_workflow_id(), requested_by in valid_actor()) {
        let json = payload_json("CancelRequested", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("requested_by", serde_json::Value::String(requested_by.clone())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::CancelRequested { workflow_id, requested_by });
    }
}

// 15. SignalAwaiting
proptest! {
    #[test]
    fn roundtrip_signal_awaiting(
        workflow_id in valid_workflow_id(),
        signal_name in valid_signal_name(),
        instance_id in valid_instance_id(),
        awaited_at_ms in any::<u64>(),
    ) {
        let json = payload_json("SignalAwaiting", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("signal_name", serde_json::Value::String(signal_name.clone())),
            ("instance_id", serde_json::Value::String(instance_id.clone())),
            ("awaited_at_ms", serde_json::Value::Number(awaited_at_ms.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::SignalAwaiting { workflow_id, signal_name, instance_id, awaited_at_ms });
    }
}

// 16. InstanceResumed
proptest! {
    #[test]
    fn roundtrip_instance_resumed(workflow_id in valid_workflow_id(), resumed_at_ms in any::<u64>()) {
        let json = payload_json("InstanceResumed", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("resumed_at_ms", serde_json::Value::Number(resumed_at_ms.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::InstanceResumed { workflow_id, resumed_at_ms });
    }
}

// 17. ContinuedAsNew
proptest! {
    #[test]
    fn roundtrip_continued_as_new(
        workflow_id in valid_workflow_id(),
        lineage_id in valid_lineage_id(),
        old_epoch in 1u64..=1000u64,
        new_epoch in 1u64..=1000u64,
    ) {
        let json = payload_json("ContinuedAsNew", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("lineage_id", serde_json::Value::String(lineage_id.clone())),
            ("old_epoch", serde_json::Value::Number(old_epoch.into())),
            ("new_epoch", serde_json::Value::Number(new_epoch.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::ContinuedAsNew { workflow_id, lineage_id, old_epoch, new_epoch });
    }
}

// 18. WorkflowQuarantined
proptest! {
    #[test]
    fn roundtrip_workflow_quarantined(
        workflow_id in valid_workflow_id(),
        failure_count in 1u8..=10u8,
        failure_window_seconds in 1u64..=3600u64,
    ) {
        let json = payload_json("WorkflowQuarantined", vec![
            ("workflow_id", serde_json::Value::String(workflow_id.clone())),
            ("failure_count", serde_json::Value::Number((failure_count as u64).into())),
            ("failure_window_seconds", serde_json::Value::Number(failure_window_seconds.into())),
        ]);
        let parsed = EventPayload::try_from_json(&json).unwrap();
        prop_assert_eq!(parsed, EventPayload::WorkflowQuarantined { workflow_id, failure_count, failure_window_seconds });
    }
}

// EventEnvelope roundtrip (serde -> from_str)
proptest! {
    #[test]
    fn envelope_roundtrip(
        instance_id in valid_instance_id(),
        sequence in 1u64..1000u64,
        timestamp_ms in any::<u64>(),
    ) {
        let envelope = EventEnvelope { schema_version: 1, instance_id, sequence, timestamp_ms, payload: serde_json::json!({"type": "TimerFired"}), metadata: Default::default() };
        let json = serde_json::to_string(&envelope).unwrap();
        let restored: EventEnvelope = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored, envelope);
    }
}

// Negative: unknown payload type
proptest! {
    #[test]
    fn reject_unknown_payload_type(type_name in "[A-Z][a-zA-Z]{3,20}") {
        let known = ["WorkflowStarted","WorkflowCompleted","WorkflowFailed","WorkflowCancelled","StepScheduled","StepStarted","StepCompleted","StepFailed","EffectPrepared","EffectCommitted","TimerSet","TimerScheduled","TimerFired","CancelRequested","SignalAwaiting","InstanceResumed","ContinuedAsNew","WorkflowQuarantined"];
        prop_assume!(!known.contains(&type_name.as_str()));
        let json = payload_json(&type_name, vec![("workflow_id", serde_json::Value::String("test".to_string()))]);
        prop_assert!(EventPayload::try_from_json(&json).is_err());
    }
}

// Negative: unsupported version
proptest! {
    #[test]
    fn reject_unsupported_version(version in 2u64..255u64) {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), serde_json::Value::String("WorkflowCompleted".to_string()));
        map.insert("version".to_string(), serde_json::Value::Number(version.into()));
        map.insert("workflow_id".to_string(), serde_json::Value::String("test".to_string()));
        map.insert("completion_time_ms".to_string(), serde_json::Value::Number(0u64.into()));
        prop_assert!(EventPayload::try_from_json(&serde_json::Value::Object(map)).is_err());
    }
}

// Negative: missing required field
#[test]
fn reject_missing_workflow_id() {
    let json = payload_json("WorkflowCompleted", vec![("completion_time_ms", serde_json::Value::Number(0u64.into()))]);
    assert!(EventPayload::try_from_json(&json).is_err());
}

// Negative: null input
#[test]
fn reject_null_input() {
    assert!(EventPayload::try_from_json(&serde_json::Value::Null).is_err());
}
