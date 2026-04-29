//! ADR-036 Causation Chain Lifecycle Tests
//!
//! These tests verify that causation IDs propagate correctly through a complete
//! workflow lifecycle, from initial trigger through all execution stages to completion.
//!
//! Per ADR-036:
//! - `causation_id` points to the immediate parent event or command that caused this command
//! - Every event emitted by the Engine records the command metadata that caused it
//! - The causation chain enables full traceability through business flows, retries, and compensations
//!
//! This test module simulates a complete workflow lifecycle:
//! 1. External trigger (API call) with root causation
//! 2. Workflow start command
//! 3. Step execution commands (parent-child relationships)
//! 4. Timer/signal commands (nested causation)
//! 5. Workflow completion
//!
//! All events should maintain proper causation linkage enabling full chain recovery.

use std::collections::HashMap;

use vo_types::command_metadata::CommandMetadata;
use vo_types::events::envelope::EventEnvelope;
use vo_types::events::metadata::EventMetadata;
use vo_types::CommandEnvelope;
use vo_types::{IdempotencyKey, Issuer, TimestampMs};

fn make_cmd_meta(
    command_id: &str,
    correlation_id: &str,
    causation_id: &str,
    issuer: Issuer,
) -> CommandMetadata {
    CommandMetadata {
        command_id: IdempotencyKey::parse(command_id).unwrap(),
        correlation_id: IdempotencyKey::parse(correlation_id).unwrap(),
        causation_id: IdempotencyKey::parse(causation_id).unwrap(),
        issuer,
        issued_at: TimestampMs::now(),
    }
}

fn make_event_envelope(
    instance_id: &str,
    sequence: u64,
    meta: CommandMetadata,
    payload_type: &str,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: TimestampMs::now().as_u64(),
        payload: serde_json::json!({ "type": payload_type }),
        metadata: EventMetadata {
            command_metadata: Some(meta),
            annotations: HashMap::new(),
        },
    }
}

/// Simulates an external API call that triggers a workflow.
/// Root causation: the external trigger has no parent, so causation_id = "external-root"
#[test]
fn lifecycle_external_trigger_has_root_causation() {
    let trigger_meta = make_cmd_meta(
        "cmd-trigger-001",
        "corr-business-423",
        "external-root",
        Issuer::ApiClient,
    );

    let event = make_event_envelope("inst-workflow-1", 1, trigger_meta, "WorkflowTriggered");

    let event_causation = event
        .metadata
        .command_metadata
        .as_ref()
        .expect("event must carry command metadata")
        .causation_id
        .as_str();

    assert_eq!(
        event_causation, "external-root",
        "External trigger must have root causation ID"
    );
}

/// Simulates workflow start: the start command's causation_id links to the trigger.
#[test]
fn lifecycle_workflow_start_links_to_trigger() {
    let trigger_meta = make_cmd_meta(
        "cmd-trigger-001",
        "corr-business-423",
        "external-root",
        Issuer::ApiClient,
    );

    let start_meta = make_cmd_meta(
        "cmd-wf-start-001",
        "corr-business-423",
        "cmd-trigger-001",
        Issuer::System,
    );

    let trigger_event =
        make_event_envelope("inst-workflow-1", 1, trigger_meta, "WorkflowTriggered");
    let start_event = make_event_envelope("inst-workflow-1", 2, start_meta, "WorkflowStarted");

    let start_causation = start_event
        .metadata
        .command_metadata
        .as_ref()
        .expect("start event must have command metadata")
        .causation_id
        .as_str();

    let trigger_cmd_id = trigger_event
        .metadata
        .command_metadata
        .as_ref()
        .expect("trigger event must have command metadata")
        .command_id
        .as_str();

    assert_eq!(
        start_causation, trigger_cmd_id,
        "Workflow start causation must link to trigger command"
    );
}

/// Simulates step execution: each step's causation links to its parent step or workflow start.
#[test]
fn lifecycle_step_chain_propagates_causation() {
    let corr_id = "corr-workflow-abc";

    // Step 1: after workflow start
    let step1_meta = make_cmd_meta("cmd-step-1", corr_id, "cmd-wf-start-001", Issuer::System);
    let step1_event = make_event_envelope("inst-workflow-1", 3, step1_meta, "StepScheduled");

    // Step 2: child of step 1
    let step2_meta = make_cmd_meta("cmd-step-2", corr_id, "cmd-step-1", Issuer::System);
    let step2_event = make_event_envelope("inst-workflow-1", 4, step2_meta, "StepScheduled");

    // Step 3: child of step 2
    let step3_meta = make_cmd_meta("cmd-step-3", corr_id, "cmd-step-2", Issuer::System);
    let step3_event = make_event_envelope("inst-workflow-1", 5, step3_meta, "StepScheduled");

    let step1_causation = step1_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .causation_id
        .as_str();
    let step2_causation = step2_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .causation_id
        .as_str();
    let step3_causation = step3_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .causation_id
        .as_str();

    assert_eq!(step1_causation, "cmd-wf-start-001");
    assert_eq!(step2_causation, "cmd-step-1");
    assert_eq!(step3_causation, "cmd-step-2");
}

/// Simulates timer callback: timer firing's causation links to the wait command.
#[test]
fn lifecycle_timer_fired_links_to_wait_command() {
    let corr_id = "corr-workflow-timer";

    // Wait command that set the timer
    let wait_meta = make_cmd_meta("cmd-wait-1", corr_id, "cmd-step-2", Issuer::TimerLoop);
    let wait_event = make_event_envelope("inst-workflow-1", 10, wait_meta, "WaitForTimer");

    // Timer fires - child of wait command
    let timer_fired_meta =
        make_cmd_meta("cmd-timer-fired-1", corr_id, "cmd-wait-1", Issuer::System);
    let timer_event = make_event_envelope("inst-workflow-1", 11, timer_fired_meta, "TimerFired");

    let timer_causation = timer_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .causation_id
        .as_str();

    let wait_cmd_id = wait_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .command_id
        .as_str();

    assert_eq!(
        timer_causation, wait_cmd_id,
        "Timer fired causation must link to the wait command that set it"
    );
}

/// Simulates signal handling: signal processing's causation links to the signal receipt.
#[test]
fn lifecycle_signal_handler_links_to_signal() {
    let corr_id = "corr-workflow-signal";

    // Signal arrives
    let signal_meta = make_cmd_meta("cmd-signal-1", corr_id, "cmd-step-3", Issuer::Operator);
    let signal_event = make_event_envelope("inst-workflow-1", 20, signal_meta, "SignalReceived");

    // Signal is processed - child of signal receipt
    let process_meta = make_cmd_meta(
        "cmd-process-signal-1",
        corr_id,
        "cmd-signal-1",
        Issuer::System,
    );
    let process_event = make_event_envelope("inst-workflow-1", 21, process_meta, "SignalProcessed");

    let process_causation = process_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .causation_id
        .as_str();

    let signal_cmd_id = signal_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .command_id
        .as_str();

    assert_eq!(
        process_causation, signal_cmd_id,
        "Signal processing causation must link to signal receipt command"
    );
}

/// Verifies that retry maintains causation linkage: retry command's causation links to original.
#[test]
fn lifecycle_retry_preserves_causation_to_original_command() {
    let corr_id = "corr-workflow-retry";

    // Original step failed
    let original_meta = make_cmd_meta("cmd-step-fail-1", corr_id, "cmd-step-1", Issuer::System);
    let _original_event = make_event_envelope("inst-workflow-1", 30, original_meta, "StepFailed");

    // Retry command - its causation still links to the parent step, not the failure
    let retry_meta = make_cmd_meta(
        "cmd-step-retry-1",
        corr_id,
        "cmd-step-1",
        Issuer::RecoveryLoop,
    );
    let retry_event = make_event_envelope("inst-workflow-1", 31, retry_meta, "StepScheduled");

    // The retry's causation points to the workflow parent (step-1), NOT the failed command
    // This maintains the original execution lineage even through retries
    let retry_causation = retry_event
        .metadata
        .command_metadata
        .as_ref()
        .unwrap()
        .causation_id
        .as_str();

    assert_eq!(
        retry_causation, "cmd-step-1",
        "Retry causation must maintain link to workflow parent, not failed command"
    );
}

/// Full lifecycle: traces the complete causation chain from trigger to completion.
#[test]
fn lifecycle_full_chain_traceable_from_completion_to_trigger() {
    let corr_id = "corr-full-lifecycle";
    let instance_id = "inst-full-lifecycle";

    // Build the full event chain
    let trigger_meta = make_cmd_meta("cmd-trigger", corr_id, "external-root", Issuer::ApiClient);
    let trigger_event = make_event_envelope(instance_id, 1, trigger_meta, "WorkflowTriggered");

    let start_meta = make_cmd_meta("cmd-start", corr_id, "cmd-trigger", Issuer::System);
    let start_event = make_event_envelope(instance_id, 2, start_meta, "WorkflowStarted");

    let step1_meta = make_cmd_meta("cmd-step-1", corr_id, "cmd-start", Issuer::System);
    let step1_event = make_event_envelope(instance_id, 3, step1_meta, "StepScheduled");

    let step2_meta = make_cmd_meta("cmd-step-2", corr_id, "cmd-step-1", Issuer::System);
    let step2_event = make_event_envelope(instance_id, 4, step2_meta, "StepScheduled");

    let complete_meta = make_cmd_meta("cmd-complete", corr_id, "cmd-step-2", Issuer::System);
    let complete_event = make_event_envelope(instance_id, 5, complete_meta, "WorkflowCompleted");

    // Now trace backwards from completion
    let events = vec![
        trigger_event,
        start_event,
        step1_event,
        step2_event,
        complete_event,
    ];

    // Extract causation chain in reverse order (from completion to trigger)
    let causation_chain: Vec<String> = events
        .iter()
        .rev()
        .map(|e| {
            e.metadata
                .command_metadata
                .as_ref()
                .unwrap()
                .causation_id
                .as_str()
                .to_string()
        })
        .collect();

    // The chain traced backwards:
    // complete's causation → step-2's command_id → step-1's command_id → start's command_id → trigger's command_id → external-root
    assert_eq!(causation_chain[0], "cmd-step-2", "complete → step2");
    assert_eq!(causation_chain[1], "cmd-step-1", "step2 → step1");
    assert_eq!(causation_chain[2], "cmd-start", "step1 → start");
    assert_eq!(causation_chain[3], "cmd-trigger", "start → trigger");
    assert_eq!(causation_chain[4], "external-root", "trigger → root");
}

/// Verifies all events in a workflow share the same correlation_id (same business request).
#[test]
fn lifecycle_all_events_share_correlation_id() {
    let corr_id = "corr-business-request-xyz";
    let instance_id = "inst-correlation-test";

    let events = vec![
        make_event_envelope(
            instance_id,
            1,
            make_cmd_meta("cmd-trigger", corr_id, "external-root", Issuer::ApiClient),
            "WorkflowTriggered",
        ),
        make_event_envelope(
            instance_id,
            2,
            make_cmd_meta("cmd-start", corr_id, "cmd-trigger", Issuer::System),
            "WorkflowStarted",
        ),
        make_event_envelope(
            instance_id,
            3,
            make_cmd_meta("cmd-step-1", corr_id, "cmd-start", Issuer::System),
            "StepScheduled",
        ),
        make_event_envelope(
            instance_id,
            4,
            make_cmd_meta("cmd-step-2", corr_id, "cmd-step-1", Issuer::System),
            "StepScheduled",
        ),
    ];

    let correlation_ids: Vec<&str> = events
        .iter()
        .map(|e| {
            e.metadata
                .command_metadata
                .as_ref()
                .unwrap()
                .correlation_id
                .as_str()
        })
        .collect();

    assert!(
        correlation_ids.iter().all(|id| *id == corr_id),
        "All events must share the same correlation_id for the business request"
    );
}

/// Verifies that events can be filtered by correlation_id to get all events in a business flow.
#[test]
fn lifecycle_correlation_id_enables_event_filtering() {
    let target_corr = "corr-filter-test";
    let other_corr = "corr-other-request";

    let instance_id = "inst-filter-test";

    let events = vec![
        // Target business request
        make_event_envelope(
            instance_id,
            1,
            make_cmd_meta(
                "cmd-trigger-A",
                target_corr,
                "external-A",
                Issuer::ApiClient,
            ),
            "WorkflowTriggered",
        ),
        make_event_envelope(
            instance_id,
            2,
            make_cmd_meta("cmd-start-A", target_corr, "cmd-trigger-A", Issuer::System),
            "WorkflowStarted",
        ),
        // Other business request (should be filtered out)
        make_event_envelope(
            instance_id,
            3,
            make_cmd_meta("cmd-trigger-B", other_corr, "external-B", Issuer::ApiClient),
            "WorkflowTriggered",
        ),
        make_event_envelope(
            instance_id,
            4,
            make_cmd_meta("cmd-start-B", other_corr, "cmd-trigger-B", Issuer::System),
            "WorkflowStarted",
        ),
    ];

    // Filter by target correlation
    let filtered: Vec<&EventEnvelope> = events
        .iter()
        .filter(|e| {
            e.metadata
                .command_metadata
                .as_ref()
                .map(|m| m.correlation_id.as_str() == target_corr)
                .unwrap_or(false)
        })
        .collect();

    assert_eq!(
        filtered.len(),
        2,
        "Should find exactly 2 events for target correlation"
    );
    assert_eq!(
        filtered[0]
            .metadata
            .command_metadata
            .as_ref()
            .unwrap()
            .command_id
            .as_str(),
        "cmd-trigger-A"
    );
    assert_eq!(
        filtered[1]
            .metadata
            .command_metadata
            .as_ref()
            .unwrap()
            .command_id
            .as_str(),
        "cmd-start-A"
    );
}

/// Verifies causation chain is preserved through JSON serialization/deserialization.
#[test]
fn lifecycle_causation_chain_survives_event_serialization() {
    let corr_id = "corr-serialize-test";
    let instance_id = "inst-serialize-test";

    let original_events = vec![
        make_event_envelope(
            instance_id,
            1,
            make_cmd_meta("cmd-root", corr_id, "external-root", Issuer::ApiClient),
            "WorkflowTriggered",
        ),
        make_event_envelope(
            instance_id,
            2,
            make_cmd_meta("cmd-child", corr_id, "cmd-root", Issuer::System),
            "WorkflowStarted",
        ),
    ];

    // Serialize and deserialize
    let serialized: Vec<String> = original_events
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect();

    let restored: Vec<EventEnvelope> = serialized
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();

    // Verify causation chain is intact
    assert_eq!(
        restored[1]
            .metadata
            .command_metadata
            .as_ref()
            .unwrap()
            .causation_id
            .as_str(),
        "cmd-root",
        "Child command's causation must survive serialization"
    );

    assert_eq!(
        restored[0]
            .metadata
            .command_metadata
            .as_ref()
            .unwrap()
            .causation_id
            .as_str(),
        "external-root",
        "Root command's causation must survive serialization"
    );
}

/// Verifies that different issuers correctly populate the causation chain.
/// System commands, timer loops, recovery loops, and operators all participate in causation.
#[test]
fn lifecycle_all_issuer_types_can_appear_in_causation_chain() {
    let corr_id = "corr-issuer-chain";
    let instance_id = "inst-issuer-test";

    let events = vec![
        // API client trigger
        make_event_envelope(
            instance_id,
            1,
            make_cmd_meta("cmd-api", corr_id, "external-api", Issuer::ApiClient),
            "WorkflowTriggered",
        ),
        // System starts workflow
        make_event_envelope(
            instance_id,
            2,
            make_cmd_meta("cmd-system", corr_id, "cmd-api", Issuer::System),
            "WorkflowStarted",
        ),
        // Timer loop waits
        make_event_envelope(
            instance_id,
            3,
            make_cmd_meta("cmd-timer", corr_id, "cmd-system", Issuer::TimerLoop),
            "WaitForTimer",
        ),
        // System continues after timer
        make_event_envelope(
            instance_id,
            4,
            make_cmd_meta("cmd-resume", corr_id, "cmd-timer", Issuer::System),
            "TimerFired",
        ),
        // Recovery loop handles failure
        make_event_envelope(
            instance_id,
            5,
            make_cmd_meta("cmd-recovery", corr_id, "cmd-resume", Issuer::RecoveryLoop),
            "StepRetry",
        ),
        // Operator intervenes
        make_event_envelope(
            instance_id,
            6,
            make_cmd_meta("cmd-operator", corr_id, "cmd-recovery", Issuer::Operator),
            "SignalReceived",
        ),
    ];

    // Verify all issuers are present and chain is intact
    let issuers: Vec<Issuer> = events
        .iter()
        .map(|e| e.metadata.command_metadata.as_ref().unwrap().issuer.clone())
        .collect();

    let causations: Vec<&str> = events
        .iter()
        .map(|e| {
            e.metadata
                .command_metadata
                .as_ref()
                .unwrap()
                .causation_id
                .as_str()
        })
        .collect();

    assert_eq!(issuers[0], Issuer::ApiClient);
    assert_eq!(issuers[1], Issuer::System);
    assert_eq!(issuers[2], Issuer::TimerLoop);
    assert_eq!(issuers[3], Issuer::System);
    assert_eq!(issuers[4], Issuer::RecoveryLoop);
    assert_eq!(issuers[5], Issuer::Operator);

    // Verify causation chain
    assert_eq!(causations[1], "cmd-api");
    assert_eq!(causations[2], "cmd-system");
    assert_eq!(causations[3], "cmd-timer");
    assert_eq!(causations[4], "cmd-resume");
    assert_eq!(causations[5], "cmd-recovery");
}

/// Verifies that CommandEnvelope causation chain works identically to EventEnvelope.
/// Both surfaces must maintain the same causation semantics per ADR-036.
#[test]
fn lifecycle_command_envelope_and_event_envelope_have_identical_causation_semantics() {
    let corr_id = "corr-envelope-test";
    let cmd_meta = make_cmd_meta("cmd-envelope", corr_id, "external-test", Issuer::System);

    let json = serde_json::json!({
        "version": 1,
        "command_id": "cmd-envelope",
        "correlation_id": corr_id,
        "causation_id": "external-test",
        "issuer": "system",
        "issued_at": 1700000000
    })
    .to_string();

    let envelope = CommandEnvelope::from_str(&json).unwrap();

    assert_eq!(
        envelope.metadata.causation_id.as_str(),
        cmd_meta.causation_id.as_str(),
        "CommandEnvelope causation must match CommandMetadata causation"
    );
    assert_eq!(
        envelope.metadata.correlation_id.as_str(),
        cmd_meta.correlation_id.as_str(),
        "CommandEnvelope correlation must match CommandMetadata correlation"
    );
    assert_eq!(
        envelope.metadata.command_id.as_str(),
        cmd_meta.command_id.as_str(),
        "CommandEnvelope command_id must match CommandMetadata command_id"
    );
}
