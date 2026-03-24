use super::*;
use bytes::Bytes;
use wtf_common::{ActivityId, EffectDeclaration};

fn transition(from: &str, event: &str, to: &str, effects: Vec<EffectDeclaration>) -> WorkflowEvent {
    WorkflowEvent::TransitionApplied {
        from_state: from.into(),
        event_name: event.into(),
        to_state: to.into(),
        effects,
    }
}

fn no_effect_transition(from: &str, event: &str, to: &str) -> WorkflowEvent {
    transition(from, event, to, vec![])
}

#[test]
fn new_state_starts_in_initial_state() {
    let state = FsmActorState::new("Pending");
    assert_eq!(state.current_state, "Pending");
}

#[test]
fn apply_transition_updates_current_state() {
    let state = FsmActorState::new("Pending");
    let event = no_effect_transition("Pending", "Authorize", "Authorized");
    let (next, _) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("apply");
    assert_eq!(next.current_state, "Authorized");
}

#[test]
fn apply_transition_adds_seq_to_applied_set() {
    let state = FsmActorState::new("Pending");
    let event = no_effect_transition("Pending", "Authorize", "Authorized");
    let (next, _) = apply_event(&state, &event, 42, ExecutionPhase::Live).expect("apply");
    assert!(next.applied_seq.contains(&42));
}

#[test]
fn apply_duplicate_seq_returns_already_applied() {
    let state = FsmActorState::new("Pending");
    let event = no_effect_transition("Pending", "Authorize", "Authorized");
    let (s1, _) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("first apply");
    let (s2, result) = apply_event(&s1, &event, 1, ExecutionPhase::Live).expect("second apply");
    assert!(matches!(result, ApplyResult::AlreadyApplied));
    assert_eq!(s2.current_state, "Authorized");
}

#[test]
fn replay_phase_returns_no_effects() {
    let effect = EffectDeclaration {
        effect_type: "CallPayment".into(),
        payload: Bytes::from_static(b"{}"),
    };
    let state = FsmActorState::new("Pending");
    let event = transition("Pending", "Charge", "Charged", vec![effect]);
    let (_, result) = apply_event(&state, &event, 1, ExecutionPhase::Replay).expect("apply");
    if let ApplyResult::Effects(e) = result { assert!(e.is_empty(), "replay should not return effects") }
}

#[test]
fn live_phase_returns_effects() {
    let effect = EffectDeclaration {
        effect_type: "CallPayment".into(),
        payload: Bytes::from_static(b"{}"),
    };
    let state = FsmActorState::new("Pending");
    let event = transition("Pending", "Charge", "Charged", vec![effect.clone()]);
    let (_, result) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("apply");
    match result {
        ApplyResult::Effects(e) => assert_eq!(e.len(), 1),
        _ => panic!("expected Effects"),
    }
}

#[test]
fn snapshot_taken_resets_events_since_snapshot() {
    let mut state = FsmActorState::new("Pending");
    state.events_since_snapshot = 99;
    let event = WorkflowEvent::SnapshotTaken {
        seq: 10,
        checksum: 0,
    };
    let (next, _) = apply_event(&state, &event, 11, ExecutionPhase::Replay).expect("apply");
    assert_eq!(next.events_since_snapshot, 0);
}

#[test]
fn activity_dispatched_adds_to_in_flight() {
    let state = FsmActorState::new("Authorized");
    let event = WorkflowEvent::ActivityDispatched {
        activity_id: "act-1".into(),
        activity_type: "charge".into(),
        payload: Bytes::new(),
        retry_policy: wtf_common::RetryPolicy::default(),
        attempt: 1,
    };
    let (next, _) = apply_event(&state, &event, 1, ExecutionPhase::Live).expect("apply");
    assert!(next.in_flight.contains_key(&ActivityId::new("act-1")));
}

#[test]
fn activity_completed_removes_from_in_flight() {
    let mut state = FsmActorState::new("Charged");
    state
        .in_flight
        .insert(ActivityId::new("act-1"), "charge".into());
    let event = WorkflowEvent::ActivityCompleted {
        activity_id: "act-1".into(),
        result: Bytes::from_static(b"ok"),
        duration_ms: 50,
    };
    let (next, _) = apply_event(&state, &event, 2, ExecutionPhase::Live).expect("apply");
    assert!(!next.in_flight.contains_key(&ActivityId::new("act-1")));
}

#[test]
fn multiple_transitions_accumulate_correctly() {
    let s0 = FsmActorState::new("Pending");
    let e1 = no_effect_transition("Pending", "Authorize", "Authorized");
    let e2 = no_effect_transition("Authorized", "Charge", "Charged");
    let e3 = no_effect_transition("Charged", "Fulfill", "Fulfilled");

    let (s1, _) = apply_event(&s0, &e1, 1, ExecutionPhase::Replay).expect("e1");
    let (s2, _) = apply_event(&s1, &e2, 2, ExecutionPhase::Replay).expect("e2");
    let (s3, _) = apply_event(&s2, &e3, 3, ExecutionPhase::Replay).expect("e3");

    assert_eq!(s3.current_state, "Fulfilled");
    assert_eq!(s3.applied_seq.len(), 3);
    assert_eq!(s3.events_since_snapshot, 3);
}

#[test]
fn fsm_definition_transition_returns_some_when_valid() {
    let mut def = FsmDefinition::new();
    def.add_transition("Pending", "Authorize", "Authorized", vec![]);
    let result = def.transition("Pending", "Authorize");
    assert!(result.is_some());
}

#[test]
fn fsm_definition_transition_returns_none_for_unknown_event() {
    let mut def = FsmDefinition::new();
    def.add_transition("Pending", "Authorize", "Authorized", vec![]);
    assert!(def.transition("Pending", "Bogus").is_none());
}

#[test]
fn duplicate_fsm_event_returns_none_from_new_state() {
    let mut def = FsmDefinition::new();
    def.add_transition("Pending", "Authorize", "Authorized", vec![]);
    let state = FsmActorState::new("Pending");
    let plan1 = plan_fsm_signal(&def, &state, "Authorize");
    assert!(plan1.is_some());
    let next = plan1.unwrap().next_state;
    assert!(plan_fsm_signal(&def, &next, "Authorize").is_none());
}
