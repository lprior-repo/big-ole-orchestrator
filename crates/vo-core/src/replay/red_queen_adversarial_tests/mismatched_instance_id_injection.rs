use super::*;

#[test]
fn replay_rejects_instance_id_switch_at_sequence_2() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-2", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail at instance mismatch");
    assert!(matches!(
        err,
        ReplayError::InstanceMismatch {
            expected: _,
            actual: _
        }
    ));
}

#[test]
fn replay_rejects_instance_id_switch_at_sequence_3() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-2", 3, step_started_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail at instance mismatch");
    assert!(matches!(
        err,
        ReplayError::InstanceMismatch {
            expected: _,
            actual: _
        }
    ));
}

#[test]
fn replay_rejects_instance_id_switch_at_sequence_4() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-3", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail at instance mismatch");
    assert!(matches!(
        err,
        ReplayError::InstanceMismatch {
            expected: _,
            actual: _
        }
    ));
}

#[test]
fn replay_rejects_whitespace_instance_id_variant() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1 ", 3, step_started_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail - trailing space");
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}

#[test]
fn replay_rejects_case_mismatch_instance_id() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("Inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail - case mismatch");
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}

#[test]
fn replay_rejects_empty_instance_id_at_sequence_2() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail - empty instance_id");
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}

#[test]
fn replay_rejects_instance_id_change_after_failure_recovery() {
    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_failed_payload("wf-1", "step-1")),
        make_event("inst-1", 5, instance_resumed_payload("wf-1")),
        make_event("inst-2", 6, step_scheduled_payload("wf-1", "step-1")),
    ];
    let err = engine
        .replay(&events)
        .expect_err("should fail at instance mismatch after recovery");
    assert!(matches!(err, ReplayError::InstanceMismatch { .. }));
}
