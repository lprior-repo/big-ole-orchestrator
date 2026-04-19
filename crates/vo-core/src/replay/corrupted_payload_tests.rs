//! Corrupted payload detection tests for event replay (ve-xqp2f).
//!
//! Tests that the replay engine validates payload integrity and rejects
//! corrupted events: malformed JSON, missing required fields, type mismatches,
//! and truncation.

use super::test_helpers::*;
use super::{ReplayEngine, ReplayError, ReplayErrorKind};

// ── Malformed JSON ──────────────────────────────────────────────────────

#[test]
fn replay_rejects_malformed_json_payload() {
    let engine = ReplayEngine::new();
    let events = vec![make_event(
        "inst-1",
        1,
        serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1"}),
    )];

    // Valid event first
    let result = engine.replay(&events);
    assert!(result.is_ok());

    // Malformed JSON: not valid JSON at all
    let bad_event = vo_types::events::EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 2,
        timestamp_ms: 2000,
        payload: serde_json::from_str("<not json>").unwrap(), // JSON value, but nonsensical
        metadata: vo_types::events::EventMetadata::default(),
    };

    let events_with_bad = vec![events[0].clone(), bad_event];
    let result = engine.replay(&events_with_bad);
    assert!(result.is_err());
    match result.unwrap_err() {
        ReplayError::PayloadDecodeFailed { sequence, .. } => {
            assert_eq!(sequence, 2);
        }
        other => panic!("expected PayloadDecodeFailed, got {other:?}"),
    }
}

#[test]
fn replay_rejects_missing_type_field() {
    let engine = ReplayEngine::new();
    let events = vec![make_event(
        "inst-1",
        1,
        serde_json::json!({"workflow_id": "wf-1"}), // missing "type"
    )];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}

#[test]
fn replay_rejects_unknown_event_type() {
    let engine = ReplayEngine::new();
    let events = vec![make_event(
        "inst-1",
        1,
        serde_json::json!({"type": "FakeEvent", "data": 42}),
    )];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}

// ── Truncation: missing required fields ─────────────────────────────────

#[test]
fn replay_rejects_truncated_workflow_started() {
    let engine = ReplayEngine::new();
    let events = vec![make_event(
        "inst-1",
        1,
        serde_json::json!({"type": "WorkflowStarted"}), // missing workflow_id, binary_hash
    )];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}

#[test]
fn replay_rejects_truncated_step_completed() {
    let engine = ReplayEngine::new();
    let started = make_event("inst-1", 1, workflow_started_payload("wf-1"));
    let scheduled = make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1"));
    let step_started = make_event("inst-1", 3, step_started_payload("wf-1", "step-1"));
    // StepCompleted missing step_id, attempt, fence
    let truncated_completed = make_event(
        "inst-1",
        4,
        serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-1"}),
    );

    let events = vec![started, scheduled, step_started, truncated_completed];
    let result = engine.replay(&events);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}

// ── Type mismatch (wrong field types) ───────────────────────────────────

#[test]
fn replay_rejects_wrong_sequence_type() {
    let engine = ReplayEngine::new();
    // sequence field is a number, not a string — this is fine at envelope level.
    // But inside payload, if version field is wrong type, deserialization should fail.
    let events = vec![make_event(
        "inst-1",
        1,
        serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "wf-1",
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash",
            "version": "not-a-number" // version should be u32
        }),
    )];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}

// ── Error kind classification ───────────────────────────────────────────

#[test]
fn corrupted_payload_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = vec![make_event(
        "inst-1",
        1,
        serde_json::json!({"type": "UnknownType"}),
    )];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        ReplayErrorKind::Deterministic,
        "corrupted payload errors must be deterministic — no retry"
    );
}

#[test]
fn sequence_gap_error_is_deterministic() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 5, step_scheduled_payload("wf-1", "step-1")), // gap: 2,3,4 missing
    ];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ReplayErrorKind::Deterministic);
}

// ── Valid payloads after corruption (recovery) ──────────────────────────

#[test]
fn replay_succeeds_with_valid_payloads_after_removing_corrupt_event() {
    let engine = ReplayEngine::new();
    let events = vec![
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];

    let result = engine.replay(&events);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().events_applied, 4);
}

// ── Empty payload ───────────────────────────────────────────────────────

#[test]
fn replay_rejects_empty_json_object_payload() {
    let engine = ReplayEngine::new();
    let events = vec![make_event("inst-1", 1, serde_json::json!({}))];

    let result = engine.replay(&events);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}

#[test]
fn replay_rejects_null_payload() {
    let engine = ReplayEngine::new();
    let event = vo_types::events::EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::Value::Null,
        metadata: vo_types::events::EventMetadata::default(),
    };

    let result = engine.replay(&[event]);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ReplayError::PayloadDecodeFailed { .. }));
}
