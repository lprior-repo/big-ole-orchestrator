use super::*;
use crate::events::error::Error;

#[test]
fn error_serialization_error_displays_correctly() {
    let err = Error::SerializationError("test_error".to_string());
    assert_eq!(err.to_string(), "Serialization error: test_error");
}

#[test]
fn error_envelope_decode_failed_displays_correctly() {
    let inner = Error::InvalidInput;
    let err = Error::EnvelopeDecodeFailed(Box::new(inner));
    assert_eq!(
        err.to_string(),
        "Envelope decode failed: Input bytes are not valid UTF-8"
    );
}

#[test]
fn error_payload_decode_failed_displays_correctly() {
    let inner = Error::InvalidPayloadFormat;
    let err = Error::PayloadDecodeFailed(Box::new(inner));
    assert_eq!(
        err.to_string(),
        "Payload decode failed: Payload JSON is malformed"
    );
}

#[test]
fn error_payload_decode_skipped_displays_correctly() {
    let err = Error::PayloadDecodeSkipped;
    assert_eq!(
        err.to_string(),
        "Payload decode skipped due to unsupported envelope version"
    );
}

#[test]
fn error_invalid_schema_version_format_displays_correctly() {
    let err = Error::InvalidSchemaVersionFormat;
    assert_eq!(err.to_string(), "Invalid schema version format");
}

#[test]
fn error_unsupported_schema_version_displays_correctly() {
    let err = Error::UnsupportedSchemaVersion(99);
    assert_eq!(err.to_string(), "Unsupported schema version: 99");
}

#[test]
fn error_missing_schema_version_displays_correctly() {
    let err = Error::MissingSchemaVersion;
    assert_eq!(err.to_string(), "Missing schema version");
}
