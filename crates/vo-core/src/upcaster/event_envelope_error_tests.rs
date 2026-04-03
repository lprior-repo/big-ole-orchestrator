//! Unit tests for EventEnvelopeError (vo-types Error enum).
//!
//! These tests verify all error variants have correct messages and behavior.

use vo_types::events::Error as EventEnvelopeError;

#[test]
fn event_envelope_error_invalid_input_message_contains_not_utf8() {
    let err = EventEnvelopeError::InvalidInput;
    let msg = err.to_string();
    assert!(
        msg.contains("not valid UTF-8") || msg.contains("UTF-8"),
        "Error message should mention UTF-8: {msg}"
    );
}

#[test]
fn event_envelope_error_invalid_envelope_format_message_contains_malformed() {
    let err = EventEnvelopeError::InvalidEnvelopeFormat;
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("malformed"),
        "Error message should contain 'malformed': {msg}"
    );
}

#[test]
fn event_envelope_error_missing_envelope_field_message_contains_field_name() {
    let err = EventEnvelopeError::MissingEnvelopeField("version".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("version"),
        "Error message should contain field name 'version': {msg}"
    );
}

#[test]
fn event_envelope_error_invalid_envelope_field_message_contains_field_name() {
    let err = EventEnvelopeError::InvalidEnvelopeField("version must be an integer".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("version"),
        "Error message should contain field name: {msg}"
    );
}

#[test]
fn event_envelope_error_unsupported_envelope_version_message_contains_version() {
    let err = EventEnvelopeError::UnsupportedEnvelopeVersion(5);
    let msg = err.to_string();
    assert!(
        msg.contains("5"),
        "Error message should contain version 5: {msg}"
    );
}

#[test]
fn event_envelope_error_unsupported_envelope_version_message_contains_max_info() {
    let err = EventEnvelopeError::UnsupportedEnvelopeVersion(255);
    let msg = err.to_string();
    // Message should indicate something about supported version
    assert!(
        msg.contains("255") || msg.contains("supported") || msg.contains("version"),
        "Error message should contain version or supported info: {msg}"
    );
}

#[test]
fn event_envelope_error_invalid_payload_format_message() {
    let err = EventEnvelopeError::InvalidPayloadFormat;
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("malformed") || msg.to_lowercase().contains("payload"),
        "Error message should mention malformed or payload: {msg}"
    );
}

#[test]
fn event_envelope_error_missing_payload_field_message() {
    let err = EventEnvelopeError::MissingPayloadField("workflow_id".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("workflow_id"),
        "Error message should contain field name: {msg}"
    );
}

#[test]
fn event_envelope_error_invalid_payload_field_message() {
    let err = EventEnvelopeError::InvalidPayloadField("attempt must be an integer".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("attempt"),
        "Error message should contain field name: {msg}"
    );
}

#[test]
fn event_envelope_error_unsupported_payload_version_message() {
    let err = EventEnvelopeError::UnsupportedPayloadVersion(2);
    let msg = err.to_string();
    assert!(
        msg.contains("2"),
        "Error message should contain version: {msg}"
    );
}

#[test]
fn event_envelope_error_unknown_payload_type_message() {
    let err = EventEnvelopeError::UnknownPayloadType("CustomEvent".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("CustomEvent"),
        "Error message should contain type name: {msg}"
    );
}

#[test]
fn event_envelope_error_envelope_decode_failed_message() {
    let inner = EventEnvelopeError::InvalidEnvelopeFormat;
    let err = EventEnvelopeError::EnvelopeDecodeFailed(Box::new(inner));
    let msg = err.to_string();
    // Should contain info about the inner error
    assert!(
        msg.contains("decode") || msg.contains("failed") || msg.contains("envelope"),
        "Error message should mention decode or envelope: {msg}"
    );
}

#[test]
fn event_envelope_error_payload_decode_skipped_message() {
    let err = EventEnvelopeError::PayloadDecodeSkipped;
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("skipped") || msg.to_lowercase().contains("unsupported"),
        "Error message should mention skipped or unsupported: {msg}"
    );
}

#[test]
fn event_envelope_error_payload_decode_failed_message() {
    let inner = EventEnvelopeError::InvalidPayloadFormat;
    let err = EventEnvelopeError::PayloadDecodeFailed(Box::new(inner));
    let msg = err.to_string();
    assert!(
        msg.contains("decode") || msg.contains("failed") || msg.contains("payload"),
        "Error message should mention decode, failed, or payload: {msg}"
    );
}

#[test]
fn event_envelope_error_serialization_error_message() {
    let err = EventEnvelopeError::SerializationError("custom error".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("custom error"),
        "Error message should contain custom error: {msg}"
    );
}

// Equality tests

#[test]
fn event_envelope_error_invalid_input_equality() {
    assert_eq!(
        EventEnvelopeError::InvalidInput,
        EventEnvelopeError::InvalidInput
    );
}

#[test]
fn event_envelope_error_invalid_envelope_format_equality() {
    assert_eq!(
        EventEnvelopeError::InvalidEnvelopeFormat,
        EventEnvelopeError::InvalidEnvelopeFormat
    );
}

#[test]
fn event_envelope_error_missing_envelope_field_equality() {
    assert_eq!(
        EventEnvelopeError::MissingEnvelopeField("a".to_string()),
        EventEnvelopeError::MissingEnvelopeField("a".to_string())
    );
    assert_ne!(
        EventEnvelopeError::MissingEnvelopeField("a".to_string()),
        EventEnvelopeError::MissingEnvelopeField("b".to_string())
    );
}

#[test]
fn event_envelope_error_invalid_envelope_field_equality() {
    assert_eq!(
        EventEnvelopeError::InvalidEnvelopeField("x".to_string()),
        EventEnvelopeError::InvalidEnvelopeField("x".to_string())
    );
    assert_ne!(
        EventEnvelopeError::InvalidEnvelopeField("x".to_string()),
        EventEnvelopeError::InvalidEnvelopeField("y".to_string())
    );
}

#[test]
fn event_envelope_error_unsupported_envelope_version_equality() {
    assert_eq!(
        EventEnvelopeError::UnsupportedEnvelopeVersion(1),
        EventEnvelopeError::UnsupportedEnvelopeVersion(1)
    );
    assert_ne!(
        EventEnvelopeError::UnsupportedEnvelopeVersion(1),
        EventEnvelopeError::UnsupportedEnvelopeVersion(2)
    );
}

#[test]
fn event_envelope_error_unknown_payload_type_equality() {
    assert_eq!(
        EventEnvelopeError::UnknownPayloadType("A".to_string()),
        EventEnvelopeError::UnknownPayloadType("A".to_string())
    );
    assert_ne!(
        EventEnvelopeError::UnknownPayloadType("A".to_string()),
        EventEnvelopeError::UnknownPayloadType("B".to_string())
    );
}
