use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::PathBuf;
use vo_ipc::*;

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fixture_driver"))
}

fn config(payload: impl AsRef<[u8]>, timeout_ms: u64) -> SubprocessConfig {
    SubprocessConfig::new(fixture_binary(), timeout_ms, payload.as_ref().to_vec()).unwrap()
}

fn make_fd3_envelope(
    instance_id: &str,
    node_id: &str,
    input: serde_json::Value,
    secrets: BTreeMap<String, String>,
) -> Fd3Envelope {
    Fd3Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        input,
        secrets,
        metadata: BTreeMap::new(),
    }
}

fn make_fd4_success(
    instance_id: &str,
    node_id: &str,
    output: serde_json::Value,
) -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        result: TaskResult::Success { output },
    }
}

fn make_fd4_failure(
    instance_id: &str,
    node_id: &str,
    code: &str,
    message: &str,
) -> Fd4Envelope {
    Fd4Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        result: TaskResult::Failure {
            error: TaskError {
                code: code.to_string(),
                message: message.to_string(),
                details: None,
            },
        },
    }
}

#[test]
fn fd3_envelope_serialized_via_length_prefixed_frame() {
    let env = make_fd3_envelope("i1", "n1", serde_json::json!({"key": "val"}), BTreeMap::new());
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    assert!(buf.len() > 4, "must have 4-byte length prefix + payload");

    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    assert_eq!(len, buf.len() - 4, "length prefix must match payload size");
}

#[test]
fn fd4_envelope_serialized_via_length_prefixed_frame() {
    let env = make_fd4_success("i1", "n1", serde_json::json!(42));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    assert_eq!(len, buf.len() - 4);
}

#[test]
fn fd3_secrets_are_in_json_payload_not_env() {
    let mut secrets = BTreeMap::new();
    secrets.insert("STRIPE_KEY".to_string(), "sk_live_abc123".to_string());
    secrets.insert("DB_PASSWORD".to_string(), "supersecret".to_string());

    let env = make_fd3_envelope("i1", "n1", serde_json::json!({}), secrets);
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    let payload = &buf[4..];
    let payload_str = String::from_utf8(payload.to_vec()).unwrap();

    assert!(payload_str.contains("STRIPE_KEY"));
    assert!(payload_str.contains("sk_live_abc123"));
    assert!(payload_str.contains("DB_PASSWORD"));
    assert!(payload_str.contains("supersecret"));
}

#[test]
fn fd3_roundtrip_preserves_secret_values() {
    let mut secrets = BTreeMap::new();
    secrets.insert("API_KEY".to_string(), "secret123".to_string());
    secrets.insert("TOKEN".to_string(), "tok_456".to_string());

    let env = make_fd3_envelope("inst1", "node1", serde_json::json!({"action": "process"}), secrets);
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(decoded.secrets.get("API_KEY").unwrap(), "secret123");
    assert_eq!(decoded.secrets.get("TOKEN").unwrap(), "tok_456");
}

#[test]
fn fd4_success_roundtrip_preserves_output_value() {
    let cases = vec![
        serde_json::json!(null),
        serde_json::json!(42),
        serde_json::json!("hello"),
        serde_json::json!([1, 2, 3]),
        serde_json::json!({"nested": {"deep": true}}),
        serde_json::json!(false),
    ];

    for (idx, output) in cases.into_iter().enumerate() {
        let env = make_fd4_success("i", "n", output.clone());
        let mut buf = Vec::new();
        write_envelope(&mut buf, &env).unwrap();
        let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
        assert_eq!(decoded.result, TaskResult::Success { output }, "case {}", idx);
    }
}

#[test]
fn fd4_failure_roundtrip_preserves_error_details() {
    let env = make_fd4_failure("inst1", "node1", "ERR_TIMEOUT", "task exceeded 30s limit");
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();

    match decoded.result {
        TaskResult::Failure { error } => {
            assert_eq!(error.code, "ERR_TIMEOUT");
            assert_eq!(error.message, "task exceeded 30s limit");
            assert!(error.details.is_none());
        }
        TaskResult::Success { .. } => panic!("expected failure"),
    }
}

#[test]
fn fd4_failure_with_details_roundtrip() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Failure {
            error: TaskError {
                code: "ERR_VALIDATION".into(),
                message: "invalid input".into(),
                details: Some(serde_json::json!({"field": "email", "reason": "empty"})),
            },
        },
    };

    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(env, decoded);
}

#[test]
fn engine_receive_rejects_wrong_instance_id() {
    let env = make_fd4_success("instA", "node1", serde_json::json!(null));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    let result = engine_receive_envelope(&mut Cursor::new(buf), "instB", "node1");
    assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
}

#[test]
fn engine_receive_rejects_wrong_node_id() {
    let env = make_fd4_success("inst1", "nodeA", serde_json::json!(null));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    let result = engine_receive_envelope(&mut Cursor::new(buf), "inst1", "nodeB");
    assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
}

#[test]
fn engine_receive_rejects_both_ids_wrong() {
    let env = make_fd4_success("instA", "nodeA", serde_json::json!(null));
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();

    let result = engine_receive_envelope(&mut Cursor::new(buf), "instB", "nodeB");
    assert!(matches!(result, Err(IpcError::IdentityMismatch { .. })));
}

#[test]
fn fd3_metadata_roundtrip_preserved() {
    let mut metadata = BTreeMap::new();
    metadata.insert("trace_id".to_string(), "abc-123".to_string());
    metadata.insert("span_id".to_string(), "def-456".to_string());

    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata,
    };

    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(decoded.metadata.get("trace_id").unwrap(), "abc-123");
    assert_eq!(decoded.metadata.get("span_id").unwrap(), "def-456");
}

#[test]
fn fd3_empty_secrets_and_metadata_roundtrip() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert!(decoded.secrets.is_empty());
    assert!(decoded.metadata.is_empty());
}

#[test]
fn max_payload_size_enforced_on_write() {
    let huge_input = serde_json::json!("x".repeat(11 * 1024 * 1024));
    let env = make_fd3_envelope("i", "n", huge_input, BTreeMap::new());
    let mut buf = Vec::new();
    let result = write_envelope(&mut buf, &env);
    assert!(matches!(result, Err(IpcError::PayloadTooLarge(_))));
}

#[test]
fn max_payload_size_enforced_on_read() {
    let len: u32 = 10_485_761;
    let mut buf = len.to_be_bytes().to_vec();
    buf.extend(vec![0u8; 100]);
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(result, Err(IpcError::PayloadTooLarge(_))));
}

#[tokio::test]
async fn cloexec_grandchild_does_not_block_parent() {
    let output = run_subprocess(config("grandchild-hold 2000", 500)).await.unwrap();
    assert_eq!(output.fd4_bytes, b"child-done");
}

#[tokio::test]
async fn fd3_payload_delivery_via_dedicated_fd() {
    let output = run_subprocess(config("echo-fd3 hello world", 500)).await.unwrap();
    assert_eq!(output.fd4_bytes, b"echo-fd3 hello world");
}

#[test]
fn write_then_read_preserves_envelope_order() {
    let env1 = make_fd3_envelope("i1", "n1", serde_json::json!(1), BTreeMap::new());
    let env2 = make_fd3_envelope("i2", "n2", serde_json::json!(2), BTreeMap::new());

    let mut buf = Vec::new();
    write_envelope(&mut buf, &env1).unwrap();
    write_envelope(&mut buf, &env2).unwrap();

    let mut cursor = Cursor::new(buf);
    let decoded1: Fd3Envelope = read_envelope(&mut cursor).unwrap();
    let decoded2: Fd3Envelope = read_envelope(&mut cursor).unwrap();

    assert_eq!(decoded1.instance_id, "i1");
    assert_eq!(decoded2.instance_id, "i2");
}
