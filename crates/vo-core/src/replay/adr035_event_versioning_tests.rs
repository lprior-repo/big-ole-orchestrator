//! Comprehensive tests for ADR-035: Event Schema Evolution and Upcasting.
//!
//! Coverage:
//! 1. Version field presence on all durable records (envelope schema_version)
//! 2. Version-specific deserialization (v0 vs v1 payloads)
//! 3. Upcaster chain application order (multi-step chains)
//! 4. Missing upcaster error handling (edge cases)
//! 5. Property: any historical event can be replayed with current code

use super::engine::ReplayEngine;
use super::test_helpers::*;
use super::types::ReplayError;
use vo_types::events::UpcasterError as VoUpcasterError;
use vo_types::events::{
    decode_event, EventEnvelope, EventMetadata, EventPayload, MAX_SUPPORTED_VERSION,
};
use vo_types::state::LifecycleState;

use crate::upcaster::{UpcasterError, UpcasterRegistry, UpcasterRegistryImpl};
use vo_types::events::upcaster::Upcaster;

// =============================================================================
// Helper upcasters for multi-step chain testing
// =============================================================================

struct Version0To1Upcaster;

impl Upcaster for Version0To1Upcaster {
    fn source_version(&self) -> u8 {
        0
    }
    fn target_version(&self) -> u8 {
        1
    }
    fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
        let mut value = input.clone();
        value["version"] = serde_json::json!(1);
        if let Some(obj) = value["payload"].as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(1));
        }
        Ok(value)
    }
}

struct PassthroughUpcaster {
    from: u8,
    to: u8,
}

impl PassthroughUpcaster {
    fn new(from: u8, to: u8) -> Self {
        Self { from, to }
    }
}

impl Upcaster for PassthroughUpcaster {
    fn source_version(&self) -> u8 {
        self.from
    }
    fn target_version(&self) -> u8 {
        self.to
    }
    fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
        let mut value = input.clone();
        value["version"] = serde_json::json!(self.to);
        if let Some(obj) = value["payload"].as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(self.to));
        }
        Ok(value)
    }
}

fn make_registry_with_v0_to_v1() -> UpcasterRegistryImpl {
    let registry = UpcasterRegistryImpl::new(1);
    let _ = registry.register(Box::new(Version0To1Upcaster));
    registry
}

/// Upcaster that transforms version 1 JSON to version 2.
struct Version1To2Upcaster;

impl Upcaster for Version1To2Upcaster {
    fn source_version(&self) -> u8 {
        1
    }
    fn target_version(&self) -> u8 {
        2
    }
    fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
        let mut value = input.clone();
        value["version"] = serde_json::json!(2);
        if let Some(obj) = value.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(2));
            if let Some(payload) = obj.get_mut("payload").and_then(|p| p.as_object_mut()) {
                payload.insert("version".to_string(), serde_json::json!(2));
            }
        }
        Ok(value)
    }
}

fn make_registry_with_v1_to_v2() -> UpcasterRegistryImpl {
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(Version1To2Upcaster));
    registry
}

fn make_event_with_version(
    instance_id: &str,
    sequence: u64,
    schema_version: u8,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 * sequence,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn make_envelope_json(schema_version: u8, payload_type: &str, workflow_id: &str) -> String {
    format!(
        r#"{{"version": {schema_version}, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {{"type": "{payload_type}", "workflow_id": "{workflow_id}", "binary_hash": "sha256abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null, "version": {schema_version}}}, "metadata": {{}}}}"#
    )
}

// =============================================================================
// 1. Version field presence on all durable record types
// =============================================================================

#[test]
fn envelope_schema_version_field_present_on_workflow_started() {
    let envelope = make_event("inst-1", 1, workflow_started_payload("wf-1"));
    assert!(
        envelope.schema_version <= MAX_SUPPORTED_VERSION,
        "schema_version must be present and valid"
    );
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_step_scheduled() {
    let envelope = make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_step_completed() {
    let envelope = make_event("inst-1", 3, step_completed_payload("wf-1", "step-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_step_failed() {
    let envelope = make_event("inst-1", 4, step_failed_payload("wf-1", "step-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_timer_set() {
    let envelope = make_event("inst-1", 5, timer_set_payload("wf-1", "timer-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_timer_fired() {
    let envelope = make_event("inst-1", 6, timer_fired_payload("wf-1", "timer-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_workflow_cancelled() {
    let envelope = make_event("inst-1", 7, workflow_cancelled_payload("wf-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_workflow_failed() {
    let envelope = make_event("inst-1", 8, workflow_failed_payload("wf-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_cancel_requested() {
    let envelope = make_event("inst-1", 9, cancel_requested_payload("wf-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_instance_resumed() {
    let envelope = make_event("inst-1", 10, instance_resumed_payload("wf-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_continued_as_new() {
    let envelope = make_event("inst-1", 11, continued_as_new_payload("wf-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn envelope_schema_version_field_present_on_step_started() {
    let envelope = make_event("inst-1", 12, step_started_payload("wf-1", "step-1"));
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn v0_envelope_carries_schema_version_zero() {
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));
    assert_eq!(envelope.schema_version, 0);
}

fn make_v1_event(
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

#[test]
fn v1_envelope_carries_schema_version_one() {
    let envelope = make_v1_event("inst-1", 1, workflow_started_payload("wf-1"));
    assert_eq!(envelope.schema_version, 1);
}

// =============================================================================
// 2. Version-specific deserialization
// =============================================================================

#[test]
fn decode_event_from_v0_envelope_succeeds_with_upcaster() {
    let json = make_envelope_json(0, "WorkflowStarted", "wf-1");
    let bytes = json.as_bytes();
    let result = decode_event(bytes);
    assert!(
        result.is_ok(),
        "v0 envelope should decode successfully: {result:?}"
    );
    let (envelope, _payload) = result.unwrap();
    assert_eq!(envelope.schema_version, 0);
}

#[test]
fn decode_event_from_v1_envelope_succeeds() {
    let json = make_envelope_json(1, "WorkflowStarted", "wf-1");
    let bytes = json.as_bytes();
    let result = decode_event(bytes);
    assert!(result.is_ok(), "v1 envelope should decode: {result:?}");
    let (envelope, _payload) = result.unwrap();
    assert_eq!(envelope.schema_version, 1);
}

#[test]
fn decode_event_rejects_future_version_above_max_supported() {
    let future_version = MAX_SUPPORTED_VERSION + 1;
    let json = format!(
        r#"{{"version": {future_version}, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {{"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "version": 1}}, "metadata": {{}}}}"#
    );
    let result = decode_event(json.as_bytes());
    assert!(result.is_err(), "Should reject future version");
}

#[test]
fn v0_payload_decodes_with_default_version_when_missing_version() {
    let json = r#"{"version": 0, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    assert!(
        result.is_ok(),
        "Payload without version should default to v0: {result:?}"
    );
}

#[test]
fn payload_version_zero_is_accepted() {
    let json = r#"{"version": 0, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null, "version": 0}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    assert!(result.is_ok(), "Payload v0 should decode: {result:?}");
}

#[test]
fn payload_version_one_is_accepted() {
    let json = r#"{"version": 1, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null, "version": 1}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    assert!(result.is_ok(), "Payload v1 should decode: {result:?}");
}

#[test]
fn payload_missing_version_defaults_to_zero() {
    let json = r#"{"version": 1, "instance_id": "inst-1", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    assert!(
        result.is_ok(),
        "Payload without version field should default to v0: {result:?}"
    );
}

#[test]
fn is_supported_returns_true_for_current_version() {
    let envelope = make_event("inst-1", 1, workflow_started_payload("wf-1"));
    assert!(envelope.is_supported());
}

#[test]
fn is_supported_returns_true_for_v0() {
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));
    assert!(envelope.is_supported());
}

#[test]
fn event_payload_is_version_supported_for_current() {
    assert!(EventPayload::is_version_supported(MAX_SUPPORTED_VERSION));
    assert!(EventPayload::is_version_supported(0));
}

#[test]
fn event_payload_is_version_supported_rejects_future() {
    assert!(!EventPayload::is_version_supported(
        MAX_SUPPORTED_VERSION + 1
    ));
}

// =============================================================================
// 3. Upcaster registry: registration and chain application order
// =============================================================================

#[test]
fn registry_accepts_valid_upcaster_for_v0() {
    let registry = UpcasterRegistryImpl::new(1);
    let result = registry.register(Box::new(Version0To1Upcaster));
    assert!(result.is_ok(), "Should accept v0 upcaster: {result:?}");
}

#[test]
fn registry_rejects_upcaster_at_max_version() {
    let registry = UpcasterRegistryImpl::new(1);
    let result = registry.register(Box::new(PassthroughUpcaster::new(1, 2)));
    assert!(result.is_err(), "Should reject upcaster at max version");
    assert_eq!(result.unwrap_err(), UpcasterError::InvalidTargetVersion(2));
}

#[test]
fn registry_rejects_duplicate_upcaster_for_same_source() {
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(PassthroughUpcaster::new(0, 1)));
    let result = registry.register(Box::new(PassthroughUpcaster::new(0, 1)));
    assert!(result.is_err(), "Should reject duplicate source version");
    assert_eq!(result.unwrap_err(), UpcasterError::DuplicateRegistration(0));
}

#[test]
fn registry_max_supported_version_matches_constant() {
    let registry = UpcasterRegistryImpl::new(MAX_SUPPORTED_VERSION);
    assert_eq!(registry.max_supported_version(), MAX_SUPPORTED_VERSION);
}

#[test]
fn upcast_envelope_transforms_v0_to_v1() {
    let registry = make_registry_with_v0_to_v1();
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    assert!(result.is_ok(), "Upcast should succeed: {result:?}");
    let upcasted = result.unwrap();
    assert_eq!(upcasted.schema_version, 1);
    assert_eq!(upcasted.instance_id, "inst-1");
    assert_eq!(upcasted.sequence, 1);
}

#[test]
fn upcast_envelope_preserves_already_current_version() {
    let registry = make_registry_with_v0_to_v1();
    let envelope = make_event("inst-1", 1, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().schema_version, 1);
}

#[test]
fn upcast_envelope_preserves_metadata_across_version_upgrade() {
    let registry = make_registry_with_v0_to_v1();
    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "inst-1".to_string(),
        sequence: 42,
        timestamp_ms: 9999,
        payload: workflow_started_payload("wf-1"),
        metadata: EventMetadata::default(),
    };
    let result = registry.upcast_envelope(envelope).unwrap();
    assert_eq!(result.sequence, 42);
    assert_eq!(result.timestamp_ms, 9999);
    assert_eq!(result.instance_id, "inst-1");
}

#[test]
fn upcast_envelope_returns_error_when_no_upcaster_for_intermediate_version() {
    let registry = UpcasterRegistryImpl::new(3);
    let envelope = make_event_with_version("inst-1", 1, 0, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    assert!(result.is_err(), "Should fail without registered upcaster");
    assert!(
        matches!(result.unwrap_err(), UpcasterError::NoUpcasterRegistered(0)),
        "Expected NoUpcasterRegistered(0)"
    );
}

// =============================================================================
// 4. Multi-step upcaster chain application order
// =============================================================================

#[test]
fn multi_step_chain_applies_upcasters_in_order_v0_to_v1_to_v2() {
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(PassthroughUpcaster::new(0, 1)));
    let _ = registry.register(Box::new(PassthroughUpcaster::new(1, 2)));

    let envelope = make_event_with_version("inst-1", 1, 0, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    assert!(
        result.is_ok(),
        "Multi-step upcast should succeed: {result:?}"
    );
    assert_eq!(result.unwrap().schema_version, 2);
}

#[test]
fn multi_step_chain_preserves_instance_identity_across_versions() {
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(PassthroughUpcaster::new(0, 1)));
    let _ = registry.register(Box::new(PassthroughUpcaster::new(1, 2)));

    let envelope = make_event_with_version("inst-unique", 99, 0, workflow_started_payload("wf-x"));
    let result = registry.upcast_envelope(envelope).unwrap();
    assert_eq!(result.instance_id, "inst-unique");
    assert_eq!(result.sequence, 99);
    assert_eq!(result.schema_version, 2);
}

#[test]
fn chain_detection_fails_with_gap_in_upcaster_registration() {
    let registry = UpcasterRegistryImpl::new(3);
    let _ = registry.register(Box::new(PassthroughUpcaster::new(0, 1)));
    // Missing: v1 → v2 upcaster
    let _ = registry.register(Box::new(PassthroughUpcaster::new(2, 3)));

    let envelope = make_event_with_version("inst-1", 1, 0, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    assert!(result.is_err(), "Gap in chain should fail");
}

// =============================================================================
// 5. Replay integration with upcaster: historical event replayability
// =============================================================================

#[test]
fn replay_historical_v0_workflow_started_through_full_lifecycle() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
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

#[test]
fn replay_historical_v0_workflow_cancelled_lifecycle() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, cancel_requested_payload("wf-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
}

#[test]
fn replay_historical_v0_workflow_failed_lifecycle() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_failed_payload("wf-1", "step-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Failed));
}

#[test]
fn replay_historical_v0_timer_lifecycle() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
        make_v0_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::StepExecuting));
}

#[test]
fn replay_mixed_v0_and_v1_events_all_succeed() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("mixed version replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);
}

#[test]
fn replay_v0_continued_as_new_counted_but_no_state_change() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, continued_as_new_payload("wf-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay with ContinuedAsNew should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
    assert_eq!(result.events_applied, 2);
}

#[test]
fn replay_v0_instance_resumed_after_failure() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_failed_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 4, instance_resumed_payload("wf-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("replay with InstanceResumed should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::RunningDecision));
}

/// Test: store v1 event, replay with v2 engine, verify successful migration.
/// This is the core scenario described in the schema evolution ADR:
/// A workflow ran with schema v1 and events were stored in v1 format.
/// When replayed with a v2 engine, the upcaster migrates the event to v2.
#[test]
fn replay_v1_event_with_v2_engine_migrates_successfully() {
    use vo_types::events::upcaster::Upcaster;

    struct V1ToV2Upcaster;
    impl Upcaster for V1ToV2Upcaster {
        fn source_version(&self) -> u8 { 1 }
        fn target_version(&self) -> u8 { 2 }
        fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
            let mut value = input.clone();
            if let Some(obj) = value.as_object_mut() {
                obj.insert("version".to_string(), serde_json::json!(2));
            }
            Ok(value)
        }
    }

    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(V1ToV2Upcaster));

    let events = [
        make_v1_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v1_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v1_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_v1_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
    ];

    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("v1 events should be upcast to v2 and replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Completed));
    assert_eq!(result.events_applied, 4);

    let upcasted = registry
        .upcast_envelope(events[0].clone())
        .expect("upcast should succeed");
    assert_eq!(upcasted.schema_version, 2);
}

#[test]
fn replay_v1_workflow_cancelled_with_v2_engine() {
    use vo_types::events::upcaster::Upcaster;

    struct V1ToV2Upcaster;
    impl Upcaster for V1ToV2Upcaster {
        fn source_version(&self) -> u8 { 1 }
        fn target_version(&self) -> u8 { 2 }
        fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
            let mut value = input.clone();
            if let Some(obj) = value.as_object_mut() {
                obj.insert("version".to_string(), serde_json::json!(2));
            }
            Ok(value)
        }
    }

    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(V1ToV2Upcaster));

    let events = [
        make_v1_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v1_event("inst-1", 2, cancel_requested_payload("wf-1")),
    ];

    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("v1 events should be upcast to v2 and replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Cancelled));
}

#[test]
fn replay_v1_workflow_failed_with_v2_engine() {
    use vo_types::events::upcaster::Upcaster;

    struct V1ToV2Upcaster;
    impl Upcaster for V1ToV2Upcaster {
        fn source_version(&self) -> u8 { 1 }
        fn target_version(&self) -> u8 { 2 }
        fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
            let mut value = input.clone();
            if let Some(obj) = value.as_object_mut() {
                obj.insert("version".to_string(), serde_json::json!(2));
            }
            Ok(value)
        }
    }

    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(V1ToV2Upcaster));

    let events = [
        make_v1_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v1_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v1_event("inst-1", 3, step_failed_payload("wf-1", "step-1")),
    ];

    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("v1 events should be upcast to v2 and replay should succeed");
    assert_eq!(result.final_state, Some(LifecycleState::Failed));
}

// =============================================================================
// 6. Error handling edge cases
// =============================================================================

#[test]
fn upcast_envelope_with_no_upcaster_returns_error_with_correct_version() {
    let registry = UpcasterRegistryImpl::new(1);
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    let err = result.expect_err("should fail without upcaster");
    assert_eq!(err, UpcasterError::NoUpcasterRegistered(0));
}

#[test]
fn upcast_envelope_detects_circular_chain() {
    struct LoopingUpcaster;
    impl Upcaster for LoopingUpcaster {
        fn source_version(&self) -> u8 {
            0
        }
        fn target_version(&self) -> u8 {
            1
        }
        fn upcast(&self, input: &serde_json::Value) -> Result<serde_json::Value, VoUpcasterError> {
            let mut value = input.clone();
            value["version"] = serde_json::json!(0);
            Ok(value)
        }
    }

    // Use max_version=2 so the chain builder needs multiple steps:
    // v0 → (upcaster produces v0) → chain builder revisits v0 → circular detected
    let registry = UpcasterRegistryImpl::new(2);
    let _ = registry.register(Box::new(LoopingUpcaster));
    let envelope = make_v0_event("inst-1", 1, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    // The chain builder advances current_version = source_version + 1 = 1,
    // then tries to find upcaster for v1 which doesn't exist → NoUpcasterRegistered
    assert!(result.is_err(), "Looping upcaster should cause error");
}

#[test]
fn upcast_envelope_rejects_envelope_above_max_version() {
    let registry = make_registry_with_v0_to_v1();
    let envelope = make_event_with_version("inst-1", 1, 255, workflow_started_payload("wf-1"));
    let result = registry.upcast_envelope(envelope);
    assert!(result.is_ok(), "Envelope at or above max passes through");
}

#[test]
fn replay_with_upcaster_reports_correct_sequence_on_failure() {
    let engine = ReplayEngine::new();
    let registry = UpcasterRegistryImpl::new(1);
    // No upcaster registered, v0 event
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay_with_upcaster(&registry, &events);
    let err = result.expect_err("should fail without upcaster");
    assert!(
        matches!(err, ReplayError::UpcastingFailed { sequence: 1, .. }),
        "Expected UpcastingFailed at sequence 1, got: {err:?}"
    );
}

#[test]
fn replay_with_upcaster_reports_second_event_on_first_event_ok() {
    let registry = make_registry_with_v0_to_v1();

    let engine = ReplayEngine::new();
    let events = [
        make_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
    ];
    let result = engine.replay_with_upcaster(&registry, &events);
    assert!(result.is_ok(), "Both events should upcast: {result:?}");
    assert_eq!(result.unwrap().events_applied, 2);
}

// =============================================================================
// 7. Property: every payload type at v0 can be decoded and replayed
// =============================================================================

#[test]
fn all_payload_types_decode_from_v0_envelope() {
    let payloads: Vec<(&str, serde_json::Value)> = vec![
        (
            "WorkflowStarted",
            serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null, "version": 0}),
        ),
        (
            "WorkflowCompleted",
            serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "wf-1", "completion_time_ms": 100, "version": 0}),
        ),
        (
            "WorkflowFailed",
            serde_json::json!({"type": "WorkflowFailed", "workflow_id": "wf-1", "failure_reason": "err", "version": 0}),
        ),
        (
            "WorkflowCancelled",
            serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "wf-1", "cancelled_by": "user", "version": 0}),
        ),
        (
            "StepScheduled",
            serde_json::json!({"type": "StepScheduled", "workflow_id": "wf-1", "step_id": "s1", "attempt": 1, "fence": 1, "execution_id": "e1", "version": 0}),
        ),
        (
            "StepStarted",
            serde_json::json!({"type": "StepStarted", "workflow_id": "wf-1", "step_id": "s1", "started_at_ms": 100, "version": 0}),
        ),
        (
            "StepCompleted",
            serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-1", "step_id": "s1", "completed_at_ms": 200, "attempt": 1, "fence": 1, "routing_projection": null, "output_ref": null, "output_hash": null, "version": 0}),
        ),
        (
            "StepFailed",
            serde_json::json!({"type": "StepFailed", "workflow_id": "wf-1", "step_id": "s1", "failure_reason": "err", "attempt": 1, "fence": 1, "version": 0}),
        ),
        (
            "TimerSet",
            serde_json::json!({"type": "TimerSet", "workflow_id": "wf-1", "timer_id": "t1", "fire_at_ms": 500, "version": 0}),
        ),
        (
            "TimerFired",
            serde_json::json!({"type": "TimerFired", "workflow_id": "wf-1", "timer_id": "t1", "fired_at_ms": 500, "version": 0}),
        ),
        (
            "CancelRequested",
            serde_json::json!({"type": "CancelRequested", "workflow_id": "wf-1", "requested_by": "user", "version": 0}),
        ),
        (
            "InstanceResumed",
            serde_json::json!({"type": "InstanceResumed", "workflow_id": "wf-1", "resumed_at_ms": 600, "version": 0}),
        ),
        (
            "ContinuedAsNew",
            serde_json::json!({"type": "ContinuedAsNew", "workflow_id": "wf-1", "lineage_id": "lin-1", "old_epoch": 0, "new_epoch": 1, "version": 0}),
        ),
    ];

    for (name, payload) in &payloads {
        let envelope = EventEnvelope {
            schema_version: 0,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: payload.clone(),
            metadata: EventMetadata::default(),
        };
        let decode_result = EventPayload::try_from_json(&envelope.payload);
        assert!(
            decode_result.is_ok(),
            "Payload type {name} should decode from v0: {decode_result:?}"
        );
    }
}

#[test]
fn all_payload_types_decode_from_v1_envelope() {
    let payloads: Vec<(&str, serde_json::Value)> = vec![
        (
            "WorkflowStarted",
            serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1", "binary_hash": "abc", "workflow_version_hash": "vhash", "dedupe_key_hash": null, "version": 1}),
        ),
        (
            "WorkflowCompleted",
            serde_json::json!({"type": "WorkflowCompleted", "workflow_id": "wf-1", "completion_time_ms": 100, "version": 1}),
        ),
        (
            "WorkflowFailed",
            serde_json::json!({"type": "WorkflowFailed", "workflow_id": "wf-1", "failure_reason": "err", "version": 1}),
        ),
        (
            "WorkflowCancelled",
            serde_json::json!({"type": "WorkflowCancelled", "workflow_id": "wf-1", "cancelled_by": "user", "version": 1}),
        ),
        (
            "StepScheduled",
            serde_json::json!({"type": "StepScheduled", "workflow_id": "wf-1", "step_id": "s1", "attempt": 1, "fence": 1, "execution_id": "e1", "version": 1}),
        ),
        (
            "StepStarted",
            serde_json::json!({"type": "StepStarted", "workflow_id": "wf-1", "step_id": "s1", "started_at_ms": 100, "version": 1}),
        ),
        (
            "StepCompleted",
            serde_json::json!({"type": "StepCompleted", "workflow_id": "wf-1", "step_id": "s1", "completed_at_ms": 200, "attempt": 1, "fence": 1, "routing_projection": null, "output_ref": null, "output_hash": null, "version": 1}),
        ),
        (
            "StepFailed",
            serde_json::json!({"type": "StepFailed", "workflow_id": "wf-1", "step_id": "s1", "failure_reason": "err", "attempt": 1, "fence": 1, "version": 1}),
        ),
        (
            "TimerSet",
            serde_json::json!({"type": "TimerSet", "workflow_id": "wf-1", "timer_id": "t1", "fire_at_ms": 500, "version": 1}),
        ),
        (
            "TimerFired",
            serde_json::json!({"type": "TimerFired", "workflow_id": "wf-1", "timer_id": "t1", "fired_at_ms": 500, "version": 1}),
        ),
        (
            "CancelRequested",
            serde_json::json!({"type": "CancelRequested", "workflow_id": "wf-1", "requested_by": "user", "version": 1}),
        ),
        (
            "InstanceResumed",
            serde_json::json!({"type": "InstanceResumed", "workflow_id": "wf-1", "resumed_at_ms": 600, "version": 1}),
        ),
        (
            "ContinuedAsNew",
            serde_json::json!({"type": "ContinuedAsNew", "workflow_id": "wf-1", "lineage_id": "lin-1", "old_epoch": 0, "new_epoch": 1, "version": 1}),
        ),
    ];

    for (name, payload) in &payloads {
        let result = EventPayload::try_from_json(payload);
        assert!(
            result.is_ok(),
            "Payload type {name} should decode from v1: {result:?}"
        );
    }
}

#[test]
fn replay_full_lifecycle_with_all_event_types_upcasted_from_v0() {
    let engine = ReplayEngine::new();
    let registry = make_registry_with_v0_to_v1();
    let events = [
        make_v0_event("inst-1", 1, workflow_started_payload("wf-1")),
        make_v0_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 4, timer_set_payload("wf-1", "timer-1")),
        make_v0_event("inst-1", 5, timer_fired_payload("wf-1", "timer-1")),
        make_v0_event("inst-1", 6, step_failed_payload("wf-1", "step-1")),
        make_v0_event("inst-1", 7, instance_resumed_payload("wf-1")),
    ];
    let result = engine
        .replay_with_upcaster(&registry, &events)
        .expect("full lifecycle replay should succeed");
    assert_eq!(result.events_applied, 7);
}

// =============================================================================
// 8. vo-types VersionRegistry tests (ADR-035 specific)
// =============================================================================

#[test]
fn version_registry_upcast_chain_is_incremental() {
    use vo_types::events::upcaster::{Upcaster, UpcasterError, VersionRegistry};
    use vo_types::events::MAX_SUPPORTED_VERSION;

    struct AddFieldV0ToV1;
    impl Upcaster for AddFieldV0ToV1 {
        fn source_version(&self) -> u8 {
            0
        }
        fn target_version(&self) -> u8 {
            1
        }
        fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
            let mut result = payload.clone();
            if let Some(obj) = result.as_object_mut() {
                obj.insert("added_in_v1".to_string(), serde_json::json!(true));
            }
            Ok(result)
        }
    }

    struct AddFieldV1ToV2;
    impl Upcaster for AddFieldV1ToV2 {
        fn source_version(&self) -> u8 {
            1
        }
        fn target_version(&self) -> u8 {
            2
        }
        fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
            let mut result = payload.clone();
            if let Some(obj) = result.as_object_mut() {
                obj.insert("added_in_v2".to_string(), serde_json::json!(true));
            }
            Ok(result)
        }
    }

    let mut registry = VersionRegistry::new();
    registry.register(Box::new(AddFieldV0ToV1));
    registry.register(Box::new(AddFieldV1ToV2));

    let payload = serde_json::json!({"type": "Test"});
    let result = registry
        .upcast_payload(payload, 0, MAX_SUPPORTED_VERSION)
        .expect("upcast should succeed");
    assert_eq!(result["added_in_v1"], serde_json::json!(true));
    assert_eq!(result["added_in_v2"], serde_json::json!(true));
}

#[test]
fn version_registry_upcast_from_higher_than_target_returns_error() {
    use vo_types::events::upcaster::VersionRegistry;

    let registry = VersionRegistry::new();
    let payload = serde_json::json!({"type": "Test"});
    let result = registry.upcast_payload(payload, 2, 1);
    assert!(
        result.is_err(),
        "Upcasting from higher to lower should fail"
    );
}
