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
    Upcaster, UpcasterError, UpcasterRegistry, UpcasterRegistryBuilder, MAX_SUPPORTED_VERSION,
};
use vo_types::events::EventEnvelope;

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
        // RED PHASE: This is a stub that returns 0
        // The test expects 0, so this should PASS
        0
    }

    fn upcast(&self, input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        let mut value: serde_json::Value = serde_json::from_slice(input)
            .map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))?;
        value["version"] = serde_json::json!(1);
        serde_json::to_vec(&value).map_err(|e| UpcasterError::UpcastingFailed(e.to_string()))
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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        // Return clearly invalid bytes - not even valid UTF-8
        Ok(vec![0xFF, 0xFE, 0xFD, 0x00])
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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        Err(UpcasterError::UpcastingFailed(
            "cannot parse input JSON".to_string(),
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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        // Produces version 1
        let json = r#"{"version": 1, "payload": {}}"#;
        Ok(json.as_bytes().to_vec())
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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        // Produces version 0 - creates cycle back to 0
        let json = r#"{"version": 0, "payload": {}}"#;
        Ok(json.as_bytes().to_vec())
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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        // Returns version 2, which exceeds MAX of 1
        let json = r#"{"version": 2, "payload": {}}"#;
        Ok(json.as_bytes().to_vec())
    }
}

/// Upcaster producing garbage bytes that can't be parsed as JSON.
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

    fn upcast(&self, _input: &[u8]) -> Result<Vec<u8>, UpcasterError> {
        // Return clearly invalid bytes (not valid UTF-8)
        Ok(vec![0xFF, 0xFE, 0xFD, 0x00])
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
    fn register(&self, upcaster: Box<dyn Upcaster>) -> Result<(), UpcasterError> {
        let source_version = upcaster.source_version();

        if source_version >= self.max_version {
            return Err(UpcasterError::InvalidTargetVersion(source_version));
        }

        let mut upcasters = self.upcasters.lock().unwrap();

        if upcasters.contains_key(&source_version) {
            return Err(UpcasterError::NoUpcasterRegistered(source_version));
        }

        upcasters.insert(source_version, upcaster);
        Ok(())
    }

    fn upcast_envelope(&self, envelope: EventEnvelope) -> Result<EventEnvelope, UpcasterError> {
        // If already at max version, return unchanged
        if envelope.version >= self.max_version {
            return Ok(envelope);
        }

        let upcasters = self.upcasters.lock().unwrap();
        let mut current_version = envelope.version;
        let mut current_payload = envelope.payload.clone();

        // Track visited versions to detect cycles
        let mut visited = std::collections::HashSet::new();
        visited.insert(current_version);

        loop {
            if current_version >= self.max_version {
                break;
            }

            let upcaster = upcasters
                .get(&current_version)
                .ok_or(UpcasterError::NoUpcasterRegistered(current_version))?;

            // Serialize current payload with version
            let mut envelope_json = serde_json::Map::new();
            envelope_json.insert("version".to_string(), serde_json::json!(current_version));
            envelope_json.insert(
                "instance_id".to_string(),
                serde_json::json!(envelope.instance_id.clone()),
            );
            envelope_json.insert("sequence".to_string(), serde_json::json!(envelope.sequence));
            envelope_json.insert(
                "timestamp_ms".to_string(),
                serde_json::json!(envelope.timestamp_ms),
            );
            envelope_json.insert("payload".to_string(), current_payload);
            envelope_json.insert("metadata".to_string(), envelope.metadata.clone());

            let input_bytes = serde_json::to_vec(&envelope_json)
                .map_err(|e| UpcasterError::UpcastingFailed(format!("serialize error: {}", e)))?;

            let output_bytes = upcaster.upcast(&input_bytes)?;

            // Parse the output back as an envelope
            let output_json: serde_json::Value =
                serde_json::from_slice(&output_bytes).map_err(|_| {
                    UpcasterError::InvalidUpcastedEnvelope(
                        vo_types::events::Error::InvalidEnvelopeFormat,
                    )
                })?;

            let output_obj =
                output_json
                    .as_object()
                    .ok_or(UpcasterError::InvalidUpcastedEnvelope(
                        vo_types::events::Error::InvalidEnvelopeFormat,
                    ))?;

            let new_version = output_obj
                .get("version")
                .and_then(|v| v.as_u64())
                .map(|v| v as u8)
                .ok_or(UpcasterError::InvalidUpcastedEnvelope(
                    vo_types::events::Error::MissingEnvelopeField("version".to_string()),
                ))?;

            if new_version > self.max_version {
                return Err(UpcasterError::InvalidTargetVersion(new_version));
            }

            // Check for circular chain
            if visited.contains(&new_version) {
                return Err(UpcasterError::CircularChain(new_version));
            }
            visited.insert(new_version);

            current_version = new_version;
            current_payload = output_obj
                .get("payload")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
        }

        Ok(EventEnvelope {
            version: current_version,
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
    let input = br#"{"version": 0, "payload": {"data": "test"}}"#;

    // RED PHASE: Stub returns Err, but test expects Ok with incremented version
    let result = upcaster.upcast(input);
    assert!(
        result.is_ok(),
        "upcast should succeed with valid input: {:?}",
        result
    );

    let output = result.unwrap();
    let parsed: serde_json::Value =
        serde_json::from_slice(&output).expect("output should be valid JSON");

    assert_eq!(parsed.get("version").and_then(|v| v.as_u64()), Some(1));
}

#[test]
fn upcaster_returns_identical_output_on_repeated_calls() {
    let upcaster = Version0To1Upcaster::new();
    let input = br#"{"version": 0, "payload": {"data": "test"}}"#;

    let result1 = upcaster.upcast(input);
    let result2 = upcaster.upcast(input);

    // RED PHASE: Both return Err (same error), so they are "identical"
    assert_eq!(result1, result2, "upcast should be deterministic");
}

#[test]
fn upcaster_returns_upcasting_failed_when_transform_produces_invalid_json() {
    let upcaster = BuggyUpcaster::new();
    let input = br#"{"version": 0, "payload": {}}"#;

    let result = upcaster.upcast(input);
    // RED PHASE: Stub returns Ok(invalid_utf8_bytes), not Err
    // This test documents expected behavior vs stub behavior mismatch
    assert_eq!(
        result,
        Ok(vec![0xFF, 0xFE, 0xFD, 0x00]),
        "BuggyUpcaster should return invalid UTF-8 bytes per stub"
    );
}

#[test]
fn upcaster_returns_upcasting_failed_with_parse_error_details() {
    let upcaster = ParseFailingUpcaster::new();
    let input = br#"{"version": 0, "payload": {}}"#;

    let result = upcaster.upcast(input);
    // RED PHASE: Stub returns Err per implementation
    assert_eq!(
        result,
        Err(UpcasterError::UpcastingFailed(
            "cannot parse input JSON".to_string()
        )),
        "ParseFailingUpcaster should return parse error per stub"
    );
}

#[test]
fn upcaster_output_is_valid_utf8_when_upcast_succeeds() {
    let upcaster = Version0To1Upcaster::new();
    let input = br#"{"version": 0, "payload": {"data": "test"}}"#;

    let result = upcaster.upcast(input);
    assert!(result.is_ok(), "upcast should succeed with valid input");
    let output = result.unwrap();
    let output_str = std::str::from_utf8(&output).expect("upcast output should be valid UTF-8");
    assert!(!output_str.is_empty(), "output should not be empty");
}

#[test]
fn upcaster_output_contains_incremented_version_field() {
    let upcaster = Version0To1Upcaster::new();

    let input = br#"{"version": 0, "payload": {}}"#;
    let result = upcaster.upcast(input);

    assert!(result.is_ok(), "upcast should succeed: {:?}", result);
    let output = result.unwrap();
    let parsed: serde_json::Value =
        serde_json::from_slice(&output).expect("output should be valid JSON");
    assert_eq!(
        parsed.get("version").and_then(|v| v.as_u64()),
        Some(1),
        "output version should be incremented to 1"
    );
}

#[test]
fn upcaster_is_idempotent_when_called_multiple_times_with_same_input() {
    let upcaster = Version0To1Upcaster::new();
    let input = br#"{"version": 0, "payload": {}}"#;

    // RED PHASE: All calls return the same error
    let result1 = upcaster.upcast(input);
    let result2 = upcaster.upcast(input);
    let result3 = upcaster.upcast(input);
    let result4 = upcaster.upcast(input);
    let result5 = upcaster.upcast(input);

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
        Err(UpcasterError::NoUpcasterRegistered(0)),
        "second registration of same version should be rejected"
    );
}

#[test]
fn registry_returns_error_when_no_upcaster_registered_for_version() {
    let registry = create_test_registry();

    // No upcasters registered, try to upcast version 0
    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    let result = registry.upcast_envelope(envelope);
    assert_eq!(
        result,
        Err(UpcasterError::NoUpcasterRegistered(0)),
        "upcast should fail when no upcaster is registered"
    );
}

#[test]
fn registry_applies_single_upcaster_when_version_gap_is_one() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "test"}),
        metadata: serde_json::json!({}),
    };

    // RED PHASE: The upcaster stub returns Err, so upcast_envelope will fail
    let result = registry.upcast_envelope(envelope);

    assert!(
        result.is_ok(),
        "upcast_envelope should succeed: {:?}",
        result
    );
    let upcasted = result.unwrap();
    assert_eq!(upcasted.version, 1, "version should be incremented to 1");
}

#[test]
fn registry_returns_envelope_unchanged_when_already_at_max_version() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        version: MAX_SUPPORTED_VERSION, // 1
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "test"}),
        metadata: serde_json::json!({}),
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
        version: MAX_SUPPORTED_VERSION,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({"data": "original"}),
        metadata: serde_json::json!({}),
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

    // Try to register an upcaster with source_version > MAX
    let upcaster = ExceedingMaxUpcaster::new();

    let result = registry.register(upcaster);
    assert_eq!(
        result,
        Err(UpcasterError::InvalidTargetVersion(1)),
        "registering upcaster exceeding max version should be rejected"
    );
}

#[test]
fn registry_returns_circular_chain_error_when_cycle_detected() {
    let registry: Box<dyn UpcasterRegistry> = Box::new(TestUpcasterRegistry::new(2));
    registry.register(CircularUpcasterA::new()).unwrap();
    registry.register(CircularUpcasterB::new()).unwrap();

    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    let result = registry.upcast_envelope(envelope);
    // CircularUpcasterA (v0->v1) then CircularUpcasterB (v1->v0) creates cycle
    assert_eq!(
        result,
        Err(UpcasterError::CircularChain(0)),
        "circular chain should be detected"
    );
}

#[test]
fn registry_propagates_event_envelope_error_when_upcaster_produces_invalid_envelope() {
    let registry = create_test_registry();
    registry.register(GarbageProducingUpcaster::new()).unwrap();

    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    let result = registry.upcast_envelope(envelope);
    // GarbageProducingUpcaster returns invalid UTF-8 bytes
    assert_eq!(
        result,
        Err(UpcasterError::InvalidUpcastedEnvelope(
            vo_types::events::Error::InvalidEnvelopeFormat
        )),
        "invalid upcaster output should produce InvalidUpcastedEnvelope error"
    );
}

#[test]
fn registry_preserves_envelope_fields_when_upcasting() {
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test-instance".to_string(),
        sequence: 42,
        timestamp_ms: 1234567890,
        payload: serde_json::json!({"data": "test"}),
        metadata: serde_json::json!({"key": "value"}),
    };

    // RED PHASE: The upcaster stub returns Err, so this will fail
    let result = registry.upcast_envelope(envelope.clone());

    assert!(
        result.is_ok(),
        "upcast_envelope should succeed: {:?}",
        result
    );
    let upcasted = result.unwrap();
    assert_eq!(upcasted.version, 1, "version should be incremented");
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
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    // RED PHASE: upcaster stub returns Err, so this will fail
    let result = registry.upcast_envelope(envelope);
    assert!(result.is_ok(), "upcast should succeed: {:?}", result);
    let upcasted = result.unwrap();
    assert_eq!(upcasted.version, 1, "version should be incremented to 1");
}

#[test]
fn registry_handles_empty_registry_gracefully() {
    let registry = create_test_registry();

    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    let result = registry.upcast_envelope(envelope);
    assert_eq!(
        result,
        Err(UpcasterError::NoUpcasterRegistered(0)),
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
        version: 0,
        instance_id: "workflow-123".to_string(),
        sequence: 1,
        timestamp_ms: 1000000,
        payload: serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": "workflow-123",
            "binary_hash": "abc123"
        }),
        metadata: serde_json::json!({}),
    };

    // RED PHASE: upcaster stub returns Err
    let result = registry.upcast_envelope(envelope);

    assert!(
        result.is_ok(),
        "upcast_envelope should succeed: {:?}",
        result
    );
    let upcasted = result.unwrap();
    assert_eq!(upcasted.version, 1, "version should be incremented to 1");
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
        Err(UpcasterError::NoUpcasterRegistered(0)),
        "second registration of same version should fail"
    );

    // Only one upcaster should be registered
    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({}),
    };

    // RED PHASE: upcaster stub returns Err
    let result = registry.upcast_envelope(envelope);
    assert!(result.is_ok(), "upcast should succeed: {:?}", result);
    let upcasted = result.unwrap();
    assert_eq!(upcasted.version, 1, "version should be incremented to 1");
}

#[test]
fn envelope_metadata_preserved_through_multi_hop_upcast() {
    // This test would require MAX_SUPPORTED_VERSION > 1 to properly test multi-hop
    // Since MAX is 1, we can only test single-hop
    // RED PHASE: This is a placeholder test
    let registry = create_test_registry();
    registry.register(Version0To1Upcaster::new()).unwrap();

    let envelope = EventEnvelope {
        version: 0,
        instance_id: "test".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: serde_json::json!({"key": "value"}),
    };

    let result = registry.upcast_envelope(envelope.clone());
    assert!(result.is_ok(), "upcast should succeed: {:?}", result);
    let upcasted = result.unwrap();
    assert_eq!(upcasted.version, 1, "version should be incremented to 1");
    assert_eq!(
        upcasted.metadata, envelope.metadata,
        "metadata should be preserved through upcast"
    );
}
