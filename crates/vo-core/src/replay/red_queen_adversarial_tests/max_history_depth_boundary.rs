use super::*;
use vo_types::command_history::MAX_HISTORY_DEPTH;
use vo_types::state::LifecycleState;

#[test]
fn replay_handles_exactly_max_history_depth_events() {
    let engine = ReplayEngine::new();
    let mut events = Vec::new();

    events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

    for i in 2..=MAX_HISTORY_DEPTH {
        let payload = step_scheduled_payload("wf-1", &format!("step-{}", i));
        events.push(make_event("inst-1", i as u64, payload));
    }

    let result = engine
        .replay(&events)
        .expect("MAX_HISTORY_DEPTH events should replay");
    assert_eq!(result.events_applied, MAX_HISTORY_DEPTH);
    assert_eq!(result.final_state, Some(LifecycleState::StepScheduled));
}

#[test]
fn replay_handles_max_history_depth_plus_one_events() {
    let engine = ReplayEngine::new();
    let mut events = Vec::new();

    events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

    for i in 2..=(MAX_HISTORY_DEPTH + 1) {
        let payload = step_scheduled_payload("wf-1", &format!("step-{}", i));
        events.push(make_event("inst-1", i as u64, payload));
    }

    let result = engine
        .replay(&events)
        .expect("MAX_HISTORY_DEPTH+1 events should replay");
    assert_eq!(result.events_applied, MAX_HISTORY_DEPTH + 1);
}

#[test]
fn replay_handles_max_history_depth_with_failure_recovery() {
    let engine = ReplayEngine::new();
    let mut events = Vec::new();

    events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

    for i in 2..=(MAX_HISTORY_DEPTH / 2) {
        events.push(make_event(
            "inst-1",
            ((i - 1) * 4 + 1) as u64,
            step_scheduled_payload("wf-1", &format!("step-{}", i * 4)),
        ));
        events.push(make_event(
            "inst-1",
            ((i - 1) * 4 + 2) as u64,
            step_started_payload("wf-1", &format!("step-{}", i * 4)),
        ));
        events.push(make_event(
            "inst-1",
            ((i - 1) * 4 + 3) as u64,
            step_failed_payload("wf-1", &format!("step-{}", i * 4)),
        ));
        events.push(make_event(
            "inst-1",
            ((i - 1) * 4 + 4) as u64,
            instance_resumed_payload("wf-1"),
        ));
    }

    let result = engine
        .replay(&events)
        .expect("deep failure recovery should work");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}

#[test]
fn replay_stops_at_completed_before_max_history_depth() {
    let engine = ReplayEngine::new();
    let mut events = Vec::new();

    events.push(make_event("inst-1", 1, workflow_started_payload("wf-1")));

    for i in 2..=(MAX_HISTORY_DEPTH + 100) {
        let payload = if i == 4 {
            step_completed_payload("wf-1", "step-1")
        } else if i > 4 {
            step_scheduled_payload("wf-1", &format!("step-{}", i))
        } else {
            step_scheduled_payload("wf-1", &format!("step-{}", i))
        };
        events.push(make_event("inst-1", i as u64, payload));
    }

    let result = engine.replay(&events).expect("should stop at completed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}
