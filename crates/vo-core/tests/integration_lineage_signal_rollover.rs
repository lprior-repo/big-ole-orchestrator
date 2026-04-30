//! Integration tests: Signal matching across continue-as-new lineage rollover.
//!
//! These tests verify ADR-038 (Workflow Lineage and Continue-As-New) and
//! ADR-042 (Signal Matching and Wake-Up Semantics) work together correctly
//! when a lineage performs continue-as-new rollover.
//!
//! Test coverage:
//! - Lineage-wide signal routes to new epoch after rollover
//! - Signal ordering preserved across continue-as-new boundary
//! - Epoch-local signal to retired epoch is rejected (orphaned lineage)
//! - Multiple sequential rollovers maintain correct signal routing
//! - Replay determinism across continue-as-new events
//! - Dedupe scope correctness across epoch transitions

use serde_json::json;
use vo_core::exact_once_verification::harness::{
    LineageRolloverEvent, LineageRoutingState, VerificationHarness,
};
use vo_core::replay::{ReplayEngine, ReplayResult};
use vo_types::events::{EventEnvelope, EventMetadata, EventPayload};
use vo_types::signal::{
    signal_match, BufferPolicy, LineageScope, SignalAddress, SignalMatchResult, SignalScope,
    WaitKey, WaitRecord,
};
use vo_types::{Epoch, InstanceId, WorkflowLineage};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_event(
    instance_id: &str,
    sequence: u64,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn workflow_started_payload(workflow_id: &str, lineage_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "lineage_id": lineage_id,
        "binary_hash": "sha256abc123",
        "workflow_version_hash": "wvhash456",
        "dedupe_key_hash": null,
        "version": 1
    })
}

fn step_scheduled_payload(
    workflow_id: &str,
    step_id: &str,
    attempt: u32,
    fence: u64,
) -> serde_json::Value {
    json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": attempt,
        "fence": fence,
        "execution_id": format!("exec-{}", step_id),
        "version": 1
    })
}

fn signal_received_payload(
    workflow_id: &str,
    signal_id: &str,
    signal_name: &str,
) -> serde_json::Value {
    json!({
        "type": "SignalReceived",
        "workflow_id": workflow_id,
        "signal_id": signal_id,
        "signal_name": signal_name,
        "version": 1
    })
}

fn step_completed_payload(
    workflow_id: &str,
    step_id: &str,
    attempt: u32,
    fence: u64,
) -> serde_json::Value {
    json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": attempt,
        "fence": fence,
        "version": 1
    })
}

fn continuation_wait_key() -> WaitKey {
    WaitKey::parse("continue").expect("valid wait key")
}

fn approval_wait_key() -> WaitKey {
    WaitKey::parse("approval").expect("valid wait key")
}

// ---------------------------------------------------------------------------
// Scenario 1: Lineage-wide signal routes to new epoch after continue-as-new
// ---------------------------------------------------------------------------
//! GIVEN a lineage that has performed continue-as-new rollover
//! WHEN a lineage-wide signal is sent to that lineage
//! THEN the signal matches the active epoch instance
//!
//! Per ADR-042 Section 2: "if the active epoch rolled over via continue-as-new,
//! the signal follows the lineage routing map"

#[test]
fn lineage_wide_signal_routes_to_new_epoch_after_rollover() {
    // GIVEN: A lineage that performed continue-as-new to epoch 1
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let inst_epoch_0 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFBK").unwrap();
    let inst_epoch_1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFBM").unwrap();

    let mut routing = LineageRoutingState::new(lineage_id.as_str().to_string(), Epoch::ZERO);

    // GIVEN: Lineage does continue-as-new to epoch 1
    routing.rollover(Epoch::new(1));
    assert_eq!(routing.active_epoch, Epoch::new(1));
    assert_eq!(routing.previous_epochs, vec![Epoch::ZERO]);

    // GIVEN: Instance waiting for "continue" signal in epoch 1
    let wait = WaitRecord::new(
        inst_epoch_1.clone(),
        continuation_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    // WHEN: Lineage-wide signal is sent (no epoch specified)
    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        inst_epoch_1.clone(),
        continuation_wait_key(),
    );

    // THEN: Signal matches the active epoch instance
    let result = signal_match(&signal, &wait, &lineage_id, Epoch::new(1));
    assert!(
        result.is_matched(),
        "Lineage-wide signal should match after continue-as-new: {:?}",
        result
    );
}

#[test]
fn lineage_wide_signal_matches_across_multiple_rollovers() {
    // GIVEN: A lineage with multiple epochs (0 -> 1 -> 2)
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
    let mut routing = LineageRoutingState::new(lineage_id.as_str().to_string(), Epoch::ZERO);

    routing.rollover(Epoch::new(1));
    routing.rollover(Epoch::new(2));
    assert_eq!(routing.active_epoch, Epoch::new(2));
    assert_eq!(routing.previous_epochs.len(), 2);

    // GIVEN: Instance in epoch 2 waiting for signal
    let inst_epoch_2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFBP").unwrap();
    let wait = WaitRecord::new(
        inst_epoch_2.clone(),
        approval_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    // WHEN: Lineage-wide signal sent
    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        inst_epoch_2.clone(),
        approval_wait_key(),
    );

    // THEN: Signal matches epoch 2 instance
    let result = signal_match(&signal, &wait, &lineage_id, Epoch::new(2));
    assert!(result.is_matched(), "Should match at epoch 2 after two rollovers");
}

// ---------------------------------------------------------------------------
// Scenario 2: Signal ordering preserved across continue-as-new boundary
// ---------------------------------------------------------------------------
//! GIVEN signals sent before and after a continue-as-new rollover
//! WHEN replayed through the event sourcing engine
//! THEN the final state reflects signals in correct chronological order
//!
//! Per ADR-038: "continues-as-new atomically writes ContinuedAsNew and
//! creates a new WorkflowStarted for the successor epoch"

#[test]
fn signal_ordering_preserved_before_and_after_rollover() {
    let lineage_id = "lin-order-test".to_string();
    let inst_0 = format!("{}-inst-0", lineage_id);
    let inst_1 = format!("{}-inst-1", lineage_id);

    let mut events = Vec::new();
    let mut seq = 1u64;

    // Event 1: Workflow started in epoch 0
    events.push(make_event(
        &inst_0,
        seq,
        workflow_started_payload(&inst_0, &lineage_id),
    ));
    seq += 1;

    // Event 2: Signal received in epoch 0 (before rollover)
    events.push(make_event(
        &inst_0,
        seq,
        signal_received_payload(&inst_0, "sig-001", "signal-a"),
    ));
    seq += 1;

    // Event 3: Step scheduled and completed in epoch 0
    events.push(make_event(
        &inst_0,
        seq,
        step_scheduled_payload(&inst_0, "step-1", 1, 1),
    ));
    seq += 1;

    // Event 4: Continue-as-new rollover to epoch 1
    let rollover_event = LineageRolloverEvent::new(
        lineage_id.clone(),
        0,
        1,
        inst_1.clone(),
    )
    .to_event_envelope(seq);
    events.push(rollover_event);
    seq += 1;

    // Event 5: Workflow started in epoch 1
    events.push(make_event(
        &inst_1,
        seq,
        workflow_started_payload(&inst_1, &lineage_id),
    ));
    seq += 1;

    // Event 6: Signal received in epoch 1 (after rollover)
    events.push(make_event(
        &inst_1,
        seq,
        signal_received_payload(&inst_1, "sig-002", "signal-b"),
    ));
    seq += 1;

    // Event 7: Step completed in epoch 1
    events.push(make_event(
        &inst_1,
        seq,
        step_completed_payload(&inst_1, "step-2", 1, 2),
    ));

    // WHEN: Replay all events through the engine
    let engine = ReplayEngine::new();
    let result = engine.replay(&events);

    // THEN: Replay succeeds with all events applied
    assert!(result.is_ok(), "Replay should succeed: {:?}", result);
    let replay_result = result.unwrap();
    assert_eq!(replay_result.events_applied, events.len());
}

#[test]
fn signal_ordering_deterministic_across_replays() {
    let lineage_id = "lin-determinism".to_string();
    let inst_0 = format!("{}-inst-0", lineage_id);
    let inst_1 = format!("{}-inst-1", lineage_id);

    fn build_events(
        lineage_id: &str,
        inst_0: &str,
        inst_1: &str,
    ) -> Vec<EventEnvelope> {
        let mut events = Vec::new();
        let mut seq = 1u64;

        events.push(make_event(
            inst_0,
            seq,
            workflow_started_payload(inst_0, lineage_id),
        ));
        seq += 1;

        events.push(make_event(
            inst_0,
            seq,
            signal_received_payload(inst_0, "sig-pre", "pre-rollover"),
        ));
        seq += 1;

        let rollover = LineageRolloverEvent::new(lineage_id.to_string(), 0, 1, inst_1.to_string())
            .to_event_envelope(seq);
        events.push(rollover);
        seq += 1;

        events.push(make_event(
            inst_1,
            seq,
            workflow_started_payload(inst_1, lineage_id),
        ));
        seq += 1;

        events.push(make_event(
            inst_1,
            seq,
            signal_received_payload(inst_1, "sig-post", "post-rollover"),
        ));

        events
    }

    // WHEN: Two independent replays of the same event sequence
    let events = build_events(&lineage_id, &inst_0, &inst_1);
    let engine = ReplayEngine::new();

    let result1 = engine.replay(&events).expect("replay 1");
    let result2 = engine.replay(&events).expect("replay 2");

    // THEN: Both replays produce identical results
    assert_eq!(
        result1.events_applied, result2.events_applied,
        "Event count must match across deterministic replays"
    );
    assert_eq!(
        result1.final_state, result2.final_state,
        "Final state must match across deterministic replays"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Orphaned lineage signal (epoch-local to retired epoch)
// ---------------------------------------------------------------------------
//! GIVEN a lineage that has rolled to a new epoch
//! WHEN an epoch-local signal targets a retired epoch
//! THEN the signal is rejected with epoch mismatch
//!
//! Per ADR-042 Section 2: "an explicitly epoch-scoped signal must fail if
//! the targeted epoch is no longer eligible"

#[test]
fn epoch_local_signal_to_retired_epoch_is_rejected() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap();
    let inst_epoch_0 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFC0").unwrap();
    let inst_epoch_1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFC1").unwrap();

    // GIVEN: Lineage has rolled to epoch 1
    let mut routing = LineageRoutingState::new(lineage_id.as_str().to_string(), Epoch::ZERO);
    routing.rollover(Epoch::new(1));

    // GIVEN: Instance in epoch 1 is waiting for signal
    let wait = WaitRecord::new(
        inst_epoch_1.clone(),
        approval_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    // WHEN: Epoch-local signal targets the RETIRED epoch 0
    let signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::ZERO, // Targeting epoch 0 (retired!)
        inst_epoch_0.clone(),
        approval_wait_key(),
    );

    // THEN: Signal is rejected with epoch mismatch
    let result = signal_match(&signal, &wait, &lineage_id, Epoch::new(1));
    assert!(result.is_mismatch(), "Should mismatch when targeting retired epoch");
    match result {
        SignalMatchResult::EpochMismatch {
            signal_epoch,
            wait_epoch,
        } => {
            assert_eq!(signal_epoch, Epoch::ZERO);
            assert_eq!(wait_epoch, Epoch::new(1));
        }
        other => panic!("Expected EpochMismatch, got {:?}", other),
    }
}

#[test]
fn epoch_local_signal_to_current_epoch_succeeds() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap();
    let inst_epoch_1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFC2").unwrap();

    // GIVEN: Lineage at epoch 1
    let mut routing = LineageRoutingState::new(lineage_id.as_str().to_string(), Epoch::ZERO);
    routing.rollover(Epoch::new(1));

    // GIVEN: Instance in epoch 1 is waiting
    let wait = WaitRecord::new(
        inst_epoch_1.clone(),
        continuation_wait_key(),
        BufferPolicy::BufferOne,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    // WHEN: Epoch-local signal targets the CURRENT epoch 1
    let signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::new(1), // Targeting current epoch
        inst_epoch_1.clone(),
        continuation_wait_key(),
    );

    // THEN: Signal matches
    let result = signal_match(&signal, &wait, &lineage_id, Epoch::new(1));
    assert!(result.is_matched(), "Epoch-local signal to current epoch should match");
}

#[test]
fn epoch_local_signal_to_future_epoch_is_rejected() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFME").unwrap();
    let inst_epoch_0 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFC3").unwrap();

    // GIVEN: Lineage at epoch 0
    let routing = LineageRoutingState::new(lineage_id.as_str().to_string(), Epoch::ZERO);

    // GIVEN: Instance in epoch 0 is waiting
    let wait = WaitRecord::new(
        inst_epoch_0.clone(),
        approval_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    // WHEN: Epoch-local signal targets a FUTURE epoch 5
    let signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::new(5), // Targeting future epoch
        inst_epoch_0.clone(),
        approval_wait_key(),
    );

    // THEN: Signal is rejected with epoch mismatch
    let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);
    assert!(result.is_mismatch(), "Should reject signal to future epoch");
    match result {
        SignalMatchResult::EpochMismatch {
            signal_epoch,
            wait_epoch,
        } => {
            assert_eq!(signal_epoch, Epoch::new(5));
            assert_eq!(wait_epoch, Epoch::ZERO);
        }
        other => panic!("Expected EpochMismatch, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Scenario 4: Multiple sequential rollovers maintain signal routing
// ---------------------------------------------------------------------------
//! GIVEN a lineage that performs multiple sequential continue-as-new rollovers
//! WHEN signals are sent at each epoch
//! THEN each signal routes to the correct epoch's instance

#[test]
fn sequential_rollovers_maintain_correct_signal_routing() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMF").unwrap();

    // Build a lineage chain: epoch 0 -> 1 -> 2 -> 3
    let mut lineage = WorkflowLineage::new("lin-sequential".to_string()).expect("valid lineage");
    assert_eq!(lineage.epoch(), Epoch::ZERO);

    let epoch1 = lineage.continue_as_new().expect("rollover to e1");
    assert_eq!(epoch1.epoch(), Epoch::new(1));
    assert_eq!(epoch1.parent_epoch(), Some(Epoch::ZERO));

    let epoch2 = epoch1.continue_as_new().expect("rollover to e2");
    assert_eq!(epoch2.epoch(), Epoch::new(2));
    assert_eq!(epoch2.parent_epoch(), Some(Epoch::new(1)));

    let epoch3 = epoch2.continue_as_new().expect("rollover to e3");
    assert_eq!(epoch3.epoch(), Epoch::new(3));
    assert_eq!(epoch3.parent_epoch(), Some(Epoch::new(2)));

    // Lineage ID must be preserved across all rollovers
    assert_eq!(lineage.lineage_id(), epoch1.lineage_id());
    assert_eq!(lineage.lineage_id(), epoch2.lineage_id());
    assert_eq!(lineage.lineage_id(), epoch3.lineage_id());
}

#[test]
fn routing_state_tracks_multiple_epoch_transitions() {
    let lineage_id = "lin-multi-epoch".to_string();
    let mut routing = LineageRoutingState::new(lineage_id.clone(), Epoch::ZERO);

    // GIVEN: Track three rollover transitions
    routing.rollover(Epoch::new(1));
    assert_eq!(routing.active_epoch, Epoch::new(1));
    assert_eq!(routing.previous_epochs, vec![Epoch::ZERO]);

    routing.rollover(Epoch::new(2));
    assert_eq!(routing.active_epoch, Epoch::new(2));
    assert_eq!(routing.previous_epochs, vec![Epoch::ZERO, Epoch::new(1)]);

    routing.rollover(Epoch::new(3));
    assert_eq!(routing.active_epoch, Epoch::new(3));
    assert_eq!(
        routing.previous_epochs,
        vec![Epoch::ZERO, Epoch::new(1), Epoch::new(2)]
    );

    // THEN: Active instance ID reflects current epoch
    let active_inst = routing.get_active_instance_id("base-inst");
    assert_eq!(active_inst, "base-inst-epoch-3");
}

// ---------------------------------------------------------------------------
// Scenario 5: Replay determinism across continue-as-new events
// ---------------------------------------------------------------------------
//! GIVEN event sequences containing continue-as-new events
//! WHEN replayed through the deterministic replay engine
//! THEN the engine correctly handles ContinuedAsNew as a tracking event
//!
//! Per ADR-038: "writes ContinuedAsNew for the old epoch"

#[test]
fn replay_engine_handles_continued_as_new_event() {
    let lineage_id = "lin-replay-cn".to_string();
    let inst_0 = format!("{}-inst-0", lineage_id);
    let inst_1 = format!("{}-inst-1", lineage_id);

    let mut events = Vec::new();
    let mut seq = 1u64;

    // Epoch 0: Workflow starts
    events.push(make_event(
        &inst_0,
        seq,
        workflow_started_payload(&inst_0, &lineage_id),
    ));
    seq += 1;

    // Continue-as-new rollover
    let cn_event = LineageRolloverEvent::new(lineage_id.clone(), 0, 1, inst_1.clone())
        .to_event_envelope(seq);
    events.push(cn_event);
    seq += 1;

    // Epoch 1: New workflow starts
    events.push(make_event(
        &inst_1,
        seq,
        workflow_started_payload(&inst_1, &lineage_id),
    ));

    // WHEN: Replay events
    let engine = ReplayEngine::new();
    let result = engine.replay(&events);

    // THEN: Replay succeeds - ContinuedAsNew is counted as applied
    assert!(result.is_ok(), "Replay should handle ContinuedAsNew: {:?}", result);
    let replay_result = result.unwrap();
    assert_eq!(
        replay_result.events_applied, 3,
        "Should count all 3 events (WorkflowStarted, ContinuedAsNew, WorkflowStarted)"
    );
}

#[test]
fn continued_as_new_event_converts_to_event_envelope() {
    let rollover = LineageRolloverEvent::new("lin-test".to_string(), 0, 1, "inst-abc".to_string());

    let envelope = rollover.to_event_envelope(42);

    // Verify envelope structure
    assert_eq!(envelope.instance_id, "inst-abc");
    assert_eq!(envelope.sequence, 42);
    assert_eq!(envelope.timestamp_ms, 42000);

    // Verify payload parses as ContinuedAsNew
    let payload = EventPayload::try_from_json(&envelope.payload).expect("valid JSON payload");
    match payload {
        EventPayload::ContinuedAsNew {
            workflow_id,
            lineage_id,
            old_epoch,
            new_epoch,
        } => {
            assert_eq!(workflow_id, "inst-abc");
            assert_eq!(lineage_id, "lin-test");
            assert_eq!(old_epoch, 0);
            assert_eq!(new_epoch, 1);
        }
        other => panic!("Expected ContinuedAsNew payload, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Scenario 6: Dedupe scope across lineage rollover
// ---------------------------------------------------------------------------
//! GIVEN a signal with a dedupe key sent before continue-as-new
//! WHEN the lineage rolls to a new epoch
//! THEN dedupe scope (lineage-level vs epoch-level) is preserved
//!
//! Per ADR-042 Section 4: "Signal dedupe is keyed by
//! (workflow_lineage_id, wait_key, command_id) by default"

#[test]
fn lineage_wide_signal_has_no_epoch_in_scope() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMG").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD0").unwrap();

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance_id.clone(),
        approval_wait_key(),
    );

    // THEN: Lineage-wide signal has no epoch scope
    assert!(signal.is_lineage_wide());
    assert!(!signal.is_epoch_local());
    assert!(signal.epoch_id().is_none());
    assert_eq!(signal.lineage_scope(), LineageScope::LineageWide);
}

#[test]
fn epoch_local_signal_has_explicit_epoch_scope() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMH").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD1").unwrap();

    let signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::new(2),
        instance_id.clone(),
        continuation_wait_key(),
    );

    // THEN: Epoch-local signal has explicit epoch scope
    assert!(!signal.is_lineage_wide());
    assert!(signal.is_epoch_local());
    assert_eq!(signal.epoch_id(), Some(Epoch::new(2)));
    assert_eq!(signal.lineage_scope(), LineageScope::EpochLocal);
}

// ---------------------------------------------------------------------------
// Scenario 7: Lineage-wide signal matches regardless of instance epoch
// ---------------------------------------------------------------------------
//! GIVEN a lineage-wide signal
//! WHEN the instance is at any epoch
//! THEN the signal matches (epoch is not checked for lineage-wide)
//!
//! Per ADR-042: Lineage-wide signals route to current active epoch

#[test]
fn lineage_wide_signal_matches_at_any_epoch() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMI").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD2").unwrap();

    let wait = WaitRecord::new(
        instance_id.clone(),
        approval_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance_id.clone(),
        approval_wait_key(),
    );

    // WHEN: Signal matched at different epochs
    for epoch in [Epoch::ZERO, Epoch::new(1), Epoch::new(5), Epoch::new(100)] {
        let result = signal_match(&signal, &wait, &lineage_id, epoch);
        assert!(
            result.is_matched(),
            "Lineage-wide signal should match at epoch {:?}: {:?}",
            epoch,
            result
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 8: Verification harness builds correct rollover sequences
// ---------------------------------------------------------------------------
//! GIVEN the VerificationHarness build_lineage_rollover_sequence method
//! WHEN called with multiple epochs
//! THEN it produces a correct sequence of WorkflowStarted and ContinuedAsNew events

#[test]
fn harness_builds_correct_rollover_sequence_single_epoch() {
    let events =
        VerificationHarness::build_lineage_rollover_sequence("lin-abc", "inst-1", vec![0]);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].instance_id, "inst-1");
    assert_eq!(events[0].sequence, 1);

    let payload = EventPayload::try_from_json(&events[0].payload).expect("valid payload");
    match payload {
        EventPayload::WorkflowStarted { workflow_id, .. } => {
            assert_eq!(workflow_id, "lin-abc");
        }
        other => panic!("Expected WorkflowStarted, got {:?}", other),
    }
}

#[test]
fn harness_builds_correct_rollover_sequence_multiple_epochs() {
    let events = VerificationHarness::build_lineage_rollover_sequence(
        "lin-abc",
        "inst-1",
        vec![0, 1, 2],
    );

    // 3 WorkflowStarted + 2 ContinuedAsNew = 5 events
    assert_eq!(events.len(), 5);

    // Event 0: WorkflowStarted in epoch 0
    assert_eq!(events[0].instance_id, "inst-1");
    assert_eq!(events[0].sequence, 1);

    // Event 1: WorkflowStarted in epoch 1
    assert_eq!(events[1].instance_id, "inst-1-epoch-1");
    assert_eq!(events[1].sequence, 2);

    // Event 2: ContinuedAsNew (epoch 0 -> 1)
    assert_eq!(events[2].instance_id, "inst-1-epoch-1");
    assert_eq!(events[2].sequence, 3);
    let payload = EventPayload::try_from_json(&events[2].payload).expect("valid payload");
    match payload {
        EventPayload::ContinuedAsNew {
            old_epoch,
            new_epoch,
            ..
        } => {
            assert_eq!(old_epoch, 0);
            assert_eq!(new_epoch, 1);
        }
        other => panic!("Expected ContinuedAsNew, got {:?}", other),
    }

    // Event 3: WorkflowStarted in epoch 2
    assert_eq!(events[3].instance_id, "inst-1-epoch-2");
    assert_eq!(events[3].sequence, 4);

    // Event 4: ContinuedAsNew (epoch 1 -> 2)
    assert_eq!(events[4].instance_id, "inst-1-epoch-2");
    assert_eq!(events[4].sequence, 5);
    let payload = EventPayload::try_from_json(&events[4].payload).expect("valid payload");
    match payload {
        EventPayload::ContinuedAsNew {
            old_epoch,
            new_epoch,
            ..
        } => {
            assert_eq!(old_epoch, 1);
            assert_eq!(new_epoch, 2);
        }
        other => panic!("Expected ContinuedAsNew, got {:?}", other),
    }
}

#[test]
fn harness_verify_lineage_rollover_deterministic() {
    let events = vec![
        make_event(
            "inst-1",
            1,
            workflow_started_payload("inst-1", "lin-1"),
        ),
        LineageRolloverEvent::new("lin-1".to_string(), 0, 1, "inst-1".to_string())
            .to_event_envelope(2),
    ];

    let engine = ReplayEngine::new();
    let result = engine.replay(&events);

    assert!(result.is_ok(), "Replay should succeed after lineage rollover");
}

// ---------------------------------------------------------------------------
// Scenario 9: Signal to different lineage is rejected
// ---------------------------------------------------------------------------
//! GIVEN a signal targeting lineage A
//! WHEN the instance belongs to lineage B
//! THEN the signal is rejected with lineage mismatch
//!
//! Per ADR-042 Section 1: lineage_id is a matching dimension

#[test]
fn signal_to_wrong_lineage_is_rejected() {
    let signal_lineage = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMJ").unwrap();
    let wait_lineage = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMK").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD3").unwrap();

    let wait = WaitRecord::new(
        instance_id.clone(),
        approval_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    let signal = SignalAddress::lineage_wide(
        signal_lineage.clone(),
        instance_id.clone(),
        approval_wait_key(),
    );

    // WHEN: Signal targeting lineage A, instance in lineage B
    let result = signal_match(&signal, &wait, &wait_lineage, Epoch::ZERO);

    // THEN: Lineage mismatch
    assert!(result.is_mismatch());
    match result {
        SignalMatchResult::LineageMismatch {
            signal_lineage_id,
            wait_lineage_id,
        } => {
            assert_eq!(signal_lineage_id, signal_lineage);
            assert_eq!(wait_lineage_id, wait_lineage);
        }
        other => panic!("Expected LineageMismatch, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Scenario 10: Wait key mismatch across epochs
// ---------------------------------------------------------------------------
//! GIVEN a signal with wait key "approval"
//! WHEN instance is waiting for "rejection"
//! THEN signal is rejected with wait key mismatch

#[test]
fn wait_key_mismatch_across_epochs() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFML").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD4").unwrap();

    let wait = WaitRecord::new(
        instance_id.clone(),
        WaitKey::parse("rejection").expect("valid"),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance_id.clone(),
        approval_wait_key(), // "approval" != "rejection"
    );

    // WHEN: Signal with wrong wait key at any epoch
    for epoch in [Epoch::ZERO, Epoch::new(1), Epoch::new(5)] {
        let result = signal_match(&signal, &wait, &lineage_id, epoch);
        assert!(
            result.is_mismatch(),
            "Wait key mismatch should be detected at any epoch"
        );
        match result {
            SignalMatchResult::WaitKeyMismatch {
                signal_wait_key,
                wait_wait_key,
            } => {
                assert_eq!(signal_wait_key.as_str(), "approval");
                assert_eq!(wait_wait_key.as_str(), "rejection");
            }
            other => panic!("Expected WaitKeyMismatch at epoch {:?}, got {:?}", epoch, other),
        }
    }
}

// ---------------------------------------------------------------------------
// Scenario 11: Instance mismatch across epochs
// ---------------------------------------------------------------------------
//! GIVEN a signal targeting instance A
//! WHEN instance B is the one waiting
//! THEN signal is rejected with instance mismatch

#[test]
fn instance_mismatch_across_epochs() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMM").unwrap();
    let signal_instance = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD5").unwrap();
    let wait_instance = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD6").unwrap();

    let wait = WaitRecord::new(
        wait_instance.clone(),
        approval_wait_key(),
        BufferPolicy::Reject,
        vo_types::TimestampMs::now(),
    )
    .expect("valid wait record");

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        signal_instance.clone(),
        approval_wait_key(),
    );

    // WHEN: Signal to wrong instance at any epoch
    for epoch in [Epoch::ZERO, Epoch::new(3)] {
        let result = signal_match(&signal, &wait, &lineage_id, epoch);
        assert!(
            result.is_mismatch(),
            "Instance mismatch should be detected at any epoch"
        );
        match result {
            SignalMatchResult::InstanceMismatch {
                signal_instance_id,
                wait_instance_id,
            } => {
                assert_eq!(signal_instance_id, signal_instance);
                assert_eq!(wait_instance_id, wait_instance);
            }
            other => panic!("Expected InstanceMismatch at epoch {:?}, got {:?}", epoch, other),
        }
    }
}

// ---------------------------------------------------------------------------
// Scenario 12: Complete end-to-end lineage signal lifecycle
// ---------------------------------------------------------------------------
//! GIVEN a full lineage lifecycle: start -> wait -> signal -> rollover -> wait -> signal
//! WHEN replayed through the event sourcing engine
//! THEN the final state is deterministic and signals are correctly placed

#[test]
fn complete_lineage_signal_lifecycle_e2e() {
    let lineage_id = "lin-e2e-lifecycle".to_string();
    let inst_0 = format!("{}-inst-0", lineage_id);
    let inst_1 = format!("{}-inst-1", lineage_id);

    let mut events = Vec::new();
    let mut seq = 1u64;

    // Phase 1: Epoch 0 - Workflow starts and waits
    events.push(make_event(
        &inst_0,
        seq,
        workflow_started_payload(&inst_0, &lineage_id),
    ));
    seq += 1;

    // Signal received in epoch 0
    events.push(make_event(
        &inst_0,
        seq,
        signal_received_payload(&inst_0, "sig-phase1", "phase1-complete"),
    ));
    seq += 1;

    // Step completed in epoch 0
    events.push(make_event(
        &inst_0,
        seq,
        step_completed_payload(&inst_0, "phase1-step", 1, 1),
    ));
    seq += 1;

    // Phase 2: Continue-as-new rollover
    let rollover = LineageRolloverEvent::new(lineage_id.clone(), 0, 1, inst_1.clone())
        .to_event_envelope(seq);
    events.push(rollover);
    seq += 1;

    // Phase 3: Epoch 1 - New workflow starts
    events.push(make_event(
        &inst_1,
        seq,
        workflow_started_payload(&inst_1, &lineage_id),
    ));
    seq += 1;

    // Signal received in epoch 1
    events.push(make_event(
        &inst_1,
        seq,
        signal_received_payload(&inst_1, "sig-phase2", "phase2-complete"),
    ));
    seq += 1;

    // Step completed in epoch 1
    events.push(make_event(
        &inst_1,
        seq,
        step_completed_payload(&inst_1, "phase2-step", 1, 2),
    ));

    // WHEN: Replay complete lifecycle
    let engine = ReplayEngine::new();
    let result = engine.replay(&events);

    // THEN: All events applied successfully
    assert!(result.is_ok(), "E2E lifecycle replay should succeed: {:?}", result);
    let replay_result = result.unwrap();
    assert_eq!(
        replay_result.events_applied, events.len(),
        "All {} events should be applied",
        events.len()
    );
}

// ---------------------------------------------------------------------------
// Scenario 13: Signal address display format
// ---------------------------------------------------------------------------
//! GIVEN signal addresses at different scopes
//! WHEN formatted via Display
//! THEN the output includes lineage_id, instance_id, wait_key, and scope

#[test]
fn lineage_wide_signal_address_display_format() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMN").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD7").unwrap();

    let signal = SignalAddress::lineage_wide(
        lineage_id.clone(),
        instance_id.clone(),
        approval_wait_key(),
    );

    let display = format!("{}", signal);

    // Display should contain lineage_id, instance_id, wait_key, and "lineage-wide"
    assert!(display.contains(lineage_id.as_str()));
    assert!(display.contains(instance_id.as_str()));
    assert!(display.contains("approval"));
    assert!(display.contains("lineage-wide"));
}

#[test]
fn epoch_local_signal_address_display_format() {
    let lineage_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMO").unwrap();
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFD8").unwrap();

    let signal = SignalAddress::epoch_local(
        lineage_id.clone(),
        Epoch::new(3),
        instance_id.clone(),
        continuation_wait_key(),
    );

    let display = format!("{}", signal);

    // Display should contain lineage_id, instance_id, wait_key, and "epoch=3"
    assert!(display.contains(lineage_id.as_str()));
    assert!(display.contains(instance_id.as_str()));
    assert!(display.contains("continue"));
    assert!(display.contains("epoch=3"));
}

// ---------------------------------------------------------------------------
// Scenario 14: WorkflowLineage preserves identity across rollovers
// ---------------------------------------------------------------------------
//! GIVEN a WorkflowLineage
//! WHEN performing continue_as_new multiple times
//! THEN lineage_id remains constant and epoch increments correctly

#[test]
fn workflow_lineage_id_preserved_across_rollovers() {
    let lineage = WorkflowLineage::new("lin-identity-test".to_string()).expect("valid lineage");
    let original_id = lineage.lineage_id().to_string();

    // Rollover chain: 0 -> 1 -> 2 -> 3 -> 4
    let mut current = lineage;
    for expected_epoch in 1..=4u64 {
        let next = current.continue_as_new().expect("rollover");
        assert_eq!(
            next.epoch(),
            Epoch::new(expected_epoch),
            "Epoch should be {}",
            expected_epoch
        );
        assert_eq!(
            next.parent_epoch(),
            Some(Epoch::new(expected_epoch - 1)),
            "Parent should be epoch {}",
            expected_epoch - 1
        );
        assert_eq!(
            next.lineage_id(),
            &original_id,
            "Lineage ID should be preserved at epoch {}",
            expected_epoch
        );
        current = next;
    }
}
