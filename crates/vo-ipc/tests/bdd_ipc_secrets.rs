use std::collections::BTreeMap;
use std::io::Cursor;
use vo_ipc::{read_envelope, write_envelope, Fd3Envelope, Fd4Envelope, TaskResult};

#[test]
fn bdd_fd3_secrets_passed_when_child_receives_task() {
    // Given: Fd3Envelope with API credentials
    let mut s = BTreeMap::new();
    s.insert("api_key".into(), "secret123".into());
    s.insert("token".into(), "bearer_token_xyz".into());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"action": "process"}),
        secrets: s,
        metadata: BTreeMap::new(),
    };

    // When: serialized and deserialized via FD3
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();

    // Then: secrets preserved
    assert_eq!(decoded.secrets.get("api_key").unwrap(), "secret123");
    assert_eq!(decoded.secrets.get("token").unwrap(), "bearer_token_xyz");
}

#[test]
fn bdd_fd3_empty_secrets_roundtrip_preserves_empty_map() {
    // Given: Fd3Envelope with no secrets
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"data": 42}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    // When: passes through FD3
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();

    // Then: secrets is empty
    assert!(decoded.secrets.is_empty());
}

#[test]
fn bdd_fd3_special_chars_in_secrets_preserved() {
    // Given: Fd3Envelope with special character secrets
    let mut s = BTreeMap::new();
    s.insert("password".into(), "p@ssw0rd!#$%^&*()".into());
    s.insert("json".into(), "{\"key\":\"value\"}".into());
    s.insert("multiline".into(), "line1\nline2\r\nline3".into());
    let env = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"action": "secure_task"}),
        secrets: s,
        metadata: BTreeMap::new(),
    };

    // When: transmitted over FD3
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();

    // Then: all secret values preserved exactly
    assert_eq!(
        decoded.secrets.get("password").unwrap(),
        "p@ssw0rd!#$%^&*()"
    );
    assert_eq!(decoded.secrets.get("json").unwrap(), "{\"key\":\"value\"}");
    assert_eq!(
        decoded.secrets.get("multiline").unwrap(),
        "line1\nline2\r\nline3"
    );
}

#[test]
fn bdd_fd4_result_roundtrip_independent_of_fd3_secrets() {
    // Given: Fd4Envelope with task result
    let env = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"processed": true, "count": 100}),
        },
    };

    // When: passes through FD4
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();

    // Then: result matches original
    match decoded.result {
        TaskResult::Success { output } => {
            assert_eq!(output, serde_json::json!({"processed": true, "count": 100}))
        }
        TaskResult::Failure { error } => panic!("Expected Success, got Failure: {:?}", error),
    }
}

#[test]
fn bdd_fd3_fd4_contract_secrets_not_in_fd4_response() {
    // Given: FD3 contains secrets, FD4 is response
    let mut s = BTreeMap::new();
    s.insert("db_password".into(), "super_secret_db_pass".into());
    let fd3 = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"query": "SELECT * FROM users"}),
        secrets: s,
        metadata: BTreeMap::new(),
    };
    let fd4 = Fd4Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        result: TaskResult::Success {
            output: serde_json::json!({"rows": 42}),
        },
    };

    // When: both transmitted over respective file descriptors
    let mut fd3_buf = Vec::new();
    let mut fd4_buf = Vec::new();
    write_envelope(&mut fd3_buf, &fd3).unwrap();
    write_envelope(&mut fd4_buf, &fd4).unwrap();
    let decoded_fd3: Fd3Envelope = read_envelope(&mut Cursor::new(fd3_buf)).unwrap();
    let decoded_fd4: Fd4Envelope = read_envelope(&mut Cursor::new(fd4_buf)).unwrap();

    // Then: FD4 must not contain secrets field, FD3 secrets preserved
    let fd4_json = serde_json::to_string(&decoded_fd4).unwrap();
    assert!(!fd4_json.contains("secrets"), "FD4 must not expose secrets");
    assert_eq!(
        decoded_fd3.secrets.get("db_password").unwrap(),
        "super_secret_db_pass"
    );
}
