//! End-to-end business workflow validation.
//!
//! Tests the complete saga execution path by composing the existing building blocks:
//! - WorkflowDefinition (DAG construction + validation)
//! - next_nodes() (DAG traversal)
//! - EventEnvelope + EventPayload (event sourcing)
//! - ReplayEngine (deterministic state reconstruction)
//! - LifecycleState + apply() (state machine transitions)
//!
//! Zero unwrap. All fallible operations use Result/assertion macros.

use serde_json::json;
use vo_core::replay::ReplayEngine;
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::next_nodes;
use vo_types::state::LifecycleState;
use vo_types::{StepOutcome, WorkflowDefinition};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

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
    json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "binary_hash": "sha256abc123",
        "workflow_version_hash": "wvhash456",
        "dedupe_key_hash": null,
        "version": 1
    })
}

fn step_scheduled_payload(workflow_id: &str, step_id: &str, attempt: u32, fence: u64) -> serde_json::Value {
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

fn step_started_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    json!({
        "type": "StepStarted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

fn step_completed_payload(workflow_id: &str, step_id: &str, attempt: u32, fence: u64) -> serde_json::Value {
    json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "completed_at_ms": 3000,
        "attempt": attempt,
        "fence": fence,
        "routing_projection": null,
        "output_ref": null,
        "output_hash": null,
        "output": null,
        "version": 1
    })
}

fn workflow_completed_payload(workflow_id: &str) -> serde_json::Value {
    json!({
        "type": "WorkflowCompleted",
        "workflow_id": workflow_id,
        "completion_time_ms": 5000,
        "version": 1
    })
}

fn workflow_from_json(json: serde_json::Value) -> WorkflowDefinition {
    WorkflowDefinition::from_deserializer(&json)
        .unwrap_or_else(|e| panic!("workflow JSON should be valid: {e}"))
}

fn two_node_linear_workflow() -> WorkflowDefinition {
    workflow_from_json(json!({
        "workflow_name": "e2e-linear-two",
        "nodes": [
            { "node_name": "fetch", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "process", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } }
        ],
        "edges": [
            { "source_node": "fetch", "target_node": "process", "condition": "OnSuccess" }
        ]
    }))
}

/// Simulate the full saga: walk the DAG, emit events for each step, replay to verify final state.
fn simulate_saga(
    workflow: &WorkflowDefinition,
    first_node: &str,
    step_outcomes: &[(&str, StepOutcome)],
    terminal_event: &serde_json::Value,
) -> Result<(Vec<EventEnvelope>, Option<LifecycleState>), String> {
    let instance_id = "saga-inst-001";
    let workflow_id = workflow.workflow_name.as_str();
    let mut events = Vec::new();
    let mut seq: u64 = 0;
    let mut current_node = first_node.to_string();
    let mut fence: u64 = 1;
    let mut attempt: u32 = 1;

    seq += 1;
    events.push(make_event(instance_id, seq, workflow_started_payload(workflow_id)));

    for (step_name, outcome) in step_outcomes {
        seq += 1;
        events.push(make_event(instance_id, seq, step_scheduled_payload(workflow_id, step_name, attempt, fence)));
        seq += 1;
        events.push(make_event(instance_id, seq, step_started_payload(workflow_id, step_name)));

        match outcome {
            StepOutcome::Success => {
                seq += 1;
                events.push(make_event(instance_id, seq, step_completed_payload(workflow_id, step_name, attempt, fence)));
                let current = vo_types::NodeName::parse(&current_node)
                    .map_err(|e| format!("invalid node name '{current_node}': {e}"))?;
                let successors = next_nodes(&current, StepOutcome::Success, workflow);
                if !successors.is_empty() {
                    assert_eq!(successors.len(), 1, "linear step should have exactly 1 successor");
                    current_node = successors[0].node_name.as_str().to_string();
                }
                fence += 1;
                attempt = 1;
            }
            StepOutcome::Failure => {
                seq += 1;
                events.push(make_event(instance_id, seq, json!({
                    "type": "StepFailed",
                    "workflow_id": workflow_id,
                    "step_id": step_name,
                    "failure_reason": "step failed",
                    "attempt": attempt,
                    "fence": fence,
                    "version": 1
                })));
                let current = vo_types::NodeName::parse(&current_node)
                    .map_err(|e| format!("invalid node name '{current_node}': {e}"))?;
                let successors = next_nodes(&current, StepOutcome::Failure, workflow);
                if !successors.is_empty() {
                    current_node = successors[0].node_name.as_str().to_string();
                }
                fence += 1;
                attempt += 1;
            }
        }
    }

    seq += 1;
    events.push(make_event(instance_id, seq, terminal_event.clone()));

    let engine = ReplayEngine::new();
    let result = engine.replay(&events).map_err(|e| format!("replay failed: {e}"))?;
    Ok((events, result.final_state))
}

// =========================================================================
// Section 1: Linear workflow execution
// =========================================================================

#[test]
fn e2e_linear_two_node_workflow_completes_successfully() {
    let workflow = two_node_linear_workflow();
    let step_outcomes = vec![
        ("fetch", StepOutcome::Success),
        ("process", StepOutcome::Success),
    ];
    let terminal = workflow_completed_payload(workflow.workflow_name.as_str());

    let (events, final_state) = simulate_saga(&workflow, "fetch", &step_outcomes, &terminal)
        .expect("saga simulation should succeed");

    // WorkflowStarted + 2 * (Scheduled + Started + Completed) + WorkflowCompleted = 8
    assert_eq!(events.len(), 8, "should emit 8 events for 2-step linear workflow");

    for (i, event) in events.iter().enumerate() {
        assert_eq!(event.sequence as usize, i + 1, "event at index {i} should have sequence {}", i + 1);
    }

    let instance_id = events[0].instance_id.clone();
    for event in &events {
        assert_eq!(event.instance_id, instance_id, "all events must share the same instance_id");
    }

    assert_eq!(final_state, Some(LifecycleState::Completed), "workflow should complete successfully");
}

#[test]
fn e2e_single_node_workflow_completes_immediately() {
    let workflow = workflow_from_json(json!({
        "workflow_name": "e2e-single",
        "nodes": [
            { "node_name": "only", "retry_policy": { "max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0 } }
        ],
        "edges": []
    }));
    let step_outcomes = vec![("only", StepOutcome::Success)];
    let terminal = workflow_completed_payload(workflow.workflow_name.as_str());

    let (events, final_state) = simulate_saga(&workflow, "only", &step_outcomes, &terminal)
        .expect("saga simulation should succeed");

    // WorkflowStarted + Scheduled + Started + Completed + WorkflowCompleted = 5
    assert_eq!(events.len(), 5, "single-node workflow should emit 5 events");
    assert_eq!(final_state, Some(LifecycleState::Completed), "single-node workflow should complete");
}

#[test]
fn e2e_five_node_pipeline_executes_in_order() {
    let workflow = workflow_from_json(json!({
        "workflow_name": "e2e-pipeline",
        "nodes": [
            { "node_name": "validate", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "enrich", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "transform", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "persist", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "notify", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } }
        ],
        "edges": [
            { "source_node": "validate", "target_node": "enrich", "condition": "OnSuccess" },
            { "source_node": "enrich", "target_node": "transform", "condition": "OnSuccess" },
            { "source_node": "transform", "target_node": "persist", "condition": "OnSuccess" },
            { "source_node": "persist", "target_node": "notify", "condition": "OnSuccess" }
        ]
    }));
    let step_outcomes = vec![
        ("validate", StepOutcome::Success),
        ("enrich", StepOutcome::Success),
        ("transform", StepOutcome::Success),
        ("persist", StepOutcome::Success),
        ("notify", StepOutcome::Success),
    ];
    let terminal = workflow_completed_payload(workflow.workflow_name.as_str());

    let (events, final_state) = simulate_saga(&workflow, "validate", &step_outcomes, &terminal)
        .expect("saga simulation should succeed");

    // 1 (started) + 5*3 (sched+start+complete) + 1 (completed) = 17
    assert_eq!(events.len(), 17, "five-node pipeline should emit 17 events");
    assert_eq!(final_state, Some(LifecycleState::Completed), "pipeline should complete");
}

#[test]
fn e2e_next_nodes_returns_correct_successors_after_success() {
    let workflow = two_node_linear_workflow();
    let fetch = vo_types::NodeName::parse("fetch").expect("valid node name");
    let successors = next_nodes(&fetch, StepOutcome::Success, &workflow);
    assert_eq!(successors.len(), 1, "fetch should have one successor on success");
    assert_eq!(successors[0].node_name.as_str(), "process");

    let no_successors = next_nodes(&fetch, StepOutcome::Failure, &workflow);
    assert!(no_successors.is_empty(), "fetch should have no successor on failure");
}

#[test]
fn e2e_next_nodes_diamond_fan_out() {
    let workflow = workflow_from_json(json!({
        "workflow_name": "e2e-diamond",
        "nodes": [
            { "node_name": "start", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "left", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "right", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "join", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } }
        ],
        "edges": [
            { "source_node": "start", "target_node": "left", "condition": "OnSuccess" },
            { "source_node": "start", "target_node": "right", "condition": "OnSuccess" },
            { "source_node": "left", "target_node": "join", "condition": "OnSuccess" },
            { "source_node": "right", "target_node": "join", "condition": "OnSuccess" }
        ]
    }));
    let start = vo_types::NodeName::parse("start").expect("valid node name");
    let successors = next_nodes(&start, StepOutcome::Success, &workflow);
    assert_eq!(successors.len(), 2, "start should fan out to 2 nodes on success");

    let names: Vec<&str> = successors.iter().map(|n| n.node_name.as_str()).collect();
    assert!(names.contains(&"left"), "successors should include left");
    assert!(names.contains(&"right"), "successors should include right");
}

#[test]
fn e2e_next_nodes_error_branch_routes_correctly() {
    let workflow = workflow_from_json(json!({
        "workflow_name": "e2e-error-branch",
        "nodes": [
            { "node_name": "action", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "on-done", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "compensate", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } }
        ],
        "edges": [
            { "source_node": "action", "target_node": "on-done", "condition": "OnSuccess" },
            { "source_node": "action", "target_node": "compensate", "condition": "OnFailure" }
        ]
    }));
    let action = vo_types::NodeName::parse("action").expect("valid node name");

    let success_path = next_nodes(&action, StepOutcome::Success, &workflow);
    assert_eq!(success_path.len(), 1);
    assert_eq!(success_path[0].node_name.as_str(), "on-done");

    let failure_path = next_nodes(&action, StepOutcome::Failure, &workflow);
    assert_eq!(failure_path.len(), 1);
    assert_eq!(failure_path[0].node_name.as_str(), "compensate");
}

#[test]
fn e2e_step_failure_transitions_to_failed_state() {
    let workflow = workflow_from_json(json!({
        "workflow_name": "e2e-error-branch",
        "nodes": [
            { "node_name": "action", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "on-done", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } },
            { "node_name": "compensate", "retry_policy": { "max_attempts": 3, "backoff_ms": 100, "backoff_multiplier": 2.0 } }
        ],
        "edges": [
            { "source_node": "action", "target_node": "on-done", "condition": "OnSuccess" },
            { "source_node": "action", "target_node": "compensate", "condition": "OnFailure" }
        ]
    }));

    // Simulate: action fails (StepFailed transitions to Failed terminal state)
    let instance_id = "failure-inst-001";
    let workflow_id = workflow.workflow_name.as_str();
    let events = vec![
        make_event(instance_id, 1, workflow_started_payload(workflow_id)),
        make_event(instance_id, 2, step_scheduled_payload(workflow_id, "action", 1, 1)),
        make_event(instance_id, 3, step_started_payload(workflow_id, "action")),
        make_event(instance_id, 4, json!({
            "type": "StepFailed",
            "workflow_id": workflow_id,
            "step_id": "action",
            "failure_reason": "step failed",
            "attempt": 1,
            "fence": 1,
            "version": 1
        })),
    ];

    let engine = ReplayEngine::new();
    let result = engine.replay(&events).expect("replay should succeed");

    assert_eq!(result.events_applied, 4, "all 4 events should be applied");
    assert_eq!(result.final_state, Some(LifecycleState::Failed), "StepFailed should transition to Failed");

    // Verify the DAG has a compensation path via next_nodes (even though replay is terminal)
    let action = vo_types::NodeName::parse("action").expect("valid node name");
    let failure_successors = next_nodes(&action, StepOutcome::Failure, &workflow);
    assert_eq!(failure_successors.len(), 1, "DAG should have compensation path");
    assert_eq!(failure_successors[0].node_name.as_str(), "compensate");
}

#[test]
fn e2e_workflow_cancellation_reaches_cancelled_state() {
    // Cancel during StepExecuting state (which accepts Cancel transition)
    let instance_id = "cancel-inst-001";
    let workflow_id = "e2e-cancel-wf";
    let events = vec![
        make_event(instance_id, 1, workflow_started_payload(workflow_id)),
        make_event(instance_id, 2, step_scheduled_payload(workflow_id, "step-1", 1, 1)),
        make_event(instance_id, 3, step_started_payload(workflow_id, "step-1")),
        make_event(instance_id, 4, json!({
            "type": "CancelRequested",
            "workflow_id": workflow_id,
            "requested_by": "user",
            "version": 1
        })),
    ];

    let engine = ReplayEngine::new();
    let result = engine.replay(&events).expect("replay should succeed");

    assert_eq!(result.events_applied, 4);
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled), "cancel during execution should reach Cancelled");
}

#[test]
fn e2e_timer_workflow_resumes_after_timer_fired() {
    let instance_id = "timer-inst-001";
    let workflow_id = "e2e-timer";
    let events = vec![
        make_event(instance_id, 1, workflow_started_payload(workflow_id)),
        make_event(instance_id, 2, step_scheduled_payload(workflow_id, "wait-step", 1, 1)),
        make_event(instance_id, 3, step_started_payload(workflow_id, "wait-step")),
        make_event(instance_id, 4, json!({
            "type": "TimerSet",
            "workflow_id": workflow_id,
            "timer_id": "timer-001",
            "fire_at_ms": 5000,
            "version": 1
        })),
        make_event(instance_id, 5, json!({
            "type": "TimerFired",
            "workflow_id": workflow_id,
            "timer_id": "timer-001",
            "fired_at_ms": 5000,
            "version": 1
        })),
        make_event(instance_id, 6, step_completed_payload(workflow_id, "wait-step", 1, 1)),
        make_event(instance_id, 7, workflow_completed_payload(workflow_id)),
    ];

    let engine = ReplayEngine::new();
    let result = engine.replay(&events).expect("replay should succeed");
    // Replay breaks at Completed after StepCompleted (event 6).
    // WorkflowCompleted (event 7) is never processed.
    assert_eq!(result.events_applied, 6);
    assert_eq!(result.final_state, Some(LifecycleState::Completed), "timer workflow should complete");
}

