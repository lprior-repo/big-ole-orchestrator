//! Tests for command identity metadata propagation through IPC surfaces (ADR-036).

use std::collections::BTreeMap;
use std::io::Cursor;
use vo_ipc::*;

#[test]
fn fd3_envelope_carries_command_identity_in_metadata() {
    let mut metadata = BTreeMap::new();
    metadata.insert("command_id".to_string(), "cmd-ipc-001".to_string());
    metadata.insert("correlation_id".to_string(), "corr-ipc-001".to_string());
    metadata.insert("issuer".to_string(), "operator".to_string());

    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        input: serde_json::json!({"action": "start"}),
        secrets: BTreeMap::new(),
        metadata,
    };

    // Round-trip through IPC serialization
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    assert_eq!(decoded.metadata.get("command_id").unwrap(), "cmd-ipc-001");
    assert_eq!(
        decoded.metadata.get("correlation_id").unwrap(),
        "corr-ipc-001"
    );
    assert_eq!(decoded.metadata.get("issuer").unwrap(), "operator");
}

#[test]
fn fd3_metadata_preserves_command_identity_across_roundtrip() {
    let mut metadata = BTreeMap::new();
    metadata.insert("command_id".to_string(), "cmd-roundtrip".to_string());
    metadata.insert("causation_id".to_string(), "cause-parent".to_string());

    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        input: serde_json::json!(null),
        secrets: BTreeMap::new(),
        metadata,
    };

    // Serialize -> deserialize
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    assert_eq!(
        decoded.metadata, env.metadata,
        "metadata map must be preserved exactly"
    );
}

#[test]
fn engine_receive_envelope_validates_identity_context() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Success {
            output: serde_json::json!({"status": "ok"}),
        },
    };
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let result = engine_receive_envelope(&mut reader, "inst1", "node1");
    assert!(result.is_ok(), "matching identity should succeed");
}

#[test]
fn fd4_identity_mismatch_rejects_command_from_wrong_instance() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".to_string(),
        node_id: "node1".to_string(),
        result: TaskResult::Success {
            output: serde_json::json!({"status": "ok"}),
        },
    };
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let mut reader = Cursor::new(buffer);
    let result = engine_receive_envelope(&mut reader, "inst2", "node1");
    assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
}
