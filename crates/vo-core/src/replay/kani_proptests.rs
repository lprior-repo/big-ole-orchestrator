//! Kani harnesses and proptest invariants for the replay engine.


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

    proptest! {
        #[test]
        fn replay_events_applied_never_exceeds_input_len(
            seq in 1u64..=10u64,
        ) {
            let engine = ReplayEngine::new();
            let event = make_event("inst-1", seq, workflow_started_payload("wf-1"));
            let events = vec![event];
            let result = engine.replay(&events).expect("replay");
            prop_assert!(result.events_applied <= events.len());
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
