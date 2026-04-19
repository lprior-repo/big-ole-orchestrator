//! Tests for ReplayPosition tracking in the replay engine.

use super::engine::ReplayEngine;
use super::test_helpers::*;

#[test]
fn replay_result_includes_position_with_last_applied_sequence() {
    let engine = ReplayEngine::new();
    let events = [make_event("inst-1", 1, workflow_started_payload("wf-1"))];
    let result = engine.replay(&events).expect("replay should succeed");

    assert_eq!(result.position.last_applied_sequence, Some(1));
}

#[test]
fn replay_position_includes_last_applied_timestamp() {
    let engine = ReplayEngine::new();
    let events = [make_event("inst-1", 1, workflow_started_payload("wf-1"))];
    let result = engine.replay(&events).expect("replay should succeed");

    assert_eq!(result.position.last_applied_timestamp_ms, Some(1000));
}
