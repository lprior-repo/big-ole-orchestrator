//! Missing event handling tests for event replay.
//!
//! Verifies that the replay engine correctly detects and reports missing events
//! in sequences: single missing, burst missing, and end-of-stream truncation.
//!
//! bead_id: ve-4946q

use vo_core::replay::{ReplayEngine, ReplayError};
use vo_types::events::{EventEnvelope, EventMetadata, EventPayload, StepOutput};

fn make_event(instance_id: &str, sequence: u64, payload: EventPayload) -> EventEnvelope {
    let json = serde_json::json!({"type": format!("{:?}", payload)});
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload: json,
        metadata: EventMetadata::default(),
    }
}

#[test]
fn single_missing_event_in_middle_of_sequence_reports_gap() {
    let engine = ReplayEngine::new();
    let events = [
        make_event(
            "inst-1",
            1,
            EventPayload::WorkflowStarted {
                workflow_id: "wf-1".into(),
                dag_topology: serde_json::json!({}),
                binary_hash: "h1".into(),
                workflow_version_hash: "v1".into(),
                dedupe_key_hash: None,
            },
        ),
        make_event(
            "inst-1",
            2,
            EventPayload::StepScheduled {
                workflow_id: "wf-1".into(),
                step_id: "step-1".into(),
                attempt: 1,
                fence: 0,
                execution_id: "exec-1".into(),
            },
        ),
        make_event(
            "inst-1",
            4,
            EventPayload::StepCompleted {
                workflow_id: "wf-1".into(),
                step_id: "step-1".into(),
                completed_at_ms: 3000,
                attempt: 1,
                fence: 0,
                routing_projection: serde_json::json!({}),
                output_ref: None,
                output_hash: None,
                output: StepOutput::Null,
            },
        ),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should detect missing event 3");
    assert_eq!(
        err,
        ReplayError::SequenceGap {
            expected: 3,
            actual: 4,
            at_index: 2
        },
        "Gap should report expected=3, actual=4 at index 2"
    );
}

#[test]
fn burst_missing_events_reports_first_gap() {
    let engine = ReplayEngine::new();
    // Sequence: 1, 5 — events 2, 3, 4 are missing (burst of 3)
    let events = [
        make_event(
            "inst-1",
            1,
            EventPayload::WorkflowStarted {
                workflow_id: "wf-1".into(),
                dag_topology: serde_json::json!({}),
                binary_hash: "h1".into(),
                workflow_version_hash: "v1".into(),
                dedupe_key_hash: None,
            },
        ),
        make_event(
            "inst-1",
            5,
            EventPayload::StepScheduled {
                workflow_id: "wf-1".into(),
                step_id: "step-1".into(),
                attempt: 1,
                fence: 0,
                execution_id: "exec-1".into(),
            },
        ),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should detect burst missing events 2-4");
    assert_eq!(
        err,
        ReplayError::SequenceGap {
            expected: 2,
            actual: 5,
            at_index: 1
        },
        "Burst gap should report expected=2 (first missing), actual=5 at index 1"
    );
}

#[test]
fn replay_succeeds_then_gap_at_end_reports_truncation() {
    let engine = ReplayEngine::new();
    // Valid sequence 1-3, then gap: 3 → 5 (event 4 missing at end)
    let events = [
        make_event(
            "inst-1",
            1,
            EventPayload::WorkflowStarted {
                workflow_id: "wf-1".into(),
                dag_topology: serde_json::json!({}),
                binary_hash: "h1".into(),
                workflow_version_hash: "v1".into(),
                dedupe_key_hash: None,
            },
        ),
        make_event(
            "inst-1",
            2,
            EventPayload::StepScheduled {
                workflow_id: "wf-1".into(),
                step_id: "step-1".into(),
                attempt: 1,
                fence: 0,
                execution_id: "exec-1".into(),
            },
        ),
        make_event(
            "inst-1",
            3,
            EventPayload::StepStarted {
                workflow_id: "wf-1".into(),
                step_id: "step-1".into(),
                started_at_ms: 2000,
            },
        ),
        make_event(
            "inst-1",
            5,
            EventPayload::StepCompleted {
                workflow_id: "wf-1".into(),
                step_id: "step-1".into(),
                completed_at_ms: 5000,
                attempt: 1,
                fence: 0,
                routing_projection: serde_json::json!({}),
                output_ref: None,
                output_hash: None,
                output: StepOutput::Null,
            },
        ),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should detect end-of-stream gap");
    assert_eq!(
        err,
        ReplayError::SequenceGap {
            expected: 4,
            actual: 5,
            at_index: 3
        },
        "End-of-stream gap should report expected=4, actual=5 at index 3"
    );
}
