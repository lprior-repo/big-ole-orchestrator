use std::collections::BTreeMap;
use std::io::Cursor;
use vo_ipc::{read_envelope, write_envelope, Fd3Envelope, Fd4Envelope, TaskResult};

#[test]
fn fd3_secrets_roundtrip_preserves_single_secret() {
    // Given an envelope with a single secret
    let mut secrets = BTreeMap::new();
    secrets.insert("api_key".to_string(), "secret123".to_string());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"action": "process"}),
        secrets,
        metadata: BTreeMap::new(),
    };

    // When serialized and deserialized
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    // Then secrets are preserved exactly
    assert_eq!(
        decoded.secrets.get("api_key"),
        Some(&"secret123".to_string())
    );
    assert_eq!(env.secrets, decoded.secrets);
}

#[test]
fn fd3_secrets_roundtrip_preserves_multiple_secrets() {
    // Given an envelope with multiple secrets
    let mut secrets = BTreeMap::new();
    secrets.insert("api_key".to_string(), "key_val".to_string());
    secrets.insert("private_key".to_string(), "pem_data_here".to_string());
    secrets.insert("token".to_string(), " bearer_token".to_string());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };

    // When roundtripped through write/read
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    // Then all secrets remain intact
    assert_eq!(decoded.secrets.len(), 3);
    assert_eq!(env.secrets, decoded.secrets);
}

#[test]
fn fd3_empty_secrets_roundtrip_succeeds() {
    // Given an envelope with empty secrets
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"foo": "bar"}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    // When roundtripped
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    // Then secrets remain empty
    assert!(decoded.secrets.is_empty());
}

#[test]
fn fd3_secrets_special_characters_preserved() {
    // Given secrets with special characters
    let mut secrets = BTreeMap::new();
    secrets.insert("password".to_string(), "p@ss!#$%^&*()".to_string());
    secrets.insert("json_data".to_string(), r#"{"key":"value"}"#.to_string());
    secrets.insert("multiline".to_string(), "line1\nline2\r\nline3".to_string());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };

    // When roundtripped
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    // Then special characters are preserved
    assert_eq!(
        decoded.secrets.get("password"),
        Some(&"p@ss!#$%^&*()".to_string())
    );
    assert_eq!(
        decoded.secrets.get("json_data"),
        Some(&r#"{"key":"value"}"#.to_string())
    );
    assert_eq!(
        decoded.secrets.get("multiline"),
        Some(&"line1\nline2\r\nline3".to_string())
    );
}

#[test]
fn fd4_envelope_has_no_secrets_field() {
    // Given an FD4 envelope (response from child to engine)
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"status": "ok"}),
        },
    };

    // When serialized
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();

    // Then the payload contains no secrets
    let payload = &buffer[4..];
    let json_str = String::from_utf8_lossy(payload);
    assert!(!json_str.contains("secrets"));
}

#[test]
fn fd3_secrets_unicode_characters_preserved() {
    // Given secrets with unicode characters
    let mut secrets = BTreeMap::new();
    secrets.insert("greeting".to_string(), "こんにちは".to_string());
    secrets.insert("emoji".to_string(), "🔐🔑".to_string());
    secrets.insert("mixed".to_string(), "héllo wörld".to_string());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };

    // When roundtripped
    let mut buffer = Vec::new();
    write_envelope(&mut buffer, &env).unwrap();
    let mut reader = Cursor::new(buffer);
    let decoded: Fd3Envelope = read_envelope(&mut reader).unwrap();

    // Then unicode is preserved
    assert_eq!(
        decoded.secrets.get("greeting"),
        Some(&"こんにちは".to_string())
    );
    assert_eq!(decoded.secrets.get("emoji"), Some(&"🔐🔑".to_string()));
    assert_eq!(
        decoded.secrets.get("mixed"),
        Some(&"héllo wörld".to_string())
    );
}
