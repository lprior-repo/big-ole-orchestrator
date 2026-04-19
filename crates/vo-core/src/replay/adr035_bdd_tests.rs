//! BDD tests for ADR-035: Event Schema Evolution and Upcasting.
//!
//! Scenarios:
//! - S1: Given event at schema vN, When upcaster applied, Then event transforms to vN+1 correctly.
//! - S2: Given missing upcaster for vN->vN+1, When replaying, Then UpcastingFailed error raised.
//! - S3: Given chained upcasters v1->v2->v3, When applied, Then final event matches direct v3 creation.
//! - S4: Upcaster chain in vo-core registry and ReplayError::UpcastingFailed in replay engine.

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;
use vo_types::events::upcaster::{Upcaster, UpcasterError};
use vo_types::events::{decode_event, EventEnvelope, EventMetadata, MAX_SUPPORTED_VERSION};
use vo_types::state::LifecycleState;

use crate::upcaster::{UpcasterError as RegistryError, UpcasterRegistry, UpcasterRegistryImpl};

// =============================================================================
// BDD Helpers
// =============================================================================

struct V1ToV2Upcaster;

impl Upcaster for V1ToV2Upcaster {
    fn source_version(&self) -> u8 {
        1
    }
    fn target_version(&self) -> u8 {
        2
    }
    fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut value = input.clone();
        if let Some(obj) = value.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(2));
            obj.insert(
                "added_in_v2".to_string(),
                serde_json::json!("v2-field-value"),
            );
        }
        Ok(value)
    }
}

struct V2ToV3Upcaster;

impl Upcaster for V2ToV3Upcaster {
    fn source_version(&self) -> u8 {
        2
    }
    fn target_version(&self) -> u8 {
        3
    }
    fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut value = input.clone();
        if let Some(obj) = value.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(3));
            obj.insert(
                "added_in_v3".to_string(),
                serde_json::json!("v3-field-value"),
            );
        }
        Ok(value)
    }
}

struct V0ToV1Upcaster;

impl Upcaster for V0ToV1Upcaster {
    fn source_version(&self) -> u8 {
        0
    }
    fn target_version(&self) -> u8 {
        1
    }
    fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut value = input.clone();
        if let Some(obj) = value.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(1));
        }
        Ok(value)
    }
}

struct FailingUpcaster;

impl Upcaster for FailingUpcaster {
    fn source_version(&self) -> u8 {
        1
    }
    fn target_version(&self) -> u8 {
        2
    }
    fn upcast(&self, _input: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        Err(UpcasterError::UpcastFailed(
            "deliberate upcast failure for testing".to_string(),
        ))
    }
}

fn make_v1_envelope(instance_id: &str, sequence: u64, payload: serde_json::Value) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn make_v2_envelope_direct(
    instance_id: &str,
    sequence: u64,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 2,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn registry_with_v1_to_v2() -> UpcasterRegistryImpl {
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(V1ToV2Upcaster));
    registry
}

fn registry_with_chain_v1_v2_v3() -> UpcasterRegistryImpl {
    let registry = UpcasterRegistryImpl::new(3);
    let _ = registry.register(Box::new(V1ToV2Upcaster));
    let _ = registry.register(Box::new(V2ToV3Upcaster));
    registry
}

fn registry_with_chain_v0_v1_v2_v3() -> UpcasterRegistryImpl {
    let registry = UpcasterRegistryImpl::new(3);
    let _ = registry.register(Box::new(V0ToV1Upcaster));
    let _ = registry.register(Box::new(V1ToV2Upcaster));
    let _ = registry.register(Box::new(V2ToV3Upcaster));
    registry
}

// =============================================================================
// S1: Given event at schema vN, When upcaster applied, Then event transforms
//     to vN+1 correctly.
// =============================================================================

#[test]
fn s1_given_v1_event_when_upcaster_applied_then_event_transforms_to_v2() {
    let registry = registry_with_v1_to_v2();
    let envelope = make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope);

    assert!(result.is_ok(), "upcast v1->v2 should succeed");
    let upcasted = result.unwrap();
    assert_eq!(
        upcasted.schema_version, 2,
        "schema_version should be 2 after upcast"
    );
    assert_eq!(
        upcasted.payload["version"], 2,
        "payload version field should be 2"
    );
    assert_eq!(
        upcasted.payload["added_in_v2"], "v2-field-value",
        "payload should contain v2-added field"
    );
}

#[test]
fn s1_given_v1_event_when_upcasted_then_identity_fields_preserved() {
    let registry = registry_with_v1_to_v2();
    let envelope = make_v1_envelope("inst-special", 42, workflow_started_payload("wf-x"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.instance_id, "inst-special");
    assert_eq!(result.sequence, 42);
    assert_eq!(result.timestamp_ms, 42000);
}

#[test]
fn s1_given_v1_event_when_upcasted_then_metadata_preserved() {
    let registry = registry_with_v1_to_v2();
    let mut metadata = EventMetadata::default();
    metadata
        .annotations
        .insert("trace_id".to_string(), serde_json::json!("trace-abc"));
    let envelope = EventEnvelope {
        schema_version: 1,
        instance_id: "inst-1".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: workflow_started_payload("wf-1"),
        metadata,
    };

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(
        result.metadata.annotations["trace_id"],
        serde_json::json!("trace-abc")
    );
}

#[test]
fn s1_given_v0_event_when_upcaster_applied_then_transforms_to_v1() {
    let registry = UpcasterRegistryImpl::new(1);
    let _ = registry.register(Box::new(V0ToV1Upcaster));
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.schema_version, 1);
}

#[test]
fn s1_given_v1_event_when_upcasted_then_payload_type_preserved() {
    let registry = registry_with_v1_to_v2();
    let envelope = make_v1_envelope("inst-1", 1, step_scheduled_payload("wf-1", "s1"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.payload["type"], "StepScheduled");
}

#[test]
fn s1_given_v1_event_at_max_version_when_upcasted_then_passes_through_unchanged() {
    let registry = UpcasterRegistryImpl::new(1);
    let envelope = make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.schema_version, 1);
    assert!(!result
        .payload
        .as_object()
        .unwrap()
        .contains_key("added_in_v2"));
}

#[test]
fn s1_given_v1_event_when_replayed_with_upcaster_then_lifecycle_succeeds() {
    let engine = ReplayEngine::new();
    let registry = registry_with_v1_to_v2();
    let events = [
        make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1")),
        make_v1_envelope("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_v1_envelope("inst-1", 3, step_completed_payload("wf-1", "s1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events);

    assert!(
        result.is_ok(),
        "replay with upcast should succeed: {result:?}"
    );
    let replay = result.unwrap();
    assert_eq!(replay.final_state, Some(LifecycleState::Completed));
    assert_eq!(replay.events_applied, 3);
}

// =============================================================================
// S2: Given missing upcaster for vN->vN+1, When replaying, Then
//     UpcastingFailed error raised.
// =============================================================================

#[test]
fn s2_given_missing_upcaster_v0_to_v1_when_replaying_then_upcasting_failed_raised() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(1);
    let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];

    let result = engine.replay_with_upcaster(&registry, &events);

    let err = result.expect_err("should fail with missing upcaster");
    assert!(
        matches!(err, ReplayError::UpcastingFailed { sequence: 1, .. }),
        "Expected UpcastingFailed at sequence 1, got: {err:?}"
    );
}

#[test]
fn s2_given_missing_upcaster_v1_to_v2_when_replaying_then_upcasting_failed_with_correct_sequence() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(2);
    let events = [make_v1_envelope(
        "inst-1",
        5,
        workflow_started_payload("wf-1"),
    )];

    let result = engine.replay_with_upcaster(&registry, &events);

    let err = result.expect_err("should fail with missing upcaster");
    assert!(
        matches!(err, ReplayError::UpcastingFailed { sequence: 5, .. }),
        "Expected UpcastingFailed at sequence 5, got: {err:?}"
    );
}

#[test]
fn s2_given_missing_upcaster_in_chain_when_upcasting_then_registry_returns_error() {
    let registry = UpcasterRegistryImpl::new(3);
    let _ = registry.register(Box::new(V0ToV1Upcaster));
    let _ = registry.register(Box::new(V2ToV3Upcaster));
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope);

    assert!(result.is_err(), "missing v1->v2 should fail");
    assert!(
        matches!(result.unwrap_err(), RegistryError::NoUpcasterRegistered(1)),
        "Expected NoUpcasterRegistered(1)"
    );
}

#[test]
fn s2_given_failing_upcaster_when_upcasting_then_upcasting_failed_error_raised() {
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(FailingUpcaster));
    let envelope = make_v1_envelope("inst-1", 3, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope);

    assert!(result.is_err(), "failing upcaster should error");
    assert!(
        matches!(result.unwrap_err(), RegistryError::UpcastingFailed(_)),
        "Expected UpcastingFailed from registry"
    );
}

#[test]
fn s2_given_failing_upcaster_when_replaying_then_replay_error_upcasting_failed() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(FailingUpcaster));
    let events = [make_v1_envelope(
        "inst-1",
        7,
        workflow_started_payload("wf-1"),
    )];

    let result = engine.replay_with_upcaster(&registry, &events);

    let err = result.expect_err("should fail");
    assert!(
        matches!(err, ReplayError::UpcastingFailed { sequence: 7, ref reason } if reason.contains("deliberate upcast failure")),
        "Expected UpcastingFailed with failure reason, got: {err:?}"
    );
}

#[test]
fn s2_given_missing_upcaster_for_second_event_when_replaying_then_first_event_sequence_reported() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(2);
    let events = [
        make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events);

    let err = result.expect_err("should fail on second event");
    assert!(
        matches!(err, ReplayError::UpcastingFailed { sequence: 2, .. }),
        "Expected UpcastingFailed at sequence 2 (second event), got: {err:?}"
    );
}

#[test]
fn s2_given_upcasting_failed_then_error_is_upcasting_failed_variant() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(1);
    let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];

    let result = engine.replay_with_upcaster(&registry, &events);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ReplayError::UpcastingFailed { .. }),
        "Expected UpcastingFailed variant, got: {err:?}"
    );
}

// =============================================================================
// S3: Given chained upcasters v1->v2->v3, When applied, Then final event
//     matches direct v3 creation.
// =============================================================================

#[test]
fn s3_given_chained_upcasters_v1_to_v3_when_applied_then_final_version_is_v3() {
    let registry = registry_with_chain_v1_v2_v3();
    let envelope = make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.schema_version, 3, "chained upcast should reach v3");
}

#[test]
fn s3_given_chained_upcasters_v1_to_v3_when_applied_then_all_fields_present() {
    let registry = registry_with_chain_v1_v2_v3();
    let envelope = make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.payload["version"], 3);
    assert_eq!(result.payload["added_in_v2"], "v2-field-value");
    assert_eq!(result.payload["added_in_v3"], "v3-field-value");
}

#[test]
fn s3_given_chained_upcasters_v1_to_v3_when_applied_then_matches_direct_v3_creation() {
    let registry = registry_with_chain_v1_v2_v3();
    let envelope = make_v1_envelope("inst-1", 1, workflow_started_payload("wf-1"));

    let upcasted = registry.upcast_envelope(envelope).unwrap();

    let mut direct_payload = workflow_started_payload("wf-1");
    if let Some(obj) = direct_payload.as_object_mut() {
        obj.insert("version".to_string(), serde_json::json!(3));
        obj.insert(
            "added_in_v2".to_string(),
            serde_json::json!("v2-field-value"),
        );
        obj.insert(
            "added_in_v3".to_string(),
            serde_json::json!("v3-field-value"),
        );
    }
    let direct = make_v2_envelope_direct("inst-1", 1, direct_payload);
    let direct_v3 = EventEnvelope {
        schema_version: 3,
        ..direct
    };

    assert_eq!(upcasted.schema_version, direct_v3.schema_version);
    assert_eq!(upcasted.payload, direct_v3.payload);
    assert_eq!(upcasted.instance_id, direct_v3.instance_id);
    assert_eq!(upcasted.sequence, direct_v3.sequence);
    assert_eq!(upcasted.timestamp_ms, direct_v3.timestamp_ms);
}

#[test]
fn s3_given_full_chain_v0_to_v3_when_applied_then_final_version_is_v3() {
    let registry = registry_with_chain_v0_v1_v2_v3();
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope).unwrap();

    assert_eq!(result.schema_version, 3);
    assert_eq!(result.payload["version"], 3);
    assert_eq!(result.payload["added_in_v2"], "v2-field-value");
    assert_eq!(result.payload["added_in_v3"], "v3-field-value");
}

#[test]
fn s3_given_chained_upcasters_when_replayed_then_lifecycle_succeeds() {
    let engine = ReplayEngine::new();
    let registry = registry_with_chain_v0_v1_v2_v3();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_v0_event("inst-1", 3, step_completed_payload("wf-1", "s1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events);

    assert!(result.is_ok(), "chained replay should succeed: {result:?}");
    assert_eq!(result.unwrap().final_state, Some(LifecycleState::Completed));
}

#[test]
fn s3_given_chained_upcasters_when_replayed_cancelled_lifecycle_then_state_is_cancelled() {
    let engine = ReplayEngine::new();
    let registry = registry_with_chain_v0_v1_v2_v3();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, cancel_requested_payload("wf-1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events).unwrap();

    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
}

#[test]
fn s3_given_chained_upcasters_when_replayed_failed_lifecycle_then_state_is_failed() {
    let engine = ReplayEngine::new();
    let registry = registry_with_chain_v0_v1_v2_v3();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_failed_payload("wf-1", "s1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events).unwrap();

    assert_eq!(result.final_state, Some(LifecycleState::Failed));
}

#[test]
fn s3_given_chained_upcasters_when_gap_in_chain_then_upcast_fails() {
    let registry = UpcasterRegistryImpl::new(3);
    let _ = registry.register(Box::new(V1ToV2Upcaster));
    let _ = registry.register(Box::new(V0ToV1Upcaster));
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));

    let result = registry.upcast_envelope(envelope);

    assert!(result.is_err(), "gap at v2->v3 should fail");
    assert!(
        matches!(result.unwrap_err(), RegistryError::NoUpcasterRegistered(2)),
        "Expected NoUpcasterRegistered(2) for missing v2->v3"
    );
}

#[test]
fn s3_given_chained_upcasters_when_replayed_with_gap_then_upcasting_failed() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(3);
    let _ = registry.register(Box::new(V0ToV1Upcaster));
    let _ = registry.register(Box::new(V1ToV2Upcaster));
    let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];

    let result = engine.replay_with_upcaster(&registry, &events);

    assert!(result.is_err(), "gap should propagate to replay");
    assert!(
        matches!(result.unwrap_err(), ReplayError::UpcastingFailed { .. }),
        "Expected ReplayError::UpcastingFailed"
    );
}

// =============================================================================
// S4: Upcaster chain in vo-core registry and ReplayError::UpcastingFailed
//     integration.
// =============================================================================

#[test]
fn s4_given_registry_with_no_upcasters_when_v0_event_replayed_then_upcasting_failed() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(1);
    let events = [make_v0_event("inst-1", 1, workflow_started_payload("wf-1"))];

    let result = engine.replay_with_upcaster(&registry, &events);

    match result {
        Err(ReplayError::UpcastingFailed { sequence, reason }) => {
            assert_eq!(sequence, 1);
            assert!(reason.contains("0"), "reason should mention version 0");
        }
        other => panic!("Expected UpcastingFailed, got: {other:?}"),
    }
}

#[test]
fn s4_given_valid_upcaster_when_multiple_v0_events_replayed_then_all_upcasted() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(1);
    let _ = registry.register(Box::new(V0ToV1Upcaster));
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_v0_event("inst-1", 3, step_started_payload("wf-1", "s1")),
        make_v0_event("inst-1", 4, step_completed_payload("wf-1", "s1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events).unwrap();

    assert_eq!(result.events_applied, 4);
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
}

#[test]
fn s4_given_mixed_versions_when_replayed_with_chain_then_all_normalized() {
    let engine = ReplayEngine::new();
    let registry = registry_with_chain_v0_v1_v2_v3();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v1_envelope("inst-1", 2, step_scheduled_payload("wf-1", "s1")),
        make_v0_event("inst-1", 3, step_completed_payload("wf-1", "s1")),
    ];

    let result = engine.replay_with_upcaster(&registry, &events);

    assert!(
        result.is_ok(),
        "mixed version replay should succeed: {result:?}"
    );
    assert_eq!(result.unwrap().events_applied, 3);
}

#[test]
fn s4_given_registry_rejects_duplicate_registration_when_same_version_registered_twice() {
    let registry = UpcasterRegistryImpl::new(2);
    let result1 = registry.register(Box::new(V1ToV2Upcaster));
    assert!(result1.is_ok());

    let result2 = registry.register(Box::new(V1ToV2Upcaster));
    assert!(result2.is_err());
    assert_eq!(
        result2.unwrap_err(),
        RegistryError::DuplicateRegistration(1)
    );
}

#[test]
fn s4_given_registry_rejects_target_above_max_when_upcaster_targets_exceeds_max() {
    let registry = UpcasterRegistryImpl::new(1);
    let result = registry.register(Box::new(V1ToV2Upcaster));

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), RegistryError::InvalidTargetVersion(2));
}

#[test]
fn s4_given_decode_event_at_v0_when_envelope_created_then_schema_version_preserved() {
    let json = format!(
        r#"{{"version": 0, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {{"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null}}, "metadata": {{}}}}"#
    );
    let result = decode_event(json.as_bytes());

    assert!(result.is_ok());
    let (envelope, _payload) = result.unwrap();
    assert_eq!(envelope.schema_version, 0);
}

#[test]
fn s4_given_decode_event_above_max_version_when_decoded_then_rejected() {
    let future_version = MAX_SUPPORTED_VERSION + 1;
    let json = format!(
        r#"{{"version": {future_version}, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {{"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": {future_version}}}, "metadata": {{}}}}"#
    );
    let result = decode_event(json.as_bytes());

    assert!(result.is_err(), "future version should be rejected");
}

#[test]
fn s4_given_upcasting_failed_when_error_displayed_then_includes_sequence_and_reason() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(1);
    let events = [make_v0_event(
        "inst-1",
        99,
        workflow_started_payload("wf-1"),
    )];

    let result = engine.replay_with_upcaster(&registry, &events);

    let err = result.unwrap_err();
    let display = format!("{err}");
    assert!(
        display.contains("99"),
        "display should include sequence number"
    );
    assert!(
        display.contains("Upcasting failed"),
        "display should describe the error"
    );
}
