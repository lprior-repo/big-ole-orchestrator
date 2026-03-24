// Integration test: FSM crash-and-replay (ADR-015 crash window handling).
// Validates that FSM event handling and state reconstruction works correctly.
//
// This test module verifies the core FSM logic without requiring live NATS:
// - Event application to FSM state
// - State reconstruction from event sequence
// - Transition handling
// - Snapshot event handling
//
// The full crash-and-replay integration test (with actual NATS crash simulation)
// requires a separate e2e test runner with process management.
//
// Run with: cargo test -p wtf-actor --test fsm_crash_replay -- --test-threads=1

use bytes::Bytes;

use wtf_actor::fsm::{apply_event, ExecutionPhase, FsmActorState};
use wtf_common::WorkflowEvent;

fn assert_event_count(events: &[WorkflowEvent], expected: usize) -> Result<(), String> {
    if events.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "Event count mismatch: expected {}, got {}",
            expected,
            events.len()
        ))
    }
}

fn assert_no_duplicate_transitions(events: &[WorkflowEvent]) -> Result<(), String> {
    let transitions: Vec<(&str, &str, &str)> = events
        .iter()
        .filter_map(|e| {
            if let WorkflowEvent::TransitionApplied {
                from_state,
                event_name,
                to_state,
                ..
            } = e
            {
                Some((from_state.as_str(), event_name.as_str(), to_state.as_str()))
            } else {
                None
            }
        })
        .collect();

    for (i, t1) in transitions.iter().enumerate() {
        for (j, t2) in transitions.iter().enumerate() {
            if i != j && t1 == t2 {
                return Err(format!(
                    "Duplicate TransitionApplied detected: ({}, {}, {}) appears multiple times",
                    t1.0, t1.1, t1.2
                ));
            }
        }
    }
    Ok(())
}

fn assert_event_types(events: &[WorkflowEvent], expected_types: &[&str]) -> Result<(), String> {
    if events.len() != expected_types.len() {
        return Err(format!(
            "Event count mismatch: expected {} events, got {}",
            expected_types.len(),
            events.len()
        ));
    }

    for (i, (event, expected_type)) in events.iter().zip(expected_types.iter()).enumerate() {
        let type_name = match event {
            WorkflowEvent::InstanceStarted { .. } => "InstanceStarted",
            WorkflowEvent::TransitionApplied { .. } => "TransitionApplied",
            WorkflowEvent::ActivityDispatched { .. } => "ActivityDispatched",
            WorkflowEvent::ActivityCompleted { .. } => "ActivityCompleted",
            WorkflowEvent::ActivityFailed { .. } => "ActivityFailed",
            WorkflowEvent::SnapshotTaken { .. } => "SnapshotTaken",
            WorkflowEvent::InstanceCompleted { .. } => "InstanceCompleted",
            WorkflowEvent::InstanceFailed { .. } => "InstanceFailed",
            WorkflowEvent::InstanceCancelled { .. } => "InstanceCancelled",
            WorkflowEvent::ActivityHeartbeat { .. } => "ActivityHeartbeat",
            WorkflowEvent::TimerScheduled { .. } => "TimerScheduled",
            WorkflowEvent::TimerFired { .. } => "TimerFired",
            WorkflowEvent::TimerCancelled { .. } => "TimerCancelled",
            WorkflowEvent::SignalReceived { .. } => "SignalReceived",
            WorkflowEvent::ChildStarted { .. } => "ChildStarted",
            WorkflowEvent::ChildCompleted { .. } => "ChildCompleted",
            WorkflowEvent::ChildFailed { .. } => "ChildFailed",
            WorkflowEvent::NowSampled { .. } => "NowSampled",
            WorkflowEvent::RandomSampled { .. } => "RandomSampled",
        };

        if type_name != *expected_type {
            return Err(format!(
                "Event {} type mismatch: expected '{}', got '{}'",
                i + 1,
                expected_type,
                type_name
            ));
        }
    }

    Ok(())
}

fn create_instance_started_event(instance_id: &str, workflow_type: &str) -> WorkflowEvent {
    WorkflowEvent::InstanceStarted {
        instance_id: instance_id.to_string(),
        workflow_type: workflow_type.to_string(),
        input: Bytes::from_static(b"{\"amount\":100}"),
    }
}

fn create_transition_event(from_state: &str, event_name: &str, to_state: &str) -> WorkflowEvent {
    WorkflowEvent::TransitionApplied {
        from_state: from_state.to_string(),
        event_name: event_name.to_string(),
        to_state: to_state.to_string(),
        effects: vec![],
    }
}

fn create_snapshot_event(seq: u64, checksum: u32) -> WorkflowEvent {
    WorkflowEvent::SnapshotTaken { seq, checksum }
}

#[test]
fn fsm_apply_event_transitions_to_authorized_state() {
    let initial_state = FsmActorState::new("Initialized");
    let event = create_transition_event("Created", "authorize", "Authorized");

    let (new_state, result) = apply_event(&initial_state, &event, 1, ExecutionPhase::Live)
        .expect("apply_event should succeed");

    assert_eq!(new_state.current_state, "Authorized");
    assert!(matches!(result, wtf_actor::fsm::ApplyResult::Effects(_)));
}

#[test]
fn fsm_replay_reconstructs_state_from_events() {
    let events = [create_instance_started_event("test-01", "checkout"),
        create_transition_event("Created", "authorize", "Authorized")];

    let mut fsm_state = FsmActorState::new("Initialized");

    for (seq, event) in events.iter().enumerate() {
        let seq = seq as u64 + 1;
        let result = apply_event(&fsm_state, event, seq, ExecutionPhase::Live)
            .expect("apply_event should succeed");
        fsm_state = result.0;
    }

    assert_eq!(
        fsm_state.current_state, "Authorized",
        "FSM should reconstruct to Authorized state after replay"
    );
}

#[test]
fn fsm_replay_handles_multiple_transitions() {
    let events = [create_instance_started_event("test-02", "checkout"),
        create_transition_event("Created", "authorize", "Authorized"),
        create_transition_event("Authorized", "fulfill", "Fulfilled")];

    let mut fsm_state = FsmActorState::new("Initialized");

    for (seq, event) in events.iter().enumerate() {
        let seq = seq as u64 + 1;
        let result = apply_event(&fsm_state, event, seq, ExecutionPhase::Live)
            .expect("apply_event should succeed");
        fsm_state = result.0;
    }

    assert_eq!(
        fsm_state.current_state, "Fulfilled",
        "FSM should reconstruct to Fulfilled state after replay"
    );
}

#[test]
fn fsm_detects_already_applied_sequence() {
    let initial_state = FsmActorState::new("Initialized");
    let event = create_transition_event("Created", "authorize", "Authorized");

    let (state1, _) = apply_event(&initial_state, &event, 1, ExecutionPhase::Live)
        .expect("first apply should succeed");

    let (state2, result) =
        apply_event(&state1, &event, 1, ExecutionPhase::Live).expect("second apply should succeed");

    assert_eq!(state2.current_state, "Authorized");
    assert!(matches!(
        result,
        wtf_actor::fsm::ApplyResult::AlreadyApplied
    ));
}

#[test]
fn snapshot_taken_event_recorded_correctly() {
    let events = vec![
        create_instance_started_event("test-03", "checkout"),
        create_transition_event("Created", "authorize", "Authorized"),
        create_snapshot_event(2, 0xDEADBEEF),
    ];

    assert_event_count(&events, 3).expect("should have 3 events");
    assert_event_types(
        &events,
        &["InstanceStarted", "TransitionApplied", "SnapshotTaken"],
    )
    .expect("event types should match");

    if let WorkflowEvent::SnapshotTaken { seq, checksum } = &events[2] {
        assert_eq!(*seq, 2, "Snapshot seq should be 2");
        assert_eq!(*checksum, 0xDEADBEEF, "Snapshot checksum should match");
    } else {
        panic!("Third event should be SnapshotTaken");
    }
}

#[test]
fn no_duplicate_transitions_detected() {
    let events = vec![
        create_instance_started_event("test-04", "checkout"),
        create_transition_event("Created", "authorize", "Authorized"),
        create_transition_event("Authorized", "fulfill", "Fulfilled"),
    ];

    assert_no_duplicate_transitions(&events).expect("no duplicates");
}

#[test]
fn duplicate_transitions_detected_when_same_transition_applied_twice() {
    let events = vec![
        create_instance_started_event("test-05", "checkout"),
        create_transition_event("Created", "authorize", "Authorized"),
        create_transition_event("Created", "authorize", "Authorized"),
    ];

    let result = assert_no_duplicate_transitions(&events);
    assert!(result.is_err(), "duplicate should be detected");
}

#[test]
fn transition_event_structure_validation() {
    let event = create_transition_event("Created", "authorize", "Authorized");

    if let WorkflowEvent::TransitionApplied {
        from_state,
        event_name,
        to_state,
        effects,
    } = &event
    {
        assert_eq!(from_state, "Created");
        assert_eq!(event_name, "authorize");
        assert_eq!(to_state, "Authorized");
        assert!(effects.is_empty());
    } else {
        panic!("Event should be TransitionApplied");
    }
}

#[test]
fn instance_started_event_structure() {
    let event = create_instance_started_event("inst-123", "checkout");

    if let WorkflowEvent::InstanceStarted {
        instance_id,
        workflow_type,
        input,
    } = &event
    {
        assert_eq!(instance_id, "inst-123");
        assert_eq!(workflow_type, "checkout");
        assert_eq!(input.as_ref(), b"{\"amount\":100}");
    } else {
        panic!("Event should be InstanceStarted");
    }
}
