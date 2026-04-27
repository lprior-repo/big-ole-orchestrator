//! BDD: Replay starts from compatible snapshot then events (ADR-016, ADR-027, ADR-035).
//!
//! Given a compatible snapshot exists at sequence N
//! When replay recovers the instance
//! Then state is restored from snapshot and only events after N are applied
//!
//! Required proof command:
//! cargo test -p vo-core given_snapshot_and_events_when_replay_runs_then_post_snapshot_events_are_applied

use vo_core::replay::ReplayEngine;
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::state::LifecycleState;

fn make_event(instance_id: &str, sequence: u64, payload: serde_json::Value) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn workflow_started_payload(workflow_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "binary_hash": "sha256abc",
        "workflow_version_hash": "wvhash123",
        "dedupe_key_hash": null,
        "version": 1
    })
}

fn step_scheduled_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": 1,
        "fence": 1,
        "execution_id": "exec-1",
        "version": 1
    })
}

fn step_started_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepStarted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

fn step_completed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
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

fn make_full_workflow_events() -> Vec<serde_json::Value> {
    vec![
        workflow_started_payload("wf-1"),
        step_scheduled_payload("wf-1", "step-1"),
        step_started_payload("wf-1", "step-1"),
        step_completed_payload("wf-1", "step-1"),
        step_scheduled_payload("wf-1", "step-2"),
        step_started_payload("wf-1", "step-2"),
        step_completed_payload("wf-1", "step-2"),
        step_scheduled_payload("wf-1", "step-3"),
        step_started_payload("wf-1", "step-3"),
        step_completed_payload("wf-1", "step-3"),
    ]
}

fn replay_all_events() -> (Option<LifecycleState>, usize) {
    let engine = ReplayEngine::new();
    let events: Vec<_> = make_full_workflow_events()
        .iter()
        .enumerate()
        .map(|(i, payload)| make_event("inst-1", (i + 1) as u64, payload.clone()))
        .collect();
    let result = engine.replay(&events, None).expect("replay should succeed");
    (result.final_state, result.events_applied)
}

fn replay_from_snapshot_point(
    snapshot_seq: u64,
) -> (Option<LifecycleState>, usize, usize, Option<LifecycleState>) {
    let all_events = make_full_workflow_events();

    let pre_snapshot: Vec<_> = all_events
        .iter()
        .take(snapshot_seq as usize)
        .enumerate()
        .map(|(i, payload)| make_event("inst-1", (i + 1) as u64, payload.clone()))
        .collect();

    let post_snapshot: Vec<_> = all_events
        .iter()
        .skip(snapshot_seq as usize)
        .enumerate()
        .map(|(i, payload)| make_event("inst-1", snapshot_seq + i as u64 + 1, payload.clone()))
        .collect();

    let engine = ReplayEngine::new();
    let pre_result = engine.replay(&pre_snapshot, None).expect("pre-snapshot replay should succeed");
    let snapshot_state = pre_result.final_state;
    let snapshot_events_applied = pre_result.events_applied;

    let post_engine = ReplayEngine::new();
    let post_result = post_engine
        .replay(&post_snapshot, snapshot_state)
        .expect("post-snapshot replay should succeed");

    (
        snapshot_state,
        snapshot_events_applied,
        post_result.events_applied,
        post_result.final_state,
    )
}

#[test]
fn given_snapshot_and_events_when_replay_runs_then_post_snapshot_events_are_applied() {
    let (full_state, full_applied) = replay_all_events();

    for snapshot_seq in 1..=9 {
        let (snapshot_state, pre_applied, post_applied, post_final_state) =
            replay_from_snapshot_point(snapshot_seq);

        assert_eq!(
            full_state, post_final_state,
            "Post-snapshot replay final state at seq {} should match full replay final state",
            snapshot_seq
        );

        let expected_post_events = make_full_workflow_events().len() - snapshot_seq as usize;
        assert_eq!(
            post_applied, expected_post_events,
            "Post-snapshot replay at seq {} should apply exactly {} events, but applied {}",
            snapshot_seq, expected_post_events, post_applied
        );

        assert_eq!(
            pre_applied + post_applied, full_applied,
            "Total events (pre:{} + post:{}) should equal full replay applied ({})",
            pre_applied, post_applied, full_applied
        );
    }
}

#[test]
fn given_snapshot_at_seq_3_when_replay_runs_then_only_events_4_to_10_applied() {
    let snapshot_seq = 3;
    let (_, _, post_applied, _) = replay_from_snapshot_point(snapshot_seq);

    let total_events = make_full_workflow_events().len();
    let expected_post = total_events - snapshot_seq as usize;

    assert_eq!(post_applied, expected_post);
    assert_eq!(post_applied, 7);
}

#[test]
fn given_snapshot_at_terminal_state_when_replay_runs_then_no_events_applied() {
    let full_events = make_full_workflow_events();

    let engine = ReplayEngine::new();
    let all_envelopes: Vec<_> = full_events
        .iter()
        .enumerate()
        .map(|(i, payload)| make_event("inst-1", (i + 1) as u64, payload.clone()))
        .collect();

    let full_result = engine.replay(&all_envelopes, None).expect("replay should succeed");
    assert_eq!(full_result.final_state, Some(LifecycleState::Completed));

    let snapshot_engine = ReplayEngine::new();
    let snapshot_result = snapshot_engine.replay(&all_envelopes, None).expect("snapshot replay should succeed");
    assert_eq!(snapshot_result.final_state, Some(LifecycleState::Completed));

    let post_engine = ReplayEngine::new();
    let post_result = post_engine.replay(&[], None).expect("post-snapshot replay should succeed");
    assert_eq!(post_result.final_state, Some(LifecycleState::Completed));
    assert_eq!(post_result.events_applied, 0);
}

#[test]
fn given_empty_post_snapshot_events_when_replay_runs_then_state_unchanged() {
    let snapshot_seq = 10;
    let all_events = make_full_workflow_events();

    let pre_snapshot: Vec<_> = all_events
        .iter()
        .take(snapshot_seq as usize)
        .enumerate()
        .map(|(i, payload)| make_event("inst-1", (i + 1) as u64, payload.clone()))
        .collect();

    let engine = ReplayEngine::new();
    let snapshot_result = engine.replay(&pre_snapshot, None).expect("snapshot replay should succeed");
    assert_eq!(snapshot_result.final_state, Some(LifecycleState::Completed));

    let post_engine = ReplayEngine::new();
    let post_result = post_engine.replay(&[], None).expect("empty post-snapshot replay should succeed");
    assert_eq!(post_result.final_state, Some(LifecycleState::Completed));
    assert_eq!(snapshot_result.final_state, post_result.final_state);
}
