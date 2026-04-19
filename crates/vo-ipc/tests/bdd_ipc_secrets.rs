use std::collections::BTreeMap;
use std::io::Cursor;
use vo_ipc::{read_envelope, write_envelope, Fd3Envelope, Fd4Envelope, TaskResult};

#[test]
fn fd3_secrets_roundtrip_preserves_them() {
    let mut secrets = BTreeMap::new();
    secrets.insert("api_key".to_string(), "secret123".to_string());
    secrets.insert("token".to_string(), "bearer_tok".to_string());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"action": "process"}),
        secrets,
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buffer)).unwrap();

    assert_eq!(env.secrets, decoded.secrets);
}

#[test]
fn fd3_empty_secrets_roundtrip_succeeds() {
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"foo": "bar"}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buffer)).unwrap();

    assert!(decoded.secrets.is_empty());
}

#[test]
fn fd3_secrets_special_chars_preserved() {
    let mut secrets = BTreeMap::new();
    secrets.insert("json".to_string(), r#"{"key":"v"}"#.to_string());
    secrets.insert("multiline".to_string(), "a\nb\r\nc".to_string());
    secrets.insert("unicode".to_string(), "héllo 🔐".to_string());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buffer)).unwrap();

    assert_eq!(
        decoded.secrets.get("json"),
        Some(&r#"{"key":"v"}"#.to_string())
    );
    assert_eq!(
        decoded.secrets.get("multiline"),
        Some(&"a\nb\r\nc".to_string())
    );
    assert_eq!(
        decoded.secrets.get("unicode"),
        Some(&"héllo 🔐".to_string())
    );
}

#[test]
fn fd4_envelope_has_no_secrets_field() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"ok": true}),
        },
    };

    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    let payload = &buffer[4..];
    let json_str = String::from_utf8_lossy(payload);
    assert!(!json_str.contains("secrets"));
}
