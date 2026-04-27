//! EventMetadata tests for causality propagation.

use crate::events::error::Error;
use crate::events::metadata::EventMetadata;
use crate::{CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

// ============================================================================
// Happy path tests - EventMetadata correctly stores identity values
// ============================================================================

#[test]
fn test_eventmetadata_correctly_stores_identity_values() {
    // Given: CommandMetadata with causality fields (CommandId as causator, CorrelationId)
    let command_metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-001").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-001").unwrap(),
        causation_id: IdempotencyKey::parse("cause-001").unwrap(),
        issuer: Issuer::System,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };

    let metadata = EventMetadata {
        command_metadata: Some(command_metadata),
        annotations: std::collections::HashMap::new(),
    };

    // When: EventMetadata correctly stores identity values
    // Then: Behavior verified - values are accessible
    let cmd = metadata.command_metadata.unwrap();
    assert_eq!(cmd.command_id, IdempotencyKey::parse("cmd-001").unwrap());
    assert_eq!(
        cmd.correlation_id,
        IdempotencyKey::parse("corr-001").unwrap()
    );
    assert_eq!(
        cmd.causation_id,
        IdempotencyKey::parse("cause-001").unwrap()
    );
}

#[test]
fn test_eventmetadata_correctly_stores_identity_values_duplicate_for_schema() {
    // Given: CommandMetadata with causality fields (duplicate for schema validation)
    let command_metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-abc").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-xyz").unwrap(),
        causation_id: IdempotencyKey::parse("cause-123").unwrap(),
        issuer: Issuer::Operator,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };

    let metadata = EventMetadata {
        command_metadata: Some(command_metadata),
        annotations: std::collections::HashMap::new(),
    };

    // When: EventMetadata correctly stores identity values (duplicate for schema)
    // Then: Behavior verified
    let cmd = metadata.command_metadata.unwrap();
    assert_eq!(cmd.command_id, IdempotencyKey::parse("cmd-abc").unwrap());
    assert_eq!(
        cmd.correlation_id,
        IdempotencyKey::parse("corr-xyz").unwrap()
    );
    assert_eq!(
        cmd.causation_id,
        IdempotencyKey::parse("cause-123").unwrap()
    );
    assert_eq!(cmd.issuer, Issuer::Operator);
}

// ============================================================================
// Error path tests - Cannot create EventMetadata without CorrelationId
// ============================================================================

#[test]
fn test_cannot_create_eventmetadata_without_correlationid() {
    // Given: Error precondition - attempting to create without correlation_id
    let json = serde_json::json!({
        "command_metadata": {
            "command_id": "cmd-001",
            "causation_id": "cause-001",
            "issuer": "system",
            "issued_at": 1700000000
            // correlation_id is missing!
        }
    });

    // When: Cannot create EventMetadata without CorrelationId
    // Then: Appropriate error response
    let result = EventMetadata::from_json(&json);
    assert!(
        result.is_err(),
        "Expected error when correlation_id is missing"
    );
}

#[test]
fn test_cannot_create_eventmetadata_without_correlationid_duplicate_for_schema() {
    // Given: Error precondition - attempting to create without correlation_id (duplicate for schema)
    let json = serde_json::json!({
        "command_metadata": {
            "command_id": "cmd-123",
            "causation_id": "cause-123",
            "issuer": "api_client",
            "issued_at": 1700000001
        }
        // correlation_id is missing!
    });

    // When: Cannot create EventMetadata without CorrelationId (duplicate for schema)
    // Then: Appropriate error response
    let result = EventMetadata::from_json(&json);
    assert!(
        result.is_err(),
        "Expected error when correlation_id is missing"
    );
}

// ============================================================================
// Additional validation tests for causality propagation
// ============================================================================

#[test]
fn test_eventmetadata_serializes_with_command_provenance() {
    // Given: EventMetadata with full CommandMetadata
    let command_metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-provenance").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-provenance").unwrap(),
        causation_id: IdempotencyKey::parse("cause-provenance").unwrap(),
        issuer: Issuer::AiAgent,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };

    let metadata = EventMetadata {
        command_metadata: Some(command_metadata),
        annotations: std::collections::HashMap::new(),
    };

    // When: Serialize to JSON
    let json = metadata.to_json();

    // Then: Verify causality fields are present in JSON
    assert_eq!(json["command_metadata"]["command_id"], "cmd-provenance");
    assert_eq!(
        json["command_metadata"]["correlation_id"],
        "corr-provenance"
    );
    assert_eq!(json["command_metadata"]["causation_id"], "cause-provenance");
}

#[test]
fn test_eventmetadata_round_trips_with_annotations() {
    // Given: EventMetadata with annotations
    let mut annotations = std::collections::HashMap::new();
    annotations.insert("trace_id".to_string(), serde_json::json!("trace-123"));
    annotations.insert("span_id".to_string(), serde_json::json!("span-456"));

    let command_metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-annot").unwrap(),
        correlation_id: IdempotencyKey::parse("corr-annot").unwrap(),
        causation_id: IdempotencyKey::parse("cause-annot").unwrap(),
        issuer: Issuer::TimerLoop,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };

    let metadata = EventMetadata {
        command_metadata: Some(command_metadata),
        annotations,
    };

    // When: Round-trip through JSON
    let json = metadata.to_json();
    let round_tripped = EventMetadata::from_json(&json).unwrap();

    // Then: Verify all fields match
    let rt_cmd = round_tripped.command_metadata.unwrap();
    assert_eq!(
        rt_cmd.command_id,
        IdempotencyKey::parse("cmd-annot").unwrap()
    );
    assert_eq!(
        rt_cmd.correlation_id,
        IdempotencyKey::parse("corr-annot").unwrap()
    );
    assert_eq!(
        round_tripped.annotations.get("trace_id"),
        Some(&serde_json::json!("trace-123"))
    );
    assert_eq!(
        round_tripped.annotations.get("span_id"),
        Some(&serde_json::json!("span-456"))
    );
}

#[test]
fn test_eventmetadata_default_is_empty() {
    // Given: Default EventMetadata
    let metadata = EventMetadata::default();

    // When: Check default state
    // Then: command_metadata is None, annotations is empty
    assert!(metadata.command_metadata.is_none());
    assert!(metadata.annotations.is_empty());
}

#[test]
fn test_correlation_id_matches_originating_command() {
    // Given: An originating command with a specific correlation_id
    let originating_correlation_id = IdempotencyKey::parse("corr-origin-123").unwrap();

    let command_metadata = CommandMetadata {
        command_id: IdempotencyKey::parse("cmd-child").unwrap(),
        correlation_id: originating_correlation_id.clone(),
        causation_id: IdempotencyKey::parse("cmd-parent").unwrap(),
        issuer: Issuer::ApiClient,
        issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
    };

    let metadata = EventMetadata {
        command_metadata: Some(command_metadata),
        annotations: std::collections::HashMap::new(),
    };

    // When: CorrelationId matches the originating command's CorrelationId
    // Then: Invariant maintained - correlation_id is preserved
    assert_eq!(
        metadata.command_metadata.unwrap().correlation_id,
        originating_correlation_id
    );
}
