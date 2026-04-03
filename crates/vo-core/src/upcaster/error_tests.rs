//! Unit tests for UpcasterError enum variants.

use crate::upcaster::UpcasterError;
use vo_types::events::Error as EventEnvelopeError;

#[test]
fn upcaster_error_no_upcaster_registered_message_contains_version() {
    let err = UpcasterError::NoUpcasterRegistered(0);
    let msg = err.to_string();
    assert!(
        msg.contains("0"),
        "Error message should contain version 0: {msg}"
    );
}

#[test]
fn upcaster_error_no_upcaster_registered_equality() {
    assert_eq!(
        UpcasterError::NoUpcasterRegistered(0),
        UpcasterError::NoUpcasterRegistered(0)
    );
    assert_eq!(
        UpcasterError::NoUpcasterRegistered(1),
        UpcasterError::NoUpcasterRegistered(1)
    );
    assert_ne!(
        UpcasterError::NoUpcasterRegistered(0),
        UpcasterError::NoUpcasterRegistered(1)
    );
}

#[test]
fn upcaster_error_upcasting_failed_message_contains_reason() {
    let err = UpcasterError::UpcastingFailed("JSON encode error".to_string());
    let msg = err.to_string();
    assert!(
        msg.contains("JSON encode error"),
        "Error message should contain reason: {msg}"
    );
}

#[test]
fn upcaster_error_upcasting_failed_equality() {
    assert_eq!(
        UpcasterError::UpcastingFailed("error".to_string()),
        UpcasterError::UpcastingFailed("error".to_string())
    );
    assert_ne!(
        UpcasterError::UpcastingFailed("error1".to_string()),
        UpcasterError::UpcastingFailed("error2".to_string())
    );
}

#[test]
fn upcaster_error_invalid_target_version_message_contains_version_and_max() {
    let err = UpcasterError::InvalidTargetVersion(2);
    let msg = err.to_string();
    assert!(
        msg.contains("2"),
        "Error message should contain version 2: {msg}"
    );
    assert!(
        msg.contains("1"),
        "Error message should contain MAX_SUPPORTED_VERSION 1: {msg}"
    );
}

#[test]
fn upcaster_error_invalid_target_version_equality() {
    assert_eq!(
        UpcasterError::InvalidTargetVersion(2),
        UpcasterError::InvalidTargetVersion(2)
    );
    assert_ne!(
        UpcasterError::InvalidTargetVersion(2),
        UpcasterError::InvalidTargetVersion(3)
    );
}

#[test]
fn upcaster_error_circular_chain_message_contains_version() {
    let err = UpcasterError::CircularChain(0);
    let msg = err.to_string();
    assert!(
        msg.contains("0"),
        "Error message should contain version 0: {msg}"
    );
}

#[test]
fn upcaster_error_circular_chain_equality() {
    assert_eq!(
        UpcasterError::CircularChain(0),
        UpcasterError::CircularChain(0)
    );
    assert_ne!(
        UpcasterError::CircularChain(0),
        UpcasterError::CircularChain(1)
    );
}

#[test]
fn upcaster_error_invalid_upcasted_envelope_contains_inner_error() {
    let inner = EventEnvelopeError::InvalidEnvelopeFormat;
    let err = UpcasterError::InvalidUpcastedEnvelope(inner);
    let msg = err.to_string();
    assert!(
        msg.contains("malformed"),
        "Error message should contain 'malformed': {msg}"
    );
}

#[test]
fn upcaster_error_invalid_upcasted_envelope_from_error() {
    let inner = EventEnvelopeError::MissingEnvelopeField("version".to_string());
    let err = UpcasterError::InvalidUpcastedEnvelope(inner);
    assert_eq!(
        err,
        UpcasterError::InvalidUpcastedEnvelope(EventEnvelopeError::MissingEnvelopeField(
            "version".to_string()
        ))
    );
}

#[test]
fn upcaster_error_debug_format() {
    let err = UpcasterError::NoUpcasterRegistered(5);
    let debug = format!("{:?}", err);
    assert!(
        debug.contains("NoUpcasterRegistered"),
        "Debug format should contain variant name: {debug}"
    );
}
