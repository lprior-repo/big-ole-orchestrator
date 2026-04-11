//! Tests for `ReplayEngine::replay_with_upcaster`.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;
use vo_types::state::LifecycleState;

use crate::upcaster::{Upcaster, UpcasterError, UpcasterRegistry};

/// A simple upcaster that transforms version 0 JSON to version 1.
struct Version0To1Upcaster;

impl Upcaster for Version0To1Upcaster {
    fn source_version(&self) -> u8 {
        0
    }

    fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        let mut value: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
        value["version"] = serde_json::json!(1);
        serde_json::to_vec(&value).map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))
    }
}

/// An upcaster that fails to parse its input.
struct FailingUpcaster;

impl Upcaster for FailingUpcaster {
    fn source_version(&self) -> u8 {
        0
    }

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        Err(UpcasterError::UpcastingFailed(
            "cannot parse input JSON".to_string(),
        ))
    }
}

/// Helper to create a registry with a Version0To1Upcaster
fn make_registry_with_upcaster() -> crate::upcaster::UpcasterRegistryImpl {
    let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
    let _ = registry.register(Box::new(Version0To1Upcaster));
    registry
}

/// Helper to create a registry with a failing upcaster
fn make_registry_with_failing_upcaster() -> crate::upcaster::UpcasterRegistryImpl {
    let registry = crate::upcaster::UpcasterRegistryImpl::new(1);
    let _ = registry.register(Box::new(FailingUpcaster));
    registry
}

// =====================================================================
// Behavior: Empty event list with upcaster
// =====================================================================

#[test]
fn replay_with_upcaster_returns_empty_result_when_event_list_is_empty() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_upcaster();
    let result = engine
        .replay_with_upcaster(&registry, &[])
        .expect("empty replay should succeed");
    assert_eq!(result.final_state, None);
    assert_eq!(result.events_applied, 0);
}

// =====================================================================
// Behavior: Version 0 event is upcast to version 1 and replay succeeds
// =====================================================================

#[test]
fn replay_with_upcaster_upcasts_v0_event_and_replays_successfully() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_upcaster();
    let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 1);
}

// =====================================================================
// Behavior: Full lifecycle with version 0 events upcast correctly
// =====================================================================

#[test]
fn replay_with_upcaster_full_lifecycle_v0_to_v1() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_upcaster();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

// =====================================================================
// Behavior: Upcasting failure propagates as ReplayError::UpcastingFailed
// =====================================================================

#[test]
fn replay_with_upcaster_returns_upcasting_failed_when_upcaster_errors() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_failing_upcaster();
    let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];
    let result = engine.replay_with_upcaster(&registry, &events);
    let err = result.expect_err("replay should fail");
    assert!(matches!(
        err,
        ReplayError::UpcastingFailed { sequence: 1, .. }
    ));
}

// =====================================================================
// Behavior: Events already at max version pass through unchanged
// =====================================================================

#[test]
fn replay_with_upcaster_preserves_v1_events() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_upcaster();
    let events = [make_event("inst-1", 1, workflow_started_payload("wf-1"))];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 1);
}

// =====================================================================
// Behavior: Mixed version events all get upcast
// =====================================================================

#[test]
fn replay_with_upcaster_handles_mixed_version_events() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_upcaster();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}
