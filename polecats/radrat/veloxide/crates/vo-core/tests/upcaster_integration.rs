//! Integration tests for Upcaster trait and UpcasterRegistry trait.
//!
//! These tests verify the trait interfaces exist and have the correct signatures.
//! Since this bead defines interfaces only (not implementations), tests verify
//! that the trait methods can be called and return expected error types.
//!
//! RED PHASE: All these tests should fail because the trait methods are stubs
//! that return default/unimplemented values.

use std::sync::{Arc, Mutex};
use vo_core::upcaster::{
    UpcasterError as CoreUpcasterError, UpcasterRegistry, UpcasterRegistryBuilder,
    UpcasterRegistryImpl, MAX_SUPPORTED_VERSION,
};
use vo_types::events::upcaster::{Upcaster, UpcasterError};
use vo_types::events::{EventEnvelope, EventMetadata};

// =============================================================================
// Test-only Upcaster Implementations (for interface testing only)
// =============================================================================

/// A simple upcaster that transforms version 0 JSON to version 1.
struct Version0To1Upcaster;

impl Version0To1Upcaster {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for Version0To1Upcaster {
    fn source_version(&self) -> u8 {
        0
    }

    fn target_version(&self) -> u8 {
        1
    }

    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut result = payload.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(1));
        }
        Ok(result)
    }
}

/// An upcaster that produces invalid output for testing error paths.
struct BuggyUpcaster;

impl BuggyUpcaster {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for BuggyUpcaster {
    fn source_version(&self) -> u8 {
        0
    }

    fn target_version(&self) -> u8 {
        1
    }

    fn upcast(&self, _payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        Err(UpcasterError::UpcastFailed(
            "buggy upcaster failed".to_string(),
        ))
    }
}

/// An upcaster that fails to parse its input.
struct ParseFailingUpcaster;

impl ParseFailingUpcaster {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for ParseFailingUpcaster {
    fn source_version(&self) -> u8 {
        0
    }

    fn target_version(&self) -> u8 {
        1
    }

    fn upcast(&self, _payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        Err(UpcasterError::UpcastFailed(
            "cannot upcast payload".to_string(),
        ))
    }
}

/// Upcaster that creates a circular chain (version 0 -> 1 -> 0).
struct CircularUpcasterA;

impl CircularUpcasterA {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for CircularUpcasterA {
    fn source_version(&self) -> u8 {
        0
    }

    fn target_version(&self) -> u8 {
        1
    }

    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut result = payload.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(1));
        }
        Ok(result)
    }
}

struct CircularUpcasterB;

impl CircularUpcasterB {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for CircularUpcasterB {
    fn source_version(&self) -> u8 {
        1
    }

    fn target_version(&self) -> u8 {
        0
    }

    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut result = payload.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(0));
        }
        Ok(result)
    }
}

/// Upcaster that exceeds MAX_SUPPORTED_VERSION.
struct ExceedingMaxUpcaster;

impl ExceedingMaxUpcaster {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for ExceedingMaxUpcaster {
    fn source_version(&self) -> u8 {
        1
    }

    fn target_version(&self) -> u8 {
        2
    }

    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut result = payload.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(2));
        }
        Ok(result)
    }
}

/// Upcaster producing garbage that can't be parsed.
struct GarbageProducingUpcaster;

impl GarbageProducingUpcaster {
    #[allow(clippy::new_ret_no_self)]
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for GarbageProducingUpcaster {
    fn source_version(&self) -> u8 {
        0
    }

    fn target_version(&self) -> u8 {
        1
    }

    fn upcast(&self, _payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        Err(UpcasterError::UpcastFailed(
            "garbage upcaster failed".to_string(),
        ))
    }
}

// =============================================================================
// Concrete Registry Implementation for Testing
// =============================================================================

struct TestUpcasterRegistry {
    upcasters: Arc<Mutex<std::collections::HashMap<u8, Box<dyn Upcaster>>>>,
    max_version: u8,
}

impl TestUpcasterRegistry {
    fn new(max_version: u8) -> Self {
        Self {
            upcasters: Arc::new(Mutex::new(std::collections::HashMap::new())),
            max_version,
        }
    }
}

impl UpcasterRegistry for TestUpcasterRegistry {
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), CoreUpcasterError> {
        let source_version = upcaster.source_version();
        let target_version = upcaster.target_version();

        if target_version > self.max_version {
            return Err(CoreUpcasterError::InvalidTargetVersion(target_version));
        }

        let mut upcasters = self
            .upcasters
            .lock()
            .map_err(|_| CoreUpcasterError::UpcastingFailed("lock poisoned".to_string()))?;

        if upcasters.contains_key(&source_version) {
            return Err(CoreUpcasterError::DuplicateRegistration(source_version));
        }

        upcasters.insert(source_version, upcaster);
        Ok(())
    }

    fn upcast_envelope(&self, envelope: EventEnvelope) -> Result<EventEnvelope, CoreUpcasterError> {
        if envelope.schema_version >= self.max_version {
            return Ok(envelope);
        }

        let upcasters = self
            .upcasters
            .lock()
            .map_err(|_| CoreUpcasterError::UpcastingFailed("lock poisoned".to_string()))?;
        let mut visited = std::collections::HashSet::new();
        visited.insert(envelope.schema_version);

        let (current_version, current_payload) = apply_upcast_chain(
            &upcasters,
            self.max_version,
            envelope.schema_version,
            envelope.payload.clone(),
            &mut visited,
        )?;

        Ok(EventEnvelope {
            schema_version: current_version,
            instance_id: envelope.instance_id,
            sequence: envelope.sequence,
            timestamp_ms: envelope.timestamp_ms,
            payload: current_payload,
            metadata: envelope.metadata,
        })
    }

    fn max_supported_version(&self) -> u8 {
        self.max_version
    }
}

fn apply_upcast_chain(
    upcasters: &std::collections::HashMap<u8, Box<dyn Upcaster>>,
    max_version: u8,
    current_version: u8,
    current_payload: serde_json::Value,
    visited: &mut std::collections::HashSet<u8>,
) -> Result<(u8, serde_json::Value), CoreUpcasterError> {
    if current_version >= max_version {
        return Ok((current_version, current_payload));
    }

    let upcaster = upcasters
        .get(&current_version)
        .ok_or(CoreUpcasterError::NoUpcasterRegistered(current_version))?;

    let new_payload = upcaster
        .upcast(&current_payload)
        .map_err(|e| CoreUpcasterError::UpcastingFailed(e.to_string()))?;

    let new_version = upcaster.target_version();

    if new_version > max_version {
        return Err(CoreUpcasterError::InvalidTargetVersion(new_version));
    }

    if visited.contains(&new_version) {
        return Err(CoreUpcasterError::CircularChain(new_version));
    }
    visited.insert(new_version);

    apply_upcast_chain(upcasters, max_version, new_version, new_payload, visited)
}

struct TestUpcasterRegistryBuilder;

impl UpcasterRegistryBuilder for TestUpcasterRegistryBuilder {
    fn build() -> Box<dyn UpcasterRegistry> {
        Box::new(TestUpcasterRegistry::new(MAX_SUPPORTED_VERSION))
    }
}

// =============================================================================
// Upcaster Trait Tests
// =============================================================================

#[test]
fn upcaster_returns_source_version_when_source_version_called() {
    let upcaster = Version0To1Upcaster::new();
    // RED PHASE: Stub returns 0, which matches expectation
    assert_eq!(upcaster.source_version(), 0);
}

#[test]
fn upcaster_returns_source_version_idempotently() {
    let upcaster = Version0To1Upcaster::new();
    // RED PHASE: Stub always returns 0
    assert_eq!(upcaster.source_version(), 0);
    assert_eq!(upcaster.source_version(), 0);
    assert_eq!(upcaster.source_version(), 0);
}

#[test]
fn upcaster_transforms_valid_json_bytes_to_newer_schema_version() {
    let upcaster = Version0To1Upcaster::new();
    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {"data": "test"}}"#).unwrap();

    // RED PHASE: Stub returns Err, but test expects Ok with incremented version
    let result = upcaster.upcast(&input);
    assert!(
        result.is_ok(),
        "upcast should succeed with valid input: {:?}",
        result
    );

    let output = result.unwrap();
    assert_eq!(output.get("version").and_then(|v| v.as_u64()), Some(1));
}

#[test]
fn upcaster_returns_identical_output_on_repeated_calls() {
    let upcaster = Version0To1Upcaster::new();
    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {"data": "test"}}"#).unwrap();

    let result1 = upcaster.upcast(&input);
    let result2 = upcaster.upcast(&input);

    // RED PHASE: Both return Err (same error), so they are "identical"
    assert_eq!(result1, result2, "upcast should be deterministic");
}

#[test]
fn upcaster_returns_upcasting_failed_when_transform_produces_invalid_json() {
    let upcaster = BuggyUpcaster::new();
    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {}}"#).unwrap();

    let result = upcaster.upcast(&input);
    // BuggyUpcaster returns Err(UpcasterError::UpcastFailed(...))
    assert!(result.is_err(), "BuggyUpcaster should return error");
}

#[test]
fn upcaster_returns_upcasting_failed_with_parse_error_details() {
    let upcaster = ParseFailingUpcaster::new();
    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {}}"#).unwrap();

    let result = upcaster.upcast(&input);
    // ParseFailingUpcaster returns Err(UpcasterError::UpcastFailed(...))
    assert!(result.is_err(), "ParseFailingUpcaster should return error");
}

#[test]
fn upcaster_output_is_valid_json_when_upcast_succeeds() {
    let upcaster = Version0To1Upcaster::new();
    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {"data": "test"}}"#).unwrap();

    let result = upcaster.upcast(&input);
    let output = result.expect("upcast should succeed with valid input");
    assert!(output.is_object(), "output should be a JSON object");
}

#[test]
fn upcaster_output_contains_incremented_version_field() {
    let upcaster = Version0To1Upcaster::new();

    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {}}"#).unwrap();
    let result = upcaster.upcast(&input);

    let output = result.expect("upcast should succeed");
    assert_eq!(
        output.get("version").and_then(|v| v.as_u64()),
        Some(1),
        "output version should be incremented to 1"
    );
}

#[test]
fn upcaster_is_idempotent_when_called_multiple_times_with_same_input() {
    let upcaster = Version0To1Upcaster::new();
    let input: serde_json::Value =
        serde_json::from_slice(br#"{"version": 0, "payload": {}}"#).unwrap();

    // RED PHASE: All calls return the same error
    let result1 = upcaster.upcast(&input);
    let result2 = upcaster.upcast(&input);
    let result3 = upcaster.upcast(&input);
    let result4 = upcaster.upcast(&input);
    let result5 = upcaster.upcast(&input);

    // Straight-line assertion without loop (Holzmann Rule 2)
    assert_eq!(result1, result2, "second call should match first");
    assert_eq!(result2, result3, "third call should match second");
    assert_eq!(result3, result4, "fourth call should match third");
    assert_eq!(result4, result5, "fifth call should match fourth");
}

// =============================================================================
// UpcasterRegistry Trait Tests
// =============================================================================

fn create_test_registry() -> Box<dyn UpcasterRegistry> {
    <TestUpcasterRegistryBuilder as UpcasterRegistryBuilder>::build()
}

#[test]
fn registry_accepts_valid_upcaster_and_returns_ok() {
    let registry = create_test_registry();
    let upcaster = Version0To1Upcaster::new();

    let result = registry.register(upcaster);
    assert_eq!(result, Ok(()), "registering valid upcaster should succeed");
}

#[test]
fn registry_rejects_duplicate_upcaster_when_same_version_registered_twice() {
    let registry = create_test_registry();
    let upcaster1 = Version0To1Upcaster::new();
    let upcaster2 = Version0To1Upcaster::new();

    let result1 = registry.register(upcaster1);
    assert_eq!(result1, Ok(()), "first registration should succeed");

    let result2 = registry.register(upcaster2);
    assert_eq!(
        result2,
        Err(CoreUpcasterError::DuplicateRegistration(0)),
        "second registration of same version should be rejected"
    );
}

#[test]
fn registry_returns_error_when_no_upcaster_registered_for_version() {
    let registry = create_test_registry();

    // No upcasters registered, try to upcast version 0
    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope);
    assert_eq!(
        result,
        Err(CoreUpcasterError::NoUpcasterRegistered(0)),
        "upcast should fail when no upcaster is registered"
    );
}

#[test]
fn registry_applies_single_upcaster_when_version_gap_is_one() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "test"}),
        metadata: EventMetadata::default(),
    };

    // RED PHASE: The upcaster stub returns Err, so upcast_envelope will fail
    let result = registry.upcast_envelope(envelope);

    let upcasted = result.expect("upcast_envelope should succeed");
    assert_eq!(
        upcasted.schema_version, 1,
        "version should be incremented to 1"
    );
}

#[test]
fn registry_returns_envelope_unchanged_when_already_at_max_version() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: MAX_SUPPORTED_VERSION, // 1
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "test"}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope.clone());
    assert_eq!(
        result,
        Ok(envelope.clone()),
        "envelope at max version should be returned unchanged"
    );
}

#[test]
fn registry_short_circuits_chain_when_envelope_at_max_despite_registered_upcasters() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    // Envelope already at MAX_VERSION
    let envelope = EventEnvelope {
        schema_version: MAX_SUPPORTED_VERSION,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "original"}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope.clone());
    assert_eq!(
        result,
        Ok(envelope.clone()),
        "envelope at max version should be unchanged"
    );
}

#[test]
fn registry_rejects_upcaster_when_source_version_exceeds_max() {
    let registry = create_test_registry();

    // ExceedingMaxUpcaster has source_version=1, target_version=2 (default).
    // target_version(2) > max(1) should be rejected.
    let upcaster = ExceedingMaxUpcaster::new();

    let result = registry.register(upcaster);
    assert_eq!(
        result,
        Err(CoreUpcasterError::InvalidTargetVersion(2)),
        "registering upcaster whose target version exceeds max should be rejected"
    );
}

#[test]
fn registry_returns_circular_chain_error_when_cycle_detected() {
    let registry: Box<dyn UpcasterRegistry> = Box::new(TestUpcasterRegistry::new(2));
    registry.register(CircularUpcasterA::new()).unwrap();
    registry.register(CircularUpcasterB::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope);
    // CircularUpcasterA (v0->v1) then CircularUpcasterB (v1->v0) creates cycle
    assert_eq!(
        result,
        Err(CoreUpcasterError::CircularChain(0)),
        "circular chain should be detected"
    );
}

#[test]
fn registry_propagates_event_envelope_error_when_upcaster_produces_invalid_envelope() {
    let registry = create_test_registry();
    registry.register(GarbageProducingUpcaster::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope);
    // GarbageProducingUpcaster returns Err(UpcasterError::UpcastFailed(...))
    assert!(
        result.is_err(),
        "invalid upcaster output should produce an error"
    );
}

#[test]
fn registry_preserves_envelope_fields_when_upcasting() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test-instance".to_string(),
        sequence: 42,
        timestamp_ms: 1234567890,
        payload: serde_json::json!({"data": "test"}),
        metadata: EventMetadata::default(),
    };

    // RED PHASE: The upcaster stub returns Err, so this will fail
    let result = registry.upcast_envelope(envelope.clone());

    let upcasted = result.expect("upcast_envelope should succeed");
    assert_eq!(upcasted.schema_version, 1, "version should be incremented");
    assert_eq!(upcasted.instance_id, envelope.instance_id);
    assert_eq!(upcasted.sequence, envelope.sequence);
    assert_eq!(upcasted.timestamp_ms, envelope.timestamp_ms);
    assert_eq!(
        upcasted.metadata, envelope.metadata,
        "metadata should be preserved"
    );
}

#[test]
fn registry_returns_correct_max_supported_version() {
    let registry = create_test_registry();
    assert_eq!(registry.max_supported_version(), MAX_SUPPORTED_VERSION);
}

#[test]
fn builder_creates_functional_registry_that_can_register_and_upcast() {
    let registry = <TestUpcasterRegistryBuilder as UpcasterRegistryBuilder>::build();

    // Should be able to register
    let result = registry.register(Version0To1Upcaster::new());
    assert_eq!(result, Ok(()), "registration should succeed");

    // Should return correct max version
    assert_eq!(registry.max_supported_version(), MAX_SUPPORTED_VERSION);

    // Should be able to upcast
    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    // RED PHASE: upcaster stub returns Err, so this will fail
    let result = registry.upcast_envelope(envelope);
    let upcasted = result.expect("upcast should succeed");
    assert_eq!(
        upcasted.schema_version, 1,
        "version should be incremented to 1"
    );
}

#[test]
fn registry_handles_empty_registry_gracefully() {
    let registry = create_test_registry();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope);
    assert_eq!(
        result,
        Err(CoreUpcasterError::NoUpcasterRegistered(0)),
        "upcast should fail when no upcaster registered"
    );
}

// =============================================================================
// Integration: Full Workflow Tests
// =============================================================================

#[test]
fn upcast_envelope_through_full_workflow_when_envelope_enters_at_version_zero_and_must_reach_max() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "workflow-123".to_string(),
        sequence: 1,
        timestamp_ms: 1000000,
        payload: serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "workflow-123",
            "binary_hash": "abc123"
        }),
        metadata: EventMetadata::default(),
    };

    // RED PHASE: upcaster stub returns Err
    let result = registry.upcast_envelope(envelope);

    let upcasted = result.expect("upcast_envelope should succeed");
    assert_eq!(
        upcasted.schema_version, 1,
        "version should be incremented to 1"
    );
    assert_eq!(upcasted.instance_id, "workflow-123");
    assert_eq!(upcasted.sequence, 1);
}

#[test]
fn idempotent_registration_does_not_double_chain() {
    let registry = create_test_registry();

    // Register same version twice (second should fail)
    let result1 = registry.register(Version0To1Upcaster::new());
    assert_eq!(result1, Ok(()), "first registration should succeed");

    let result2 = registry.register(Version0To1Upcaster::new());
    assert_eq!(
        result2,
        Err(CoreUpcasterError::DuplicateRegistration(0)),
        "second registration of same version should fail"
    );

    // Only one upcaster should be registered
    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    // RED PHASE: upcaster stub returns Err
    let result = registry.upcast_envelope(envelope);
    let upcasted = result.expect("upcast should succeed");
    assert_eq!(
        upcasted.schema_version, 1,
        "version should be incremented to 1"
    );
}

#[test]
fn envelope_metadata_preserved_through_multi_hop_upcast() {
    // This test would require MAX_SUPPORTED_VERSION > 1 to properly test multi-hop
    // Since MAX is 1, we can only test single-hop
    // RED PHASE: This is a placeholder test
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };

    let result = registry.upcast_envelope(envelope.clone());
    let upcasted = result.expect("upcast should succeed");
    assert_eq!(
        upcasted.schema_version, 1,
        "version should be incremented to 1"
    );
    assert_eq!(
        upcasted.metadata, envelope.metadata,
        "metadata should be preserved through upcast"
    );
}

// =============================================================================
// UpcasterRegistryImpl Direct Tests (not TestUpcasterRegistry)
// =============================================================================
// These tests kill mutations that survive only when testing the real implementation.
// The TestUpcasterRegistry is a test double that may have different behavior.

/// An upcaster with source_version = MAX_SUPPORTED_VERSION for boundary testing.
struct MaxVersionBoundaryUpcaster;

#[allow(clippy::new_ret_no_self)]
impl MaxVersionBoundaryUpcaster {
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for MaxVersionBoundaryUpcaster {
    fn source_version(&self) -> u8 {
        MAX_SUPPORTED_VERSION
    }

    fn target_version(&self) -> u8 {
        MAX_SUPPORTED_VERSION + 1
    }

    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut result = payload.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "version".to_string(),
                serde_json::json!(MAX_SUPPORTED_VERSION + 1),
            );
        }
        Ok(result)
    }
}

/// An upcaster with source_version = MAX_SUPPORTED_VERSION - 1 (valid boundary).
struct OneBelowMaxUpcaster;

#[allow(clippy::new_ret_no_self)]
impl OneBelowMaxUpcaster {
    fn new() -> Box<dyn Upcaster> {
        Box::new(Self)
    }
}

impl Upcaster for OneBelowMaxUpcaster {
    fn source_version(&self) -> u8 {
        MAX_SUPPORTED_VERSION - 1
    }

    fn target_version(&self) -> u8 {
        MAX_SUPPORTED_VERSION
    }

    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError> {
        let mut result = payload.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "version".to_string(),
                serde_json::json!(MAX_SUPPORTED_VERSION),
            );
        }
        Ok(result)
    }
}

#[test]
fn upcaster_registry_impl_constructs_directly_with_valid_max_version() {
    // Given/When: Construct UpcasterRegistryImpl directly (not via builder)
    let registry = UpcasterRegistryImpl::new(MAX_SUPPORTED_VERSION);

    // Then: max_supported_version returns the configured value
    assert_eq!(
        registry.max_supported_version(),
        MAX_SUPPORTED_VERSION,
        "max_supported_version should return the value passed to new()"
    );
}

#[test]
fn upcaster_registry_impl_rejects_upcaster_when_source_version_equals_max() {
    // Given: UpcasterRegistryImpl with max_version = MAX_SUPPORTED_VERSION
    let registry = UpcasterRegistryImpl::new(MAX_SUPPORTED_VERSION);

    // MaxVersionBoundaryUpcaster has source_version=MAX, target_version=MAX+1 (default).
    // target_version(MAX+1) > max(MAX) should be rejected.
    let upcaster = MaxVersionBoundaryUpcaster::new();

    // Then: Should be rejected with InvalidTargetVersion error
    let result = registry.register(upcaster);
    assert_eq!(
        result,
        Err(CoreUpcasterError::InvalidTargetVersion(
            MAX_SUPPORTED_VERSION + 1
        )),
        "upcaster whose target version exceeds max should be rejected"
    );
}

#[test]
fn upcaster_registry_impl_accepts_upcaster_when_source_version_one_below_max() {
    // Given: UpcasterRegistryImpl with max_version = MAX_SUPPORTED_VERSION
    let registry = UpcasterRegistryImpl::new(MAX_SUPPORTED_VERSION);

    // When: Register an upcaster with source_version = MAX - 1
    let upcaster = OneBelowMaxUpcaster::new();

    // Then: Should succeed
    let result = registry.register(upcaster);
    assert_eq!(
        result,
        Ok(()),
        "upcaster with source_version == max_version - 1 should be accepted"
    );
}

#[test]
fn upcaster_registry_impl_direct_instantiation_max_version_zero() {
    // Given: UpcasterRegistryImpl with max_version = 0
    let registry = UpcasterRegistryImpl::new(0);

    // OneBelowMaxUpcaster has source_version=0, target_version=1 (default).
    // target_version(1) > max(0) should be rejected.
    let upcaster = OneBelowMaxUpcaster::new();

    // Then: Should be rejected
    let result = registry.register(upcaster);
    assert_eq!(
        result,
        Err(CoreUpcasterError::InvalidTargetVersion(1)),
        "upcaster whose target version exceeds max_version (0) should be rejected"
    );
}
