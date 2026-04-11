use super::*;
use crate::events::envelope::EventEnvelope;
use crate::events::error::Error;
use crate::events::metadata::EventMetadata;

#[test]
fn envelope_from_bytes_returns_ok_when_input_is_valid_json() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    let envelope = result.unwrap();
    assert_eq!(envelope.schema_version, 1);
    assert_eq!(envelope.instance_id, "wf-123");
    assert_eq!(envelope.sequence, 1);
    assert_eq!(envelope.timestamp_ms, 1000);
}

#[test]
fn envelope_from_bytes_returns_invalid_envelope_format_when_json_is_malformed() {
    let json = r#"{"version": 1, "instance_id": "wf-123""#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(result, Err(Error::InvalidEnvelopeFormat));
}

#[test]
fn envelope_from_bytes_returns_invalid_input_when_bytes_are_not_valid_utf8() {
    let bytes = vec![0xFF, 0xFE, 0xFD, 0x00];
    let result = EventEnvelope::from_bytes(&bytes);
    assert_eq!(result, Err(Error::InvalidInput));
}

#[test]
fn envelope_from_bytes_returns_missing_envelope_field_when_version_is_absent() {
    let json = r#"{"instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(
        result,
        Err(Error::MissingEnvelopeField("version".to_string()))
    );
}

#[test]
fn envelope_from_bytes_returns_missing_envelope_field_when_instance_id_is_absent() {
    let json =
        r#"{"version": 1, "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(
        result,
        Err(Error::MissingEnvelopeField("instance_id".to_string()))
    );
}

#[test]
fn envelope_from_bytes_returns_missing_envelope_field_when_sequence_is_absent() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(
        result,
        Err(Error::MissingEnvelopeField("sequence".to_string()))
    );
}

#[test]
fn envelope_from_bytes_returns_missing_envelope_field_when_timestamp_ms_is_absent() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(
        result,
        Err(Error::MissingEnvelopeField("timestamp_ms".to_string()))
    );
}

#[test]
fn envelope_from_bytes_returns_missing_envelope_field_when_payload_is_absent() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(
        result,
        Err(Error::MissingEnvelopeField("payload".to_string()))
    );
}

#[test]
fn envelope_from_bytes_returns_ok_with_command_metadata_none_when_metadata_is_absent() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    let envelope = result.expect("absent metadata should succeed per POST-6");
    assert_eq!(envelope.schema_version, 1);
    assert_eq!(envelope.instance_id, "wf-123");
    assert_eq!(envelope.sequence, 1);
    assert_eq!(envelope.timestamp_ms, 123);
    assert!(
        envelope.metadata.command_metadata.is_none(),
        "POST-6: absent metadata → command_metadata: None"
    );
    assert!(envelope.metadata.annotations.is_empty());
}

#[test]
fn envelope_from_bytes_returns_invalid_envelope_field_when_version_is_not_integer() {
    let json = r#"{"version": "1", "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
}

#[test]
fn envelope_from_bytes_returns_invalid_envelope_field_when_instance_id_is_empty() {
    let json = r#"{"version": 1, "instance_id": "", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
}

#[test]
fn envelope_from_bytes_returns_invalid_envelope_field_when_sequence_is_zero() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 0, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
}

#[test]
fn envelope_from_bytes_returns_unsupported_envelope_version_when_version_exceeds_max() {
    let json = r#"{"version": 2, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(result, Err(Error::UnsupportedEnvelopeVersion(2)));
}

#[test]
fn envelope_from_bytes_returns_unsupported_envelope_version_when_version_is_u8_max() {
    let json = r#"{"version": 255, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": {}}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert_eq!(result, Err(Error::UnsupportedEnvelopeVersion(255)));
}

#[test]
fn envelope_from_bytes_returns_invalid_envelope_field_when_metadata_is_not_object() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 123, "payload": {"x":0}, "metadata": []}"#;
    let result = EventEnvelope::from_bytes(json.as_bytes());
    assert!(matches!(result, Err(Error::InvalidEnvelopeField(_))));
}

#[test]
fn envelope_from_str_returns_ok_when_input_is_valid_json() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}, "metadata": {}}"#;
    let result = EventEnvelope::from_str(json);
    result.unwrap();
}

#[test]
fn envelope_from_str_returns_invalid_envelope_format_when_json_is_malformed() {
    let json = r#"{"version": 1, "instance_id": "wf-123""#;
    let result = EventEnvelope::from_str(json);
    assert_eq!(result, Err(Error::InvalidEnvelopeFormat));
}

#[test]
fn envelope_is_supported_returns_true_when_version_is_zero() {
    let envelope = EventEnvelope {
        schema_version: 0,
        instance_id: "wf-123".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };
    assert!(envelope.is_supported());
}

#[test]
fn envelope_is_supported_returns_true_when_version_is_one() {
    let envelope = EventEnvelope {
        schema_version: 1,
        instance_id: "wf-123".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };
    assert!(envelope.is_supported());
}

#[test]
fn envelope_is_supported_returns_false_when_version_is_two() {
    let envelope = EventEnvelope {
        schema_version: 2,
        instance_id: "wf-123".to_string(),
        sequence: 1,
        timestamp_ms: 1000,
        payload: serde_json::json!({}),
        metadata: EventMetadata::default(),
    };
    assert!(!envelope.is_supported());
}
