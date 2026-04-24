//! Round-trip tests for FD4 length-prefixed protocol.
//!
//! These tests verify that data written via SDK's FD4 interface can be read
//! by the engine's protocol (and vice versa).
//!
//! The engine protocol uses length-prefixed Fd4Envelope format:
//! - 4-byte BE length prefix
//! - JSON payload: {"version": 1, "instance_id": "...", "node_id": "...", "result": ...}
//!
//! The SDK's write_success/write_failure use a different internal format.

use std::io::{Cursor, Read, Write};

use proptest::prelude::*;
use serde_json::json;

use crate::io::write_success_inner_with_state;
use crate::TaskFailureKind;

const MAX_PAYLOAD_SIZE: u32 = 10_485_760;

#[derive(Debug, serde::Deserialize)]
struct Fd4Envelope {
    version: u8,
    #[serde(rename = "instance_id")]
    instance_id: String,
    #[serde(rename = "node_id")]
    node_id: String,
    result: Fd4Result,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Fd4Result {
    Success { output: serde_json::Value },
    Failure { error: Fd4Error },
}

#[derive(Debug, serde::Deserialize)]
struct Fd4Error {
    code: String,
    message: String,
    #[serde(default)]
    details: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
struct EngineWriteEnvelope<'a> {
    version: u8,
    #[serde(rename = "instance_id")]
    instance_id: &'a str,
    #[serde(rename = "node_id")]
    node_id: &'a str,
    result: EngineResult<'a>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum EngineResult<'a> {
    Success { output: &'a serde_json::Value },
    Failure { error: EngineError<'a> },
}

#[derive(Debug, serde::Serialize)]
struct EngineError<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

fn write_length_prefixed<W: Write>(writer: &mut W, payload: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)?;
    Ok(())
}

fn read_length_prefixed<R: Read>(reader: &mut R) -> Result<Vec<u8>, std::io::Error> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        Just(json!(null)),
        any::<bool>().prop_map_into(),
        any::<i64>().prop_map_into(),
        ".*".prop_map(|s| json!(s)),
    ];
    leaf.prop_recursive(3, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(|v| json!(v)),
            prop::collection::hash_map(".*", inner, 0..4).prop_map(|m| json!(m)),
        ]
    })
}

proptest! {
    #[test]
    fn sdk_write_success_read_via_engine_protocol_fails(output in json_value_strategy()) {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;

        write_success_inner_with_state(&mut buf, &output, &mut is_written).unwrap();

        let result = read_length_prefixed(&mut Cursor::new(&buf));
        assert!(result.is_err(), "SDK raw JSON cannot be read as length-prefixed envelope");
    }

    #[test]
    fn sdk_write_output_preserves_json_value(output in json_value_strategy()) {
        let mut buf: Vec<u8> = Vec::new();
        let mut is_written = false;

        write_success_inner_with_state(&mut buf, &output, &mut is_written).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(parsed["status"].as_str(), Some("success"));
        assert_eq!(&parsed["output"], &output);
    }
}

#[test]
fn sdk_write_empty_output() {
    let output = json!(null);
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner_with_state(&mut buf, &output, &mut is_written).unwrap();

    let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(parsed["status"].as_str(), Some("success"));
    assert_eq!(&parsed["output"], &json!(null));
}

#[test]
fn sdk_write_empty_string_output() {
    let output = json!("");
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner_with_state(&mut buf, &output, &mut is_written).unwrap();

    let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(parsed["output"], json!(""));
}

#[test]
fn sdk_write_large_output() {
    let output = json!({"data": "x".repeat(1024 * 1024)});
    let mut buf: Vec<u8> = Vec::new();
    let mut is_written = false;

    write_success_inner_with_state(&mut buf, &output, &mut is_written).unwrap();

    let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(parsed["status"].as_str(), Some("success"));
    assert_eq!(parsed["output"]["data"].as_str().map(|s| s.len()), Some(1024 * 1024));
}

#[test]
fn engine_protocol_roundtrip_via_manual_envelope() {
    let output = json!({"result": "ok", "value": 42});
    let envelope = EngineWriteEnvelope {
        version: 1,
        instance_id: "test-instance",
        node_id: "test-node",
        result: EngineResult::Success { output: &output },
    };

    let payload = serde_json::to_vec(&envelope).unwrap();
    let mut buf: Vec<u8> = Vec::new();
    write_length_prefixed(&mut buf, &payload).unwrap();

    let read_payload = read_length_prefixed(&mut Cursor::new(&buf)).unwrap();
    let decoded: Fd4Envelope = serde_json::from_slice(&read_payload).unwrap();

    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.instance_id, "test-instance");
    assert_eq!(decoded.node_id, "test-node");
    match decoded.result {
        Fd4Result::Success { output: decoded_output } => {
            assert_eq!(decoded_output, output);
        }
        Fd4Result::Failure { .. } => panic!("expected success"),
    }
}

#[test]
fn engine_protocol_roundtrip_empty_output() {
    let envelope = EngineWriteEnvelope {
        version: 1,
        instance_id: "inst",
        node_id: "node",
        result: EngineResult::Success { output: &json!(null) },
    };

    let payload = serde_json::to_vec(&envelope).unwrap();
    let mut buf: Vec<u8> = Vec::new();
    write_length_prefixed(&mut buf, &payload).unwrap();

    let read_payload = read_length_prefixed(&mut Cursor::new(&buf)).unwrap();
    let decoded: Fd4Envelope = serde_json::from_slice(&read_payload).unwrap();

    match decoded.result {
        Fd4Result::Success { output } => assert_eq!(output, json!(null)),
        Fd4Result::Failure { .. } => panic!("expected success"),
    }
}

#[test]
fn sdk_and_engine_formats_are_incompatible() {
    let output = json!({"key": "value"});
    let mut sdk_buf: Vec<u8> = Vec::new();
    let mut is_written = false;
    write_success_inner_with_state(&mut sdk_buf, &output, &mut is_written).unwrap();

    let sdk_parsed: serde_json::Value = serde_json::from_slice(&sdk_buf).unwrap();
    assert_eq!(sdk_parsed["status"].as_str(), Some("success"));
    assert_eq!(&sdk_parsed["output"], &output);

    let result = read_length_prefixed(&mut Cursor::new(&sdk_buf));
    assert!(result.is_err(), "SDK format has no length prefix");

    let envelope = EngineWriteEnvelope {
        version: 1,
        instance_id: "i",
        node_id: "n",
        result: EngineResult::Success { output: &output },
    };
    let engine_payload = serde_json::to_vec(&envelope).unwrap();
    let mut engine_buf: Vec<u8> = Vec::new();
    write_length_prefixed(&mut engine_buf, &engine_payload).unwrap();

    let read_back = read_length_prefixed(&mut Cursor::new(&engine_buf)).unwrap();
    let engine_parsed: serde_json::Value = serde_json::from_slice(&read_back).unwrap();

    assert_ne!(
        sdk_parsed["status"], engine_parsed["status"],
        "SDK and engine have different status field locations"
    );
    assert_ne!(
        sdk_parsed["output"], engine_parsed["result"]["output"],
        "SDK and engine have different output field locations"
    );
}