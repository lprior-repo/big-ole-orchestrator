use std::io::Cursor;
use vo_ipc::*;

#[test]
fn current_version_is_1() {
    assert_eq!(CURRENT_VERSION, 1);
}

#[test]
fn version_negotiation_new_has_current_version() {
    let vn = VersionNegotiation::new();
    assert_eq!(vn.supported_versions, vec![1]);
}

#[test]
fn version_negotiation_negotiate_returns_version_when_supported() {
    let vn = VersionNegotiation::new();
    assert_eq!(vn.negotiate(1).unwrap(), 1);
}

#[test]
fn version_negotiation_negotiate_returns_error_when_unsupported() {
    let vn = VersionNegotiation::new();
    let result = vn.negotiate(2);
    assert!(matches!(result, Err(IpcError::VersionMismatch(2))));
}

#[test]
fn version_negotiation_negotiate_returns_error_for_version_0() {
    let vn = VersionNegotiation::new();
    let result = vn.negotiate(0);
    assert!(matches!(result, Err(IpcError::VersionMismatch(0))));
}

#[test]
fn version_negotiation_negotiate_returns_error_for_version_255() {
    let vn = VersionNegotiation::new();
    let result = vn.negotiate(255);
    assert!(matches!(result, Err(IpcError::VersionMismatch(255))));
}

#[test]
fn negotiate_version_succeeds_for_version_1() {
    assert_eq!(negotiate_version(1).unwrap(), 1);
}

#[test]
fn negotiate_version_fails_for_unsupported_version() {
    let result = negotiate_version(2);
    assert!(matches!(result, Err(IpcError::VersionMismatch(2))));
}

#[test]
fn version_negotiation_implements_default() {
    let vn = VersionNegotiation::default();
    assert_eq!(vn.supported_versions, vec![1]);
}

#[test]
fn read_envelope_rejects_version_0() {
    let env_json = serde_json::json!({
        "version": 0,
        "instance_id": "inst1",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::VersionMismatch(0))));
}

#[test]
fn read_envelope_rejects_version_255() {
    let env_json = serde_json::json!({
        "version": 255,
        "instance_id": "inst1",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::VersionMismatch(255))));
}

#[test]
fn read_envelope_accepts_version_1() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "inst1",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_ok());
}

#[test]
fn read_envelope_rejects_float_version() {
    let raw = b"{\"version\":1.5,\"instance_id\":\"i\",\"node_id\":\"n\",\"input\":{},\"secrets\":{},\"metadata\":{}}";
    let mut buf = (raw.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(raw);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_rejects_string_version() {
    let raw = b"{\"version\":\"1\",\"instance_id\":\"i\",\"node_id\":\"n\",\"input\":{},\"secrets\":{},\"metadata\":{}}";
    let mut buf = (raw.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(raw);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_rejects_negative_version() {
    let raw = b"{\"version\":-1,\"instance_id\":\"i\",\"node_id\":\"n\",\"input\":{},\"secrets\":{},\"metadata\":{}}";
    let mut buf = (raw.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(raw);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_rejects_very_large_version() {
    let raw = b"{\"version\":999999,\"instance_id\":\"i\",\"node_id\":\"n\",\"input\":{},\"secrets\":{},\"metadata\":{}}";
    let mut buf = (raw.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(raw);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_rejects_missing_instance_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_rejects_missing_node_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "inst1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_rejects_numeric_instance_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": 42,
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn read_envelope_rejects_numeric_node_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "inst1",
        "node_id": 99,
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn read_envelope_rejects_empty_string_instance_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::SchemaViolation(msg)) if msg.contains("instance_id")));
}

#[test]
fn read_envelope_rejects_empty_string_node_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "inst1",
        "node_id": "",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::SchemaViolation(msg)) if msg.contains("node_id")));
}

#[test]
fn read_envelope_rejects_special_chars_in_instance_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "inst-1",
        "node_id": "node1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn read_envelope_rejects_space_in_node_id() {
    let env_json = serde_json::json!({
        "version": 1,
        "instance_id": "inst1",
        "node_id": "node 1",
        "input": {},
        "secrets": {},
        "metadata": {}
    });
    let payload = serde_json::to_vec(&env_json).unwrap();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::SchemaViolation(_))));
}

#[test]
fn read_envelope_handles_empty_json_object() {
    let raw = b"{}";
    let mut buf = (raw.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(raw);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_handles_json_array() {
    let raw = b"[]";
    let mut buf = (raw.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(raw);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn read_envelope_handles_deeply_nested_json() {
    let mut json_str = String::new();
    for _ in 0..50 {
        json_str.push_str("{\"a\":");
    }
    json_str.push('1');
    for _ in 0..50 {
        json_str.push('}');
    }
    let payload = json_str.as_bytes();
    let mut buf = (payload.len() as u32).to_be_bytes().to_vec();
    buf.extend_from_slice(payload);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(result.is_err());
}

#[test]
fn write_envelope_produces_valid_json_payload() {
    use std::collections::BTreeMap;
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"test": true}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let payload = &buf[4..];
    let parsed: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert!(parsed.is_object());
}
