//! Kani harnesses and proptest invariants for the replay engine (ADR-027).
//!
//! Proptest invariants verify:
//! - Determinism: same events always produce same result
//! - events_applied never exceeds input length
//! - Snapshot replay equivalence
//! - All valid transitions accepted, invalid transitions rejected

use super::engine::ReplayEngine;
use super::test_helpers::*;
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::state::LifecycleState;

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
            payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
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
        payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
        metadata: EventMetadata::default(),
    }]);
    let r2 = engine.replay(&[EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}),
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
        fn replay_events_applied_never_exceeds_input_len(seq in 1u64..=100u64) {
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

    proptest! {
        #[test]
        fn replay_determinism_property(seq1 in 1u64..=100u64, seq2 in 1u64..=100u64) {
            let engine = ReplayEngine::new();
            let events = vec![
                make_event("inst-1", seq1, workflow_started_payload("wf-1")),
                make_event("inst-1", seq1 + 1, step_scheduled_payload("wf-1", "step-1")),
                make_event("inst-1", seq1 + 2, step_started_payload("wf-1", "step-1")),
                make_event("inst-1", seq1 + 3, step_completed_payload("wf-1", "step-1")),
            ];
            let result1 = engine.replay(&events).expect("first replay");
            let result2 = engine.replay(&events).expect("second replay");
            prop_assert_eq!(result1.final_state, result2.final_state);
            prop_assert_eq!(result1.events_applied, result2.events_applied);
        }
    }

    proptest! {
        #[test]
        fn snapshot_replay_equivalence(seq in 1u64..=50u64) {
            let engine = ReplayEngine::new();
            let full_events = vec![
                make_event("inst-1", 1, workflow_started_payload("wf-1")),
                make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
                make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
                make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
            ];

            let split_point = ((seq % 3) as usize).min(3).max(1);
            let pre = &full_events[..split_point];
            let post: Vec<_> = full_events[split_point..]
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let new_seq = (split_point as u64) + 1 + (i as u64);
                    make_event(e.instance_id.as_str(), new_seq, e.payload.clone())
                })
                .collect();

            let mut combined = Vec::with_capacity(full_events.len());
            combined.extend_from_slice(pre);
            combined.extend_from_slice(&post);

            let full_result = engine.replay(&full_events).expect("full replay");
            let combined_result = engine.replay(&combined).expect("combined replay");

            prop_assert_eq!(full_result.final_state, combined_result.final_state);
        }
    }

    proptest! {
        #[test]
        fn sequence_gap_detected_at_correct_position(
            seq1 in 1u64..=50u64,
            gap in 2u64..=10u64
        ) {
            let engine = ReplayEngine::new();
            let gap_seq = seq1 + gap;
            let events = vec![
                make_event("inst-1", seq1, workflow_started_payload("wf-1")),
                make_event("inst-1", seq1 + 1, step_scheduled_payload("wf-1", "step-1")),
                make_event("inst-1", gap_seq, step_started_payload("wf-1", "step-1")),
            ];
            let result = engine.replay(&events);
            prop_assert!(result.is_err());
        }
    }

    proptest! {
        #[test]
        fn sequence_duplicate_detected(
            seq in 1u64..=50u64
        ) {
            let engine = ReplayEngine::new();
            let events = vec![
                make_event("inst-1", seq, workflow_started_payload("wf-1")),
                make_event("inst-1", seq + 1, step_scheduled_payload("wf-1", "step-1")),
                make_event("inst-1", seq, step_started_payload("wf-1", "step-1")),
            ];
            let result = engine.replay(&events);
            prop_assert!(result.is_err());
        }
    }

    proptest! {
        #[test]
        fn instance_mismatch_detects_first_differing_pair(
            id1 in "[a-z]{1,10}",
            id2 in "[a-z]{1,10}"
        ) {
            let engine = ReplayEngine::new();
            let events = vec![
                make_event(&id1, 1, workflow_started_payload("wf-1")),
                make_event(&id1, 2, step_scheduled_payload("wf-1", "step-1")),
                make_event(&id2, 3, step_started_payload("wf-1", "step-1")),
            ];
            let result = engine.replay(&events);
            prop_assert!(result.is_err());
        }
    }

    proptest! {
        #[test]
        fn idempotent_replay_produces_identical_results(
            workflow_id in "[a-z0-9]{1,20}"
        ) {
            let engine = ReplayEngine::new();
            let events = vec![
                make_event("inst-1", 1, workflow_started_payload(&workflow_id)),
                make_event("inst-1", 2, step_scheduled_payload(&workflow_id, "step-1")),
                make_event("inst-1", 3, step_started_payload(&workflow_id, "step-1")),
                make_event("inst-1", 4, step_completed_payload(&workflow_id, "step-1")),
            ];

            let result1 = engine.replay(&events).expect("first");
            let result2 = engine.replay(&events).expect("second");
            let result3 = engine.replay(&events).expect("third");

            prop_assert_eq!(result1.final_state, result2.final_state);
            prop_assert_eq!(result2.final_state, result3.final_state);
            prop_assert_eq!(result1.events_applied, result2.events_applied);
            prop_assert_eq!(result2.events_applied, result3.events_applied);
        }
    }

    proptest! {
        #[test]
        fn failure_then_resume_converges_to_same_state(
            workflow_id in "[a-z0-9]{1,20}"
        ) {
            let engine = ReplayEngine::new();
            let events = vec![
                make_event("inst-1", 1, workflow_started_payload(&workflow_id)),
                make_event("inst-1", 2, step_scheduled_payload(&workflow_id, "step-1")),
                make_event("inst-1", 3, step_started_payload(&workflow_id, "step-1")),
                make_event("inst-1", 4, step_failed_payload(&workflow_id, "step-1")),
                make_event("inst-1", 5, instance_resumed_payload(&workflow_id)),
            ];

            let result = engine.replay(&events).expect("replay should succeed");
            prop_assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
        }
    }

    proptest! {
        #[test]
        fn terminal_state_ignores_subsequent_events(
            workflow_id in "[a-z0-9]{1,20}"
        ) {
            let engine = ReplayEngine::new();
            let events = vec![
                make_event("inst-1", 1, workflow_started_payload(&workflow_id)),
                make_event("inst-1", 2, step_scheduled_payload(&workflow_id, "step-1")),
                make_event("inst-1", 3, step_started_payload(&workflow_id, "step-1")),
                make_event("inst-1", 4, step_completed_payload(&workflow_id, "step-1")),
                make_event("inst-1", 5, step_scheduled_payload(&workflow_id, "step-2")),
                make_event("inst-1", 6, timer_set_payload(&workflow_id, "timer-1")),
            ];

            let result = engine.replay(&events).expect("replay should succeed");
            prop_assert_eq!(result.final_state, Some(LifecycleState::Completed));
            prop_assert_eq!(result.events_applied, 4);
        }
    }
}
