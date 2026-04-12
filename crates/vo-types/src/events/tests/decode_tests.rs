
use crate::events::decode::decode_event;
use crate::events::error::Error;
use crate::events::payload::EventPayload;

#[test]
fn decode_event_returns_ok_when_envelope_and_payload_are_valid() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123", "binary_hash": "abc123", "version": 1}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    let (envelope, payload) = result.unwrap();
    assert_eq!(envelope.schema_version, 1);
    assert_eq!(envelope.instance_id, "wf-123");
    assert_eq!(envelope.sequence, 1);
    assert_eq!(envelope.timestamp_ms, 1000);
    assert!(matches!(payload, EventPayload::WorkflowStarted { .. }));
}

#[test]
fn decode_event_returns_envelope_decode_failed_when_envelope_is_malformed() {
    let json = r#"{"version": 1, "instance_id": "wf-123""#;
    let result = decode_event(json.as_bytes());
    assert!(matches!(result, Err(Error::EnvelopeDecodeFailed(_))));
}

#[test]
fn decode_event_returns_payload_decode_failed_when_payload_is_invalid() {
    let json = r#"{"version": 1, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "UnknownType", "version": 1}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    assert!(matches!(result, Err(Error::PayloadDecodeFailed(_))));
}

#[test]
fn decode_event_returns_payload_decode_skipped_when_envelope_version_exceeds_max() {
    let json = r#"{"version": 2, "instance_id": "wf-123", "sequence": 1, "timestamp_ms": 1000, "payload": {"type": "WorkflowStarted", "version": 1}, "metadata": {}}"#;
    let result = decode_event(json.as_bytes());
    assert_eq!(result, Err(Error::PayloadDecodeSkipped));
}

#[test]
fn decode_event_returns_continued_as_new_for_valid_full_event() {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": "inst-1",
        "sequence": 42,
        "timestamp_ms": 9999,
        "payload": {
            "type": "ContinuedAsNew",
            "workflow_id": "wf-1",
            "lineage_id": "lin-1",
            "old_epoch": 2,
            "new_epoch": 3,
            "version": 1
        },
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).expect("serialize");
    let result = decode_event(&bytes);
    let (_envelope, payload) = result.expect("decode should succeed");
    match payload {
        EventPayload::ContinuedAsNew {
            workflow_id,
            lineage_id,
            old_epoch,
            new_epoch,
        } => {
            assert_eq!(workflow_id, "wf-1");
            assert_eq!(lineage_id, "lin-1");
            assert_eq!(old_epoch, 2);
            assert_eq!(new_epoch, 3);
        }
        other => panic!("Expected ContinuedAsNew, got {other:?}"),
    }
}

#[test]
fn decode_event_returns_correct_binary_hash_and_dag_topology_in_full_pipeline() {
    let json = serde_json::json!({
        "version": 1,
        "instance_id": "inst-1",
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {
            "type": "WorkflowStarted",
            "workflow_id": "wf-123",
            "dag_topology": {"nodes": []},
            "binary_hash": "sha256abc",
            "version": 1
        },
        "metadata": {}
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = decode_event(&bytes);
    let (_envelope, payload) = result.unwrap();
    match payload {
        EventPayload::WorkflowStarted {
            workflow_id,
            dag_topology,
            binary_hash,
        } => {
            assert_eq!(workflow_id, "wf-123");
            assert_eq!(dag_topology, serde_json::json!({"nodes": []}));
            assert_eq!(binary_hash, "sha256abc");
        }
        other => panic!("Expected WorkflowStarted, got {other:?}"),
    }
}
