//! BDD tests for IPC FD3/FD4 secret passing scenarios.

use std::collections::BTreeMap;
use vo_ipc::envelope::{Fd3Envelope, Fd4Envelope, TaskResult};

// Given/When/Then scenarios for secret passing over IPC channels.

#[test]
fn given_valid_secrets_when_sent_via_fd3_then_child_receives_them() {
    // Given
    let secrets: BTreeMap<String, String> = [
        ("AWS_SECRET_KEY".into(), "secret123".into()),
        ("API_TOKEN".into(), "token456".into()),
    ]
    .into();

    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst-1".into(),
        node_id: "node-1".into(),
        input: serde_json::json!({"action": "process"}),
        secrets,
        metadata: BTreeMap::new(),
    };

    // When
    let serialized = serde_json::to_vec(&envelope).unwrap();
    let deserialized: Fd3Envelope = serde_json::from_slice(&serialized).unwrap();

    // Then
    assert_eq!(
        deserialized.secrets.get("AWS_SECRET_KEY"),
        Some(&"secret123".into())
    );
    assert_eq!(
        deserialized.secrets.get("API_TOKEN"),
        Some(&"token456".into())
    );
    assert_eq!(deserialized.secrets.len(), 2);
}

#[test]
fn given_empty_secrets_when_fd3_sent_then_child_receives_empty_map() {
    // Given
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst-2".into(),
        node_id: "node-2".into(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    // When
    let serialized = serde_json::to_vec(&envelope).unwrap();
    let deserialized: Fd3Envelope = serde_json::from_slice(&serialized).unwrap();

    // Then
    assert!(deserialized.secrets.is_empty());
}

#[test]
fn given_sensitive_data_when_fd4_response_sent_then_result_includes_secrets() {
    // Given
    let output: BTreeMap<String, String> = [
        ("processed_data".into(), "result789".into()),
        ("status".into(), "completed".into()),
    ]
    .into();

    let output_value: serde_json::Map<String, serde_json::Value> = output
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();

    let envelope = Fd4Envelope {
        version: 1,
        instance_id: "inst-3".into(),
        node_id: "node-3".into(),
        result: TaskResult::Success {
            output: serde_json::Value::Object(output_value),
        },
    };

    // When
    let serialized = serde_json::to_vec(&envelope).unwrap();
    let deserialized: Fd4Envelope = serde_json::from_slice(&serialized).unwrap();

    // Then
    if let TaskResult::Success { output } = deserialized.result {
        assert_eq!(output["processed_data"], "result789");
        assert_eq!(output["status"], "completed");
    } else {
        panic!("Expected Success result");
    }
}

#[test]
fn given_missing_secret_when_child_requests_it_then_returns_none() {
    // Given
    let envelope = Fd3Envelope {
        version: 1,
        instance_id: "inst-4".into(),
        node_id: "node-4".into(),
        input: serde_json::json!({}),
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    };

    // When
    let received = envelope.secrets.get("NONEXISTENT_SECRET");

    // Then
    assert!(received.is_none());
}
