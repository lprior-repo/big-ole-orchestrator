use crate::events::envelope::EventEnvelope;
use crate::events::error::Error;
use crate::events::metadata::EventMetadata;
use rstest::rstest;

#[rstest]
#[case(0, 1, "wf-1", 0)]
#[case(1, 100, "wf-abc", 1000)]
fn envelope_roundtrip_preserves_data(
    #[case] version: u8,
    #[case] seq: u64,
    #[case] instance_id: &str,
    #[case] ts: u64,
) {
    let json = serde_json::json!({
        "version": version,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": ts,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);

    let expected = EventEnvelope {
        schema_version: version,
        instance_id: instance_id.to_string(),
        sequence: seq,
        timestamp_ms: ts,
        payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1}),
        metadata: EventMetadata::default(),
    };
    assert_eq!(result, Ok(expected));
}

#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(3)]
#[case(4)]
#[case(5)]
fn proptest_version_support_is_consistent_across_envelope_and_payload(#[case] version: u8) {
    let envelope = EventEnvelope {
        schema_version: version,
        instance_id: "wf-123".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };
    let envelope_supported = envelope.is_supported();
    let payload_supported = crate::events::EventPayload::is_version_supported(version);
    assert_eq!(
        envelope_supported, payload_supported,
        "Inconsistent for version {}",
        version
    );
}

#[rstest]
#[case(1, "wf-1")]
#[case(100, "wf-100")]
#[case(999_999_999, "wf-max")]
fn envelope_parsing_accepts_positive_sequence(#[case] seq: u64, #[case] instance_id: &str) {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);
    let Ok(envelope) = result else {
        panic!("Expected Ok, got {:?}", result);
    };
    assert_eq!(envelope.sequence, seq);
}

#[rstest]
#[case(0, "wf-zero")]
fn envelope_parsing_rejects_zero_sequence(#[case] seq: u64, #[case] instance_id: &str) {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);
    let Err(err) = result else {
        panic!("Expected Err, got {:?}", result);
    };
    assert!(matches!(err, Error::InvalidEnvelopeField(_)));
}

#[rstest]
#[case("a")]
#[case("wf-123")]
#[case("instance_with_underscores")]
fn envelope_parsing_accepts_nonempty_instance_id(#[case] instance_id: &str) {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);
    let Ok(envelope) = result else {
        panic!("Expected Ok, got {:?}", result);
    };
    assert_eq!(envelope.instance_id, instance_id);
}

#[rstest]
#[case("")]
fn envelope_parsing_rejects_empty_instance_id(#[case] instance_id: &str) {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);
    let Err(err) = result else {
        panic!("Expected Err, got {:?}", result);
    };
    assert!(matches!(err, Error::InvalidEnvelopeField(_)));
}

#[rstest]
#[case(serde_json::json!({}),)]
#[case(serde_json::json!({"key": "value"}),)]
#[case(serde_json::json!({"key1": "value1", "key2": 123}),)]
fn envelope_parsing_accepts_object_metadata(#[case] annotations_json: serde_json::Value) {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": "wf-123",
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": {
            "command_metadata": null,
            "annotations": annotations_json
        }
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);
    let Ok(envelope) = result else {
        panic!("Expected Ok, got {:?}", result);
    };
    assert!(envelope.metadata.command_metadata.is_none());
    let expected_annotations: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_value(annotations_json).unwrap_or_default();
    assert_eq!(envelope.metadata.annotations, expected_annotations);
}

#[rstest]
#[case(serde_json::json!([]))]
#[case(serde_json::json!("string"))]
#[case(serde_json::Value::Null)]
#[case(serde_json::json!(123))]
fn envelope_parsing_rejects_non_object_metadata(#[case] metadata: serde_json::Value) {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": "wf-123",
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "version": 1},
        "metadata": metadata
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = EventEnvelope::from_bytes(&bytes);
    let Err(err) = result else {
        panic!("Expected Err, got {:?}", result);
    };
    assert!(matches!(err, Error::InvalidEnvelopeField(_)));
}

#[rstest]
#[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": "bad", "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("sequence must be an integer".to_string()))]
#[case(r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": "bad", "payload": {"type": "WorkflowStarted", "workflow_id": "w1"}, "metadata": {}}"#, Error::InvalidEnvelopeField("timestamp_ms must be an integer".to_string()))]
fn envelope_from_str_invalid_types(#[case] json: &str, #[case] expected: Error) {
    let result = EventEnvelope::from_str(json);
    assert_eq!(result, Err(expected));
}
