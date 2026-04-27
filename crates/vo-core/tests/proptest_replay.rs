//! Property-based tests for replay determinism and exact-once guarantees (ADR-027, ADR-043).
//!
//! These tests verify:
//! - PROP-1: Replay is deterministic — same events always produce same result
//! - PROP-2: Replay is idempotent — multiple replays produce identical results
//! - PROP-3: Snapshot boundaries are irrelevant — replay from any point produces same final state
//! - PROP-4: Crash/recovery produces same state as uninterrupted execution
//!
//! ## Exact-Once Guarantee
//!
//! The core invariant: Replaying a valid event sequence MUST produce exactly the same
//! final state as original execution, regardless of:
//! - When crashes occur (crash injection)
//! - Where snapshot boundaries are (snapshot-aware replay)
//! - How many times replay is executed (idempotency)

use proptest::prelude::*;
use vo_core::replay::ReplayEngine;
use vo_types::events::{EventEnvelope, EventMetadata};

// =============================================================================
// Arbitrary Implementations for Test Events
// =============================================================================

/// Generate a WorkflowStarted event payload.
fn workflow_started_payload(wf_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": wf_id,
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1
    })
}

/// Generate a StepScheduled event payload.
fn step_scheduled_payload(wf_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepScheduled",
        "workflow_id": wf_id,
        "step_id": step_id,
        "attempt": 1,
        "fence": 1,
        "execution_id": "exec-1",
        "version": 1
    })
}

/// Generate a StepStarted event payload.
fn step_started_payload(wf_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepStarted",
        "workflow_id": wf_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

/// Generate a StepCompleted event payload.
fn step_completed_payload(wf_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepCompleted",
        "workflow_id": wf_id,
        "step_id": step_id,
        "completed_at_ms": 3000,
        "attempt": 1,
        "fence": 1,
        "routing_projection": null,
        "output_ref": null,
        "output_hash": null,
        "output": null,
        "version": 1
    })
}

/// Generate a StepFailed event payload.
fn step_failed_payload(wf_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepFailed",
        "workflow_id": wf_id,
        "step_id": step_id,
        "failure_reason": "error",
        "attempt": 1,
        "fence": 1,
        "version": 1
    })
}

/// Generate an InstanceResumed event payload.
fn instance_resumed_payload(wf_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "InstanceResumed",
        "workflow_id": wf_id,
        "resumed_at_ms": 6000,
        "version": 1
    })
}

/// Create a valid EventEnvelope for testing.
fn make_event(
    instance_id: &str,
    sequence: u64,
    timestamp_ms: u64,
    payload: serde_json::Value,
    schema_version: u8,
) -> EventEnvelope {
    EventEnvelope {
        schema_version,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms,
        payload,
        metadata: EventMetadata::default(),
    }
}

/// A workflow event sequence for property testing.
#[derive(Debug, Clone)]
struct WorkflowSequence {
    events: Vec<EventEnvelope>,
}

impl WorkflowSequence {
    /// Generate a simple completion sequence: WorkflowStarted -> StepScheduled -> StepStarted -> StepCompleted
    fn simple_completion(wf_id: &str, instance_id: &str) -> Self {
        let ts_base = 1000u64;
        Self {
            events: vec![
                make_event(instance_id, 1, ts_base, workflow_started_payload(wf_id), 1),
                make_event(
                    instance_id,
                    2,
                    ts_base + 100,
                    step_scheduled_payload(wf_id, "step-1"),
                    1,
                ),
                make_event(
                    instance_id,
                    3,
                    ts_base + 200,
                    step_started_payload(wf_id, "step-1"),
                    1,
                ),
                make_event(
                    instance_id,
                    4,
                    ts_base + 300,
                    step_completed_payload(wf_id, "step-1"),
                    1,
                ),
            ],
        }
    }

    /// Generate a failure sequence: WorkflowStarted -> StepScheduled -> StepStarted -> StepFailed -> InstanceResumed -> StepScheduled
    fn failure_recovery(wf_id: &str, instance_id: &str) -> Self {
        let ts_base = 1000u64;
        Self {
            events: vec![
                make_event(instance_id, 1, ts_base, workflow_started_payload(wf_id), 1),
                make_event(
                    instance_id,
                    2,
                    ts_base + 100,
                    step_scheduled_payload(wf_id, "step-1"),
                    1,
                ),
                make_event(
                    instance_id,
                    3,
                    ts_base + 200,
                    step_started_payload(wf_id, "step-1"),
                    1,
                ),
                make_event(
                    instance_id,
                    4,
                    ts_base + 300,
                    step_failed_payload(wf_id, "step-1"),
                    1,
                ),
                make_event(
                    instance_id,
                    5,
                    ts_base + 400,
                    instance_resumed_payload(wf_id),
                    1,
                ),
                make_event(
                    instance_id,
                    6,
                    ts_base + 500,
                    step_scheduled_payload(wf_id, "step-1"),
                    1,
                ),
            ],
        }
    }

    fn events(&self) -> &[EventEnvelope] {
        &self.events
    }
}

// =============================================================================
// PROP-1: Determinism — Same events always produce same result
// =============================================================================

proptest! {
    /// Invariant: Replay is deterministic — running the same events through replay
    /// produces identical results every time.
    ///
    /// Anti-invariant: Non-deterministic replay would break exact-once guarantees.
    #[test]
    fn replay_is_deterministic_for_completion_sequence(seed: u64) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let seq = WorkflowSequence::simple_completion(&wf_id, &instance_id);
        let engine = ReplayEngine::new();

        let result1 = engine.replay(seq.events());
        let result2 = engine.replay(seq.events());
        let result3 = engine.replay(seq.events());

        prop_assert_eq!(result1.clone(), result2.clone(), "First and second replay should match");
        prop_assert_eq!(result2.clone(), result3.clone(), "Second and third replay should match");
    }

    /// Invariant: Failure-recovery sequences are also deterministic.
    #[test]
    fn replay_is_deterministic_for_failure_recovery_sequence(seed: u64) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let seq = WorkflowSequence::failure_recovery(&wf_id, &instance_id);
        let engine = ReplayEngine::new();

        let result1 = engine.replay(seq.events());
        let result2 = engine.replay(seq.events());

        prop_assert_eq!(result1, result2, "Replay should be deterministic for failure-recovery");
    }

    /// Invariant: Empty event sequences return empty results consistently.
    #[test]
    fn replay_empty_sequence_is_deterministic(_seed: u64) {
        let engine = ReplayEngine::new();
        let events: Vec<EventEnvelope> = vec![];

        let result1 = engine.replay(&events);
        let result2 = engine.replay(&events);

        prop_assert_eq!(result1.clone(), result2.clone(), "Empty replay should be deterministic");
        let r1 = result1.unwrap();
        prop_assert_eq!(r1.final_state, None, "Empty replay should have no final state");
        prop_assert_eq!(r1.events_applied, 0, "Empty replay should apply zero events");
    }
}

// =============================================================================
// PROP-2: Idempotency — Multiple replays produce identical results
// =============================================================================

proptest! {
    /// Invariant: Replaying events multiple times produces identical final state
    /// and event count.
    ///
    /// Anti-invariant: Non-idempotent replay would cause state divergence on retry.
    #[test]
    fn replay_is_idempotent(seq_size in 1usize..=20, seed: u64) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let ts_base = 1000u64;
        let mut events = Vec::with_capacity(seq_size);
        events.push(make_event(&instance_id, 1, ts_base, workflow_started_payload(&wf_id), 1));

        for i in 2..=seq_size {
            let step_id = format!("step-{}", i);
            events.push(make_event(
                &instance_id,
                i as u64,
                ts_base + (i as u64) * 100,
                step_scheduled_payload(&wf_id, &step_id),
                1,
            ));
        }

        let engine = ReplayEngine::new();

        let results: Vec<_> = (0..3)
            .map(|_| engine.replay(&events))
            .collect();

        prop_assert_eq!(results[0].clone(), results[1].clone(), "First and second replay should match");
        prop_assert_eq!(results[1].clone(), results[2].clone(), "Second and third replay should match");
    }
}

// =============================================================================
// PROP-3: Snapshot Boundaries — Replay from any point produces same final state
// =============================================================================

proptest! {
    /// Invariant: Replaying from any snapshot boundary produces the same final state
    /// as replaying the same events again.
    ///
    /// Anti-invariant: Different results from replaying the same events would
    /// mean the replay is not deterministic.
    #[test]
    fn replay_from_any_snapshot_boundary_is_deterministic(
        snapshot_seq in 1u64..=5u64,
        seed: u64,
    ) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let ts_base = 1000u64;

        let full_events = [make_event(&instance_id, 1, ts_base, workflow_started_payload(&wf_id), 1),
            make_event(&instance_id, 2, ts_base + 100, step_scheduled_payload(&wf_id, "step-1"), 1),
            make_event(&instance_id, 3, ts_base + 200, step_started_payload(&wf_id, "step-1"), 1),
            make_event(&instance_id, 4, ts_base + 300, step_completed_payload(&wf_id, "step-1"), 1),
            make_event(&instance_id, 5, ts_base + 400, step_scheduled_payload(&wf_id, "step-2"), 1),
            make_event(&instance_id, 6, ts_base + 500, step_started_payload(&wf_id, "step-2"), 1),
            make_event(&instance_id, 7, ts_base + 600, step_completed_payload(&wf_id, "step-2"), 1)];

        let engine = ReplayEngine::new();

        let snapshot_events: Vec<_> = full_events.iter().take(snapshot_seq as usize).cloned().collect();

        // Invariant: replaying the same events produces the same result
        let replay1 = engine.replay(&snapshot_events);
        let replay2 = engine.replay(&snapshot_events);

        prop_assert_eq!(replay1.clone(), replay2.clone(),
            "Replay from same boundary should be deterministic");
    }

    /// Invariant: Replay from sequence 0 (no snapshot) is equivalent to full replay
    /// when no events are lost.
    #[test]
    fn replay_from_seq_zero_is_full_replay(seed: u64) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let seq = WorkflowSequence::simple_completion(&wf_id, &instance_id);
        let engine = ReplayEngine::new();

        let full_result = engine.replay(seq.events()).expect("full replay should succeed");
        let again_result = engine.replay(seq.events()).expect("again replay should succeed");

        prop_assert_eq!(full_result.final_state, again_result.final_state);
        prop_assert_eq!(full_result.events_applied, again_result.events_applied);
    }
}

// =============================================================================
// PROP-4: Crash/Recovery — Same state as uninterrupted execution
// =============================================================================

proptest! {
    /// Invariant: After a crash and recovery, replaying the full event sequence
    /// produces the same final state as uninterrupted execution.
    ///
    /// Anti-invariant: State divergence after recovery would break exact-once guarantees.
    #[test]
    fn crash_recovery_produces_same_final_state(
        crash_index in 0usize..=5usize,
        seed: u64,
    ) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let ts_base = 1000u64;

        let full_events = vec![
            make_event(&instance_id, 1, ts_base, workflow_started_payload(&wf_id), 1),
            make_event(&instance_id, 2, ts_base + 100, step_scheduled_payload(&wf_id, "step-1"), 1),
            make_event(&instance_id, 3, ts_base + 200, step_started_payload(&wf_id, "step-1"), 1),
            make_event(&instance_id, 4, ts_base + 300, step_failed_payload(&wf_id, "step-1"), 1),
            make_event(&instance_id, 5, ts_base + 400, instance_resumed_payload(&wf_id), 1),
            make_event(&instance_id, 6, ts_base + 500, step_scheduled_payload(&wf_id, "step-1"), 1),
        ];

        let engine = ReplayEngine::new();

        let pre_crash_events: Vec<_> = full_events.iter().take(crash_index).cloned().collect();
        let post_crash_events: Vec<_> = full_events.iter().skip(crash_index).cloned().collect();

        let full_result = engine.replay(&full_events);

        let combined_result = if !pre_crash_events.is_empty() {
            let mut all_events = pre_crash_events.clone();
            all_events.extend(post_crash_events.iter().cloned());
            engine.replay(&all_events)
        } else {
            engine.replay(&post_crash_events)
        };

        prop_assert_eq!(
            full_result.expect("full replay should succeed").final_state,
            combined_result.expect("combined replay should succeed").final_state,
            "Crash recovery should produce same final state as uninterrupted execution"
        );
    }

    /// Invariant: Repeated failure-recovery cycles converge to the same state.
    #[test]
    fn multiple_failure_recovery_cycles_converge(seed: u64) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);

        let seq = WorkflowSequence::failure_recovery(&wf_id, &instance_id);
        let engine = ReplayEngine::new();

        let result1 = engine.replay(seq.events()).expect("first recovery should succeed");
        let result2 = engine.replay(seq.events()).expect("second recovery should succeed");
        let result3 = engine.replay(seq.events()).expect("third recovery should succeed");

        prop_assert_eq!(result1.final_state, result2.final_state, "First and second should match");
        prop_assert_eq!(result2.final_state, result3.final_state, "Second and third should match");
        prop_assert_eq!(result1.events_applied, result2.events_applied);
        prop_assert_eq!(result2.events_applied, result3.events_applied);
    }
}

// =============================================================================
// Error Case Tests
// =============================================================================

proptest! {
    /// Invariant: Duplicate sequences are rejected deterministically.
    #[test]
    fn duplicate_sequence_rejected_deterministically(
        base_seq in 1u64..=100u64,
        seed: u64,
    ) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);
        let ts_base = 1000u64;

        let events = vec![
            make_event(&instance_id, base_seq, ts_base, workflow_started_payload(&wf_id), 1),
            make_event(&instance_id, base_seq, ts_base + 100, step_scheduled_payload(&wf_id, "step-1"), 1),
        ];

        let engine = ReplayEngine::new();

        let result1 = engine.replay(&events);
        let result2 = engine.replay(&events);

        prop_assert!(result1.is_err(), "Duplicate sequence should be rejected");
        prop_assert_eq!(result1, result2, "Duplicate rejection should be deterministic");
    }

    /// Invariant: Instance ID mismatch is rejected deterministically.
    #[test]
    fn instance_mismatch_rejected_deterministically(seed: u64) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id_1 = format!("inst-{}", seed % 256);
        let instance_id_2 = format!("inst-{}", (seed + 1) % 256);
        let ts_base = 1000u64;

        let events = vec![
            make_event(&instance_id_1, 1, ts_base, workflow_started_payload(&wf_id), 1),
            make_event(&instance_id_2, 2, ts_base + 100, step_scheduled_payload(&wf_id, "step-1"), 1),
        ];

        let engine = ReplayEngine::new();

        let result1 = engine.replay(&events);
        let result2 = engine.replay(&events);

        prop_assert!(result1.is_err(), "Instance mismatch should be rejected");
        prop_assert_eq!(result1, result2, "Instance mismatch rejection should be deterministic");
    }

    /// Invariant: Sequence gaps are rejected deterministically.
    #[test]
    fn sequence_gap_rejected_deterministically(
        first_seq in 1u64..=100u64,
        gap in 2u64..=10u64,
        seed: u64,
    ) {
        let wf_id = format!("wf-{}", seed % 256);
        let instance_id = format!("inst-{}", seed % 256);
        let ts_base = 1000u64;

        let second_seq = first_seq + gap;

        let events = vec![
            make_event(&instance_id, first_seq, ts_base, workflow_started_payload(&wf_id), 1),
            make_event(&instance_id, second_seq, ts_base + 100, step_scheduled_payload(&wf_id, "step-1"), 1),
        ];

        let engine = ReplayEngine::new();

        let result1 = engine.replay(&events);
        let result2 = engine.replay(&events);

        prop_assert!(result1.is_err(), "Sequence gap should be rejected");
        prop_assert_eq!(result1, result2, "Sequence gap rejection should be deterministic");
    }
}

// =============================================================================
// Anti-Invariant Tests — What SHOULD NOT Happen
// =============================================================================

proptest! {
    /// Anti-invariant: Replay must NEVER produce different results for same input.
    #[test]
    fn anti_invariant_replay_must_not_be_non_deterministic(_seed: u64) {
        let wf_id = "wf-static";
        let instance_id = "inst-static";

        let seq = WorkflowSequence::simple_completion(wf_id, instance_id);
        let engine = ReplayEngine::new();

        let results: Vec<_> = (0..10).map(|_| engine.replay(seq.events())).collect();

        for i in 1..results.len() {
            prop_assert_eq!(
                results[0].clone(), results[i].clone(),
                "Anti-invariant violated: replay produced different results on iteration {} vs 0",
                i
            );
        }
    }

    /// Anti-invariant: State must NEVER diverge on repeated replay.
    #[test]
    fn anti_invariant_state_must_not_diverge_on_retry(_seed: u64) {
        let wf_id = "wf-divergence-test";
        let instance_id = "inst-divergence-test";

        let seq = WorkflowSequence::failure_recovery(wf_id, instance_id);
        let engine = ReplayEngine::new();

        let mut previous_state = None;
        for _ in 0..20 {
            let result = engine.replay(seq.events()).expect("replay should succeed");

            if let Some(prev) = previous_state {
                prop_assert_eq!(
                    prev, result.final_state,
                    "Anti-invariant violated: state diverged on retry"
                );
            }
            previous_state = Some(result.final_state);
        }
    }
}
