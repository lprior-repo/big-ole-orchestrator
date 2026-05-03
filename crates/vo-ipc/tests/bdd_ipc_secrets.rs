//! BDD tests for IPC FD3/FD4 secret passing.

use std::collections::BTreeMap;
use std::io::Cursor;
use vo_ipc::envelope::{Fd3Envelope, Fd4Envelope, TaskError, TaskResult};
use vo_ipc::{read_envelope, write_envelope};

#[test]
fn given_secrets_when_sent_over_fd3_then_child_receives_intact() {
    let secrets = BTreeMap::from([
        ("API_KEY".into(), "sk_test_123".into()),
        ("DB_PASS".into(), "postgres://p@ssw0rd".into()),
    ]);
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst1".into(),
        node_id: "node1".into(),
        input: serde_json::json!({"query": "SELECT *"}),
        secrets,
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(decoded.secrets.get("API_KEY").unwrap(), "sk_test_123");
    assert_eq!(
        decoded.secrets.get("DB_PASS").unwrap(),
        "postgres://p@ssw0rd"
    );
}

#[test]
fn given_no_secrets_when_fd3_sent_then_child_receives_empty_map() {
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst2".into(),
        node_id: "node2".into(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert!(decoded.secrets.is_empty());
}

#[test]
fn given_special_chars_in_secrets_when_fd3_sent_then_child_receives_unchanged() {
    let secrets = BTreeMap::from([
        ("COMPLEX_KEY".into(), "p@ss!#$%^&*()_+{}[]|".into()),
        ("MULTILINE".into(), "line1\nline2\rline3".into()),
        ("UNICODE".into(), "日本語🔐".into()),
    ]);
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst3".into(),
        node_id: "node3".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(decoded.secrets["COMPLEX_KEY"], "p@ss!#$%^&*()_+{}[]|");
    assert_eq!(decoded.secrets["MULTILINE"], "line1\nline2\rline3");
    assert_eq!(decoded.secrets["UNICODE"], "日本語🔐");
}

#[test]
fn given_fd4_response_when_sent_then_result_preserves_output_values() {
    let output = serde_json::json!({"result": "success", "count": 42});
    let envelope = Fd4Envelope {
        version: 1,
        instance_id: "inst4".into(),
        node_id: "node4".into(),
        result: TaskResult::Success { output },
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    if let TaskResult::Success { output } = decoded.result {
        assert_eq!(output["result"], "success");
        assert_eq!(output["count"], 42);
    }
}

#[test]
fn given_task_failure_when_fd4_sent_then_child_receives_error_details() {
    let envelope = Fd4Envelope {
        version: 1,
        instance_id: "inst5".into(),
        node_id: "node5".into(),
        result: TaskResult::Failure {
            error: TaskError {
                code: "ERR_SECRET_MISSING".into(),
                message: "Required secret 'API_KEY' not found".into(),
                details: Some(serde_json::json!({"secret_name": "API_KEY"})),
            },
        },
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd4Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    match decoded.result {
        TaskResult::Failure { error } => {
            assert_eq!(error.code, "ERR_SECRET_MISSING");
            assert!(error.message.contains("API_KEY"));
        }
        TaskResult::Success { .. } => panic!("expected failure"),
        TaskResult::EffectIntent { .. } => panic!("expected failure"),
    }
}

#[test]
fn given_large_secrets_payload_when_fd3_sent_then_child_receives_all() {
    let secrets: BTreeMap<String, String> = (0..100)
        .map(|i| (format!("SECRET_{:03}", i), format!("value_{:03}", i)))
        .collect();
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst6".into(),
        node_id: "node6".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert_eq!(decoded.secrets.len(), 100);
    assert_eq!(decoded.secrets.get("SECRET_050").unwrap(), "value_050");
}

#[test]
fn given_mixed_case_secrets_when_fd3_sent_then_preserves_key_case() {
    let secrets = BTreeMap::from([
        ("CamelCase".into(), "camel".into()),
        ("lowercase".into(), "lower".into()),
        ("UPPERCASE".into(), "upper".into()),
    ]);
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst7".into(),
        node_id: "node7".into(),
        input: serde_json::json!({}),
        secrets,
        metadata: BTreeMap::new(),
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &envelope).unwrap();
    let decoded: Fd3Envelope = read_envelope(&mut Cursor::new(buf)).unwrap();
    assert!(decoded.secrets.contains_key("CamelCase"));
    assert!(decoded.secrets.contains_key("lowercase"));
    assert!(decoded.secrets.contains_key("UPPERCASE"));
}
