//! Round-trip tests for FD4 length-prefixed protocol.
//!
//! Tests that JSON written via SDK FD4 (`write_success_inner_with_state`,
//! `write_failure_inner_with_state`) can be read back via the engine's
//! length-prefixed protocol, verifying exact byte match.
//!
//! Covers success and failure envelopes, various payload sizes including empty,
//! and cross-crate framing compatibility with the engine's raw reader.

use std::io::{Cursor, Read};

use proptest::prelude::*;
use serde_json::json;

use crate::tests::write_failure_inner_with_state as write_failure_inner;
use crate::tests::write_success_inner_with_state as write_success_inner;
use crate::SdkError;
use crate::TaskFailureKind;

use crate::io::{FailureEnvelope, SuccessEnvelope};

fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(json!(null)),
        any::<bool>().prop_map_into(),
        any::<i64>().prop_map_into(),
        any::<f64>().prop_map(|f| json!(f)),
        ".*".prop_map(|s| json!(s)),
    ];
    leaf.prop_recursive(3, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(|v| json!(v)),
            prop::collection::hash_map(".*", inner, 0..4).prop_map(|m| json!(m)),
        ]
    })
}

/// Helper: serialize a success envelope to JSON bytes, matching what write_success_inner produces.
fn success_envelope_bytes(output: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&SuccessEnvelope {
        status: "success",
        output,
    })
    .expect("test helper: serialization should not fail")
}

/// Helper: serialize a failure envelope to JSON bytes, matching what write_failure_inner produces.
fn failure_envelope_bytes(kind: &str, message: &str) -> Vec<u8> {
    serde_json::to_vec(&FailureEnvelope {
        status: "failure",
        kind,
        message,
    })
    .expect("test helper: serialization should not fail")
}

/// Read FD4 format: 4-byte BE length prefix + JSON payload.
fn read_fd4_format(bytes: &[u8]) -> Result<(u32, serde_json::Value), SdkError> {
    let mut cursor = Cursor::new(bytes);
    let mut header = [0u8; 4];
    cursor.read_exact(&mut header).map_err(|_| SdkError::InvalidInput)?;
    let len = u32::from_be_bytes(header);
    let mut payload = vec![0u8; len as usize];
    cursor.read_exact(&mut payload).map_err(|_| SdkError::InvalidInput)?;
    let value: serde_json::Value =
        serde_json::from_slice(&payload).map_err(|_| SdkError::InvalidInput)?;
    Ok((len, value))
}

/// Read raw FD4 bytes using the engine protocol: 4-byte BE length prefix + payload bytes.
/// Mirrors `vo_ipc::run::perform_ipc` FD4 read logic.
fn read_fd4_raw(bytes: &[u8]) -> Result<Vec<u8>, SdkError> {
    let mut cursor = Cursor::new(bytes);
    let mut header = [0u8; 4];
    cursor.read_exact(&mut header).map_err(|_| SdkError::InvalidInput)?;
    let len = u32::from_be_bytes(header);
    let mut payload = vec![0u8; len as usize];
    cursor.read_exact(&mut payload).map_err(|_| SdkError::InvalidInput)?;
    let mut trailing = [0u8; 1];
    let trailing_len = cursor.read(&mut trailing).map_err(|_| SdkError::InvalidInput)?;
    if trailing_len != 0 {
        return Err(SdkError::InvalidInput);
    }
    Ok(payload)
}

// ============================================================================
// Success envelope round-trips
// ============================================================================

#[test]
fn fd4_roundtrip_empty_output() {
    let output = json!(null);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(len as usize, buf.len() - 4);
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["output"], json!(null));
}

#[test]
fn fd4_roundtrip_small_object() {
    let output = json!({"key": "value", "num": 42});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(len as usize, buf.len() - 4);
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_roundtrip_nested_object() {
    let output = json!({
        "user": {
            "id": 1,
            "name": "Alice",
            "addresses": [{"city": "NYC"}, {"city": "LA"}]
        },
        "active": true
    });
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
    assert_eq!(parsed["output"]["user"]["name"], "Alice");
    assert_eq!(parsed["output"]["user"]["addresses"][1]["city"], "LA");
}

#[test]
fn fd4_roundtrip_array_output() {
    let output = json!([1, 2, 3, "four", null, {"five": true}]);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_roundtrip_unicode_string() {
    let output = json!("日本語テスト 🔥 éèê");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_roundtrip_large_string() {
    let large_string = "x".repeat(1024 * 100);
    let output = json!(large_string);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_roundtrip_deeply_nested() {
    let output: serde_json::Value =
        serde_json::from_str(r#"{"a": {"b": {"c": {"d": {"e": {"f": "deep"}}}}}"#).unwrap();
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_roundtrip_various_numbers() {
    let output = json!({
        "int": 42,
        "neg": -17,
        "float": 3.14159,
        "sci": 1e10,
        "big_int": 9007199254740992_i64
    });
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_roundtrip_bool_values() {
    let output = json!({"t": true, "f": false});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_length_prefix_correct_for_various_sizes() {
    let cases = [
        (json!(null), "empty_null"),
        (json!("a"), "single_char"),
        (json!("abc"), "short_string"),
        (json!({"x": 1}), "small_object"),
        (json!([1, 2, 3]), "small_array"),
        (json!("x".repeat(255)), "255_bytes"),
        (json!("x".repeat(256)), "256_bytes"),
        (json!("x".repeat(1000)), "1kb"),
        (json!("x".repeat(1024)), "1kb_exact"),
    ];

    for (output, name) in cases {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        write_success_inner(&mut buf, &output, &mut is_written).unwrap();

        let expected_json = success_envelope_bytes(&output);
        let actual_payload_len = buf.len() - 4;
        assert_eq!(
            actual_payload_len, expected_json.len(),
            "length prefix mismatch for {}: expected {} got {}",
            name, expected_json.len(), actual_payload_len
        );

        let len_bytes: [u8; 4] = buf[0..4].try_into().unwrap();
        let len = u32::from_be_bytes(len_bytes);
        assert_eq!(
            len as usize, expected_json.len(),
            "length prefix value mismatch for {}",
            name
        );
    }
}

#[test]
fn fd4_roundtrip_preserves_exact_bytes() {
    let original = json!({
        "data": "Hello, World!",
        "number": 42,
        "nested": {"a": 1, "b": 2}
    });

    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &original, &mut is_written).unwrap();

    let expected = success_envelope_bytes(&original);
    assert_eq!(&buf[4..], expected.as_slice(), "payload bytes should match envelope exactly");
}

// ============================================================================
// Failure envelope round-trips
// ============================================================================

#[test]
fn fd4_failure_roundtrip_user() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_failure_inner(&mut buf, TaskFailureKind::User, "bad input", &mut is_written).unwrap();

    let (len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(len as usize, buf.len() - 4);
    assert_eq!(parsed["status"], "failure");
    assert_eq!(parsed["kind"], "User");
    assert_eq!(parsed["message"], "bad input");
}

#[test]
fn fd4_failure_roundtrip_system() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_failure_inner(&mut buf, TaskFailureKind::System, "disk full", &mut is_written).unwrap();

    let (len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(len as usize, buf.len() - 4);
    assert_eq!(parsed["status"], "failure");
    assert_eq!(parsed["kind"], "System");
    assert_eq!(parsed["message"], "disk full");
}

#[test]
fn fd4_failure_roundtrip_timeout() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_failure_inner(&mut buf, TaskFailureKind::Timeout, "exceeded 30s", &mut is_written).unwrap();

    let (len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(len as usize, buf.len() - 4);
    assert_eq!(parsed["status"], "failure");
    assert_eq!(parsed["kind"], "Timeout");
    assert_eq!(parsed["message"], "exceeded 30s");
}

#[test]
fn fd4_failure_roundtrip_empty_message() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_failure_inner(&mut buf, TaskFailureKind::User, "", &mut is_written).unwrap();

    let (len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(len as usize, buf.len() - 4);
    assert_eq!(parsed["message"], "");
}

#[test]
fn fd4_failure_roundtrip_unicode_message() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_failure_inner(&mut buf, TaskFailureKind::User, "エラー発生 🔥", &mut is_written).unwrap();

    let (_len, parsed) = read_fd4_format(&buf).expect("should read back");
    assert_eq!(parsed["message"], "エラー発生 🔥");
}

#[test]
fn fd4_failure_length_prefix_correct_for_all_kinds() {
    for (kind, kind_str) in [
        (TaskFailureKind::User, "User"),
        (TaskFailureKind::System, "System"),
        (TaskFailureKind::Timeout, "Timeout"),
    ] {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        write_failure_inner(&mut buf, kind, "test message", &mut is_written).unwrap();

        let expected_json = failure_envelope_bytes(kind_str, "test message");
        let actual_payload_len = buf.len() - 4;
        assert_eq!(
            actual_payload_len, expected_json.len(),
            "failure length prefix mismatch for {:?}",
            kind
        );

        let len_bytes: [u8; 4] = buf[0..4].try_into().unwrap();
        let len = u32::from_be_bytes(len_bytes);
        assert_eq!(
            len as usize, expected_json.len(),
            "failure length prefix value mismatch for {:?}",
            kind
        );
    }
}

// ============================================================================
// Cross-crate: SDK writes, engine raw reader reads
// ============================================================================

#[test]
fn fd4_cross_crate_engine_reads_success_bytes() {
    let output = json!({"result": 42, "msg": "hello"});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let raw_payload = read_fd4_raw(&buf).expect("engine should read SDK output");
    let parsed: serde_json::Value =
        serde_json::from_slice(&raw_payload).expect("should be valid JSON");
    assert_eq!(parsed["status"], "success");
    assert_eq!(parsed["output"], output);
}

#[test]
fn fd4_cross_crate_engine_reads_failure_bytes() {
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_failure_inner(&mut buf, TaskFailureKind::System, "oom", &mut is_written).unwrap();

    let raw_payload = read_fd4_raw(&buf).expect("engine should read SDK failure");
    let parsed: serde_json::Value =
        serde_json::from_slice(&raw_payload).expect("should be valid JSON");
    assert_eq!(parsed["status"], "failure");
    assert_eq!(parsed["kind"], "System");
    assert_eq!(parsed["message"], "oom");
}

#[test]
fn fd4_cross_crate_exact_byte_match() {
    let output = json!({"key": "value"});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner(&mut buf, &output, &mut is_written).unwrap();

    let raw_payload = read_fd4_raw(&buf).unwrap();
    let expected = success_envelope_bytes(&output);
    assert_eq!(raw_payload, expected, "exact byte match with expected envelope");
}

#[test]
fn fd4_cross_crate_length_prefix_exact() {
    let cases: Vec<serde_json::Value> = vec![
        json!(null),
        json!(""),
        json!(false),
        json!(0),
        json!("x".repeat(1)),
        json!("x".repeat(255)),
        json!("x".repeat(256)),
        json!("x".repeat(1024)),
        json!("x".repeat(64 * 1024)),
        json!(vec![0; 100].into_iter().map(|_| json!(null)).collect::<Vec<_>>()),
    ];

    for output in &cases {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        write_success_inner(&mut buf, output, &mut is_written).unwrap();

        let raw_payload = read_fd4_raw(&buf).unwrap_or_else(|e| {
            panic!("failed to read back output {:?}: {:?}", output, e)
        });

        let expected = success_envelope_bytes(output);
        assert_eq!(
            raw_payload.len(),
            expected.len(),
            "payload length mismatch for output {:?}",
            output
        );
        assert_eq!(raw_payload, expected, "byte mismatch for output {:?}", output);
    }
}

// ============================================================================
// Proptests
// ============================================================================

proptest! {
    #[test]
    fn fd4_roundtrip_proptest(output in ".*") {
        let json_value: serde_json::Value = serde_json::from_str(&output).unwrap_or_else(|_| json!(output));
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_success_inner(&mut buf, &json_value, &mut is_written);
        prop_assert!(result.is_ok(), "write should succeed");
        prop_assert!(is_written, "guard should be set");

        let (len, parsed) = read_fd4_format(&buf).unwrap();
        prop_assert_eq!(len as usize, buf.len() - 4);
        prop_assert_eq!(&parsed["output"], &json_value);
    }

    #[test]
    fn fd4_roundtrip_json_value_proptest(output in proptest::collection::vec(".*", 0..10)) {
        let json_value = json!(output);
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_success_inner(&mut buf, &json_value, &mut is_written);
        prop_assert!(result.is_ok());

        let (len, parsed) = read_fd4_format(&buf).unwrap();
        prop_assert_eq!(len as usize, buf.len() - 4);
        prop_assert_eq!(&parsed["output"], &json_value);
    }

    #[test]
    fn fd4_failure_roundtrip_proptest(message in ".*") {
        let bytes = message.as_bytes().to_vec();
        if bytes.len() > 1024 {
            return Ok(());
        }

        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_failure_inner(&mut buf, TaskFailureKind::User, &message, &mut is_written);
        prop_assert!(result.is_ok(), "write should succeed");
        prop_assert!(is_written, "guard should be set");

        let (len, parsed) = read_fd4_format(&buf).unwrap();
        prop_assert_eq!(len as usize, buf.len() - 4);
        prop_assert_eq!(&parsed["status"], &json!("failure"));
        prop_assert_eq!(&parsed["kind"], &json!("User"));
        prop_assert_eq!(&parsed["message"], &json!(message));
    }

    #[test]
    fn fd4_cross_crate_exact_bytes_proptest(output in ".*") {
        let json_value: serde_json::Value = serde_json::from_str(&output)
            .unwrap_or_else(|_| json!(output));
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_success_inner(&mut buf, &json_value, &mut is_written);
        prop_assert!(result.is_ok());

        let raw_payload = read_fd4_raw(&buf).unwrap();
        let expected = success_envelope_bytes(&json_value);
        prop_assert_eq!(raw_payload, expected);
    }

    #[test]
    fn fd4_exact_bytes_json_value_proptest(value in json_value_strategy()) {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_success_inner(&mut buf, &value, &mut is_written);
        prop_assert!(result.is_ok(), "write should succeed for any JSON value");
        prop_assert!(is_written, "guard should be set");

        let raw_payload = read_fd4_raw(&buf).unwrap();
        let expected = success_envelope_bytes(&value);
        prop_assert_eq!(raw_payload, expected, "exact byte match via engine protocol");
    }

    #[test]
    fn fd4_failure_exact_bytes_proptest(message in ".*") {
        let bytes = message.as_bytes().to_vec();
        if bytes.len() > 1024 {
            return Ok(());
        }

        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;
        let result = write_failure_inner(&mut buf, TaskFailureKind::User, &message, &mut is_written);
        prop_assert!(result.is_ok(), "write should succeed");
        prop_assert!(is_written, "guard should be set");

        let raw_payload = read_fd4_raw(&buf).unwrap();
        let expected = failure_envelope_bytes("User", &message);
        prop_assert_eq!(raw_payload, expected, "exact byte match for failure envelope");
    }
}
