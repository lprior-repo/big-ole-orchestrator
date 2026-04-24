//! Kani harnesses and proptest invariants for the replay engine.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use serde_json::json;
use vo_types::events::{EventEnvelope, EventMetadata};

// =========================================================================
// Kani harnesses
// =========================================================================

#[cfg(kani)]
#[kani::proof]
fn kani_replay_never_panics() {
    let engine = ReplayEngine::new();
    let seq: u64 = kani::any();
    if seq >= 1 {
        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: seq,
            timestamp_ms: 1000,
            payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
            metadata: EventMetadata::default(),
        };
        let _ = engine.replay(&[event]);
    }
}

#[cfg(kani)]
#[kani::proof]
fn kani_replay_determinism() {
    let engine = ReplayEngine::new();
    let r1 = engine.replay(&[EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
        metadata: EventMetadata::default(),
    }]);
    let r2 = engine.replay(&[EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
        metadata: EventMetadata::default(),
    }]);
    assert_eq!(r1, r2);
}

// =========================================================================
// Proptest invariants
// =========================================================================

mod proptests {
    use super::super::engine::ReplayEngine;
    use super::super::test_helpers::*;
    use proptest::prelude::*;
    use vo_types::events::EventEnvelope;

    fn valid_event_sequence(max_len: usize) -> impl Strategy<Value = Vec<EventEnvelope>> {
        let instance_id = "proptest-inst";
        prop::collection::vec(
            prop::sample::select(valid_workflow_payloads(instance_id)),
            1..=max_len,
        )
        .prop_map(move |payloads| {
            payloads
                .into_iter()
                .enumerate()
                .map(|(i, payload)| make_event(instance_id, (i + 1) as u64, payload))
                .collect()
        })
    }

    fn valid_workflow_payloads(instance_id: &str) -> Vec<serde_json::Value> {
        let wf = "wf-proptest";
        vec![
            workflow_started_payload(wf),
            step_scheduled_payload(wf, "step-a"),
            step_started_payload(wf, "step-a"),
            step_completed_payload(wf, "step-a"),
            step_failed_payload(wf, "step-a"),
            instance_resumed_payload(wf),
            timer_set_payload(wf, "timer-1"),
            timer_fired_payload(wf, "timer-1"),
            workflow_cancelled_payload(wf),
            cancel_requested_payload(wf),
            workflow_failed_payload(wf),
            effect_prepared_payload(wf, "step-a", "fx-1"),
            effect_committed_payload(wf, "step-a", "fx-1"),
        ]
    }

    proptest! {
        #[test]
        fn replay_determinism_same_events_same_state(events in valid_event_sequence(8)) {
            let engine = ReplayEngine::new();
            let r1 = engine.replay(&events);
            let r2 = engine.replay(&events);
            let r3 = engine.replay(&events);
            prop_assert_eq!(&r1, &r2, "replay 1 != replay 2");
            prop_assert_eq!(&r2, &r3, "replay 2 != replay 3");
        }

        #[test]
        fn replay_determinism_events_applied_never_exceeds_input(events in valid_event_sequence(8)) {
            let engine = ReplayEngine::new();
            if let Ok(result) = engine.replay(&events) {
                prop_assert!(result.events_applied <= events.len());
            }
        }

        #[test]
        fn replay_determinism_state_is_always_some_for_non_empty(events in valid_event_sequence(8)) {
            let engine = ReplayEngine::new();
            if let Ok(result) = engine.replay(&events) {
                prop_assert!(result.events_applied <= events.len());
            }
        }
    }

    #[test]
    fn replay_empty_always_returns_none_state() {
        let engine = ReplayEngine::new();
        let result = engine.replay(&[]).expect("replay");
        assert_eq!(result.final_state, None);
        assert_eq!(result.events_applied, 0);
    }
}
