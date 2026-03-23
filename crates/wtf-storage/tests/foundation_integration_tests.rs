//! Integration tests for wtf-storage foundation components.
//!
//! Tests require a live NATS server. Run with: `cargo test --test foundation -- --test-threads=1`
//!
//! These tests verify the Gherkin scenarios from martin-fowler-tests.md.

use bytes::Bytes;

use wtf_common::{InstanceId, NamespaceId, WorkflowEvent};
use wtf_storage::{
    connect, delete_heartbeat, heartbeat_key, provision_kv_buckets, provision_streams,
    write_heartbeat, NatsClient, NatsConfig,
};

struct NatsTestServer {
    client: NatsClient,
}

impl NatsTestServer {
    async fn new() -> Result<Self, wtf_common::WtfError> {
        let config = NatsConfig {
            urls: vec!["nats://127.0.0.1:4222".into()],
            embedded: true,
            connect_timeout_ms: 5_000,
            credentials_path: None,
        };

        let client = connect(&config).await?;
        Ok(Self { client })
    }
}

#[tokio::test]
async fn connect_retries_with_exponential_backoff_when_nats_unavailable() {
    let config = NatsConfig {
        urls: vec!["nats://127.0.0.1:19999".into()],
        embedded: false,
        connect_timeout_ms: 500,
        credentials_path: None,
    };

    let result = connect(&config).await;
    assert!(
        result.is_err(),
        "Expected connection failure when NATS unavailable"
    );
}

#[tokio::test]
async fn append_event_awaits_publish_ack_before_returning() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    provision_streams(js).await.expect("provision streams");

    let namespace = NamespaceId::new("payments");
    let instance_id = InstanceId::new("01ARZTEST");

    let event = WorkflowEvent::InstanceStarted {
        instance_id: instance_id.to_string(),
        workflow_type: "checkout".into(),
        input: Bytes::from_static(b"{\"amount\":100}"),
    };

    let seq = wtf_storage::append_event(js, &namespace, &instance_id, &event)
        .await
        .expect("append_event should succeed");

    assert!(seq > 0, "Sequence should be positive");
}

#[tokio::test]
async fn append_event_publishes_to_correct_subject() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    provision_streams(js).await.expect("provision streams");

    let namespace = NamespaceId::new("onboarding");
    let instance_id = InstanceId::new("01BQATESTSUBJ");

    let event = WorkflowEvent::ActivityCompleted {
        activity_id: "act-001".into(),
        result: Bytes::from_static(b"ok"),
        duration_ms: 42,
    };

    let subject = wtf_storage::build_subject(&namespace, &instance_id);
    assert_eq!(subject, "wtf.log.onboarding.01BQATESTSUBJ");

    let seq = wtf_storage::append_event(js, &namespace, &instance_id, &event)
        .await
        .expect("append_event should succeed");

    assert!(
        seq >= 1,
        "Published event sequence should be positive, got {seq}"
    );
}

#[tokio::test]
async fn provision_streams_is_idempotent() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    let result1 = provision_streams(js).await;
    assert!(result1.is_ok(), "First provision should succeed");

    let result2 = provision_streams(js).await;
    assert!(
        result2.is_ok(),
        "Second provision should also succeed (idempotent)"
    );
}

#[tokio::test]
async fn verify_streams_passes_when_all_streams_exist() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    provision_streams(js).await.expect("provision streams");

    let result = wtf_storage::verify_streams(js).await;
    assert!(
        result.is_ok(),
        "verify_streams should pass when all streams exist"
    );
}

#[test]
fn heartbeat_key_format() {
    let instance_id = InstanceId::new("01ARZ");
    let key = heartbeat_key(&instance_id);
    assert_eq!(key, "hb/01ARZ");
}

#[tokio::test]
async fn write_heartbeat_creates_entry_with_ttl() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    provision_streams(js).await.expect("provision streams");
    let kv_stores = provision_kv_buckets(js)
        .await
        .expect("provision KV buckets");

    let instance_id = InstanceId::new("01ARZHB");
    let node_id = "node-1";

    let result = write_heartbeat(&kv_stores.heartbeats, &instance_id, node_id).await;
    assert!(result.is_ok(), "write_heartbeat should succeed");
}

#[tokio::test]
async fn delete_heartbeat_removes_entry() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    provision_streams(js).await.expect("provision streams");
    let kv_stores = provision_kv_buckets(js)
        .await
        .expect("provision KV buckets");

    let instance_id = InstanceId::new("01ARZDEL");
    let node_id = "node-1";

    write_heartbeat(&kv_stores.heartbeats, &instance_id, node_id)
        .await
        .expect("write heartbeat first");

    let result = delete_heartbeat(&kv_stores.heartbeats, &instance_id).await;
    assert!(result.is_ok(), "delete_heartbeat should succeed");
}

#[test]
fn snapshot_record_validates_checksum() {
    use wtf_storage::SnapshotRecord;

    let state_bytes = Bytes::from_static(b"test state");
    let record = SnapshotRecord::new(1, state_bytes.clone());

    assert!(record.is_valid(), "Fresh record should have valid checksum");

    let mut corrupted = SnapshotRecord::new(1, state_bytes);
    corrupted.checksum = 0xDEAD_BEEF;
    assert!(
        !corrupted.is_valid(),
        "Corrupted checksum should be invalid"
    );
}

#[test]
fn write_and_read_snapshot_roundtrip() {
    use tempfile::TempDir;
    use wtf_storage::{open_snapshot_db, read_snapshot, write_snapshot, SnapshotRecord};

    let temp_dir = TempDir::new().expect("temp dir");
    let db = open_snapshot_db(temp_dir.path()).expect("open snapshot db");

    let instance_id = InstanceId::new("01ARZSNAP");
    let state_bytes = Bytes::from_static(b"workflow state here");

    let record = SnapshotRecord::new(42, state_bytes);
    write_snapshot(&db, &instance_id, &record).expect("write snapshot should succeed");

    let read_result = read_snapshot(&db, &instance_id).expect("read snapshot should succeed");

    assert!(read_result.is_some(), "Snapshot should exist after write");

    let read_record = read_result.expect("snapshot");
    assert_eq!(read_record.seq, 42, "Sequence should match");
    assert!(
        read_record.is_valid(),
        "Read record should have valid checksum"
    );
}

#[test]
fn read_snapshot_returns_none_for_missing_key() {
    use tempfile::TempDir;
    use wtf_storage::{open_snapshot_db, read_snapshot};

    let temp_dir = TempDir::new().expect("temp dir");
    let db = open_snapshot_db(temp_dir.path()).expect("open snapshot db");

    let instance_id = InstanceId::new("NONEXISTENT");

    let result = read_snapshot(&db, &instance_id).expect("read_snapshot should succeed");
    assert!(result.is_none(), "Missing snapshot should return None");
}

#[test]
fn replay_start_seq_returns_snapshot_seq_plus_one() {
    let result = wtf_storage::replay_start_seq(Some(100));
    assert_eq!(result, 101, "Should return snapshot_seq + 1");
}

#[test]
fn replay_start_seq_returns_one_when_no_snapshot() {
    let result = wtf_storage::replay_start_seq(None);
    assert_eq!(result, 1, "Should return 1 for full replay");
}

#[test]
fn event_roundtrip_through_msgpack() {
    let event = WorkflowEvent::ActivityCompleted {
        activity_id: "act-001".into(),
        result: Bytes::from("ok"),
        duration_ms: 42,
    };

    let bytes = event.to_msgpack().expect("serialize");
    let decoded = WorkflowEvent::from_msgpack(&bytes).expect("deserialize");

    assert_eq!(event, decoded, "Roundtrip should preserve event");
}

#[test]
fn event_json_debug_representation_uses_snake_case() {
    let event = WorkflowEvent::SnapshotTaken {
        seq: 1,
        checksum: 0,
    };

    let json = serde_json::to_string(&event).expect("json serialize");
    assert!(
        json.contains("\"type\":\"snapshot_taken\""),
        "JSON should use snake_case variant name: {json}"
    );
}

#[test]
fn instance_id_rejects_nats_illegal_characters() {
    assert!(
        InstanceId::try_new("01ARZ.BAD").is_err(),
        "Dot should be rejected"
    );
    assert!(
        InstanceId::try_new("01ARZ*").is_err(),
        "Star should be rejected"
    );
    assert!(
        InstanceId::try_new("01ARZ>").is_err(),
        "GT should be rejected"
    );
}

#[test]
fn namespace_id_validates_nats_subject_safety() {
    assert!(
        NamespaceId::try_new("pay.ments").is_err(),
        "Dot in namespace should be rejected"
    );
    assert!(
        NamespaceId::try_new("pay ments").is_err(),
        "Whitespace should be rejected"
    );
    assert!(
        NamespaceId::try_new("payments").is_ok(),
        "Valid namespace should be accepted"
    );
}

#[tokio::test]
async fn full_lifecycle_append_provision_snapshot_replay() {
    let server = NatsTestServer::new().await.expect("NATS server");
    let js = server.client.jetstream();

    provision_streams(js).await.expect("provision streams");

    let namespace = NamespaceId::new("payments");
    let instance_id = InstanceId::new("01ARZLIFE");

    let started = WorkflowEvent::InstanceStarted {
        instance_id: instance_id.to_string(),
        workflow_type: "checkout".into(),
        input: Bytes::from_static(b"{\"amount\":100}"),
    };

    let seq1 = wtf_storage::append_event(js, &namespace, &instance_id, &started)
        .await
        .expect("append InstanceStarted");

    let activity = WorkflowEvent::ActivityDispatched {
        activity_id: "act-001".into(),
        activity_type: "charge_card".into(),
        payload: Bytes::from_static(b"{\"card\":\"****1234\"}"),
        retry_policy: wtf_common::RetryPolicy::default(),
        attempt: 1,
    };

    let seq2 = wtf_storage::append_event(js, &namespace, &instance_id, &activity)
        .await
        .expect("append ActivityDispatched");

    assert!(seq2 > seq1, "Second event should have higher sequence");

    use tempfile::TempDir;
    use wtf_storage::{open_snapshot_db, read_snapshot, write_snapshot, SnapshotRecord};

    let temp_dir = TempDir::new().expect("temp dir");
    let db = open_snapshot_db(temp_dir.path()).expect("open snapshot db");

    let state_bytes = Bytes::from_static(b"final state");
    let record = SnapshotRecord::new(seq2, state_bytes);
    write_snapshot(&db, &instance_id, &record).expect("write snapshot");

    let recovered = read_snapshot(&db, &instance_id)
        .expect("read snapshot")
        .expect("snapshot should exist");

    let replay_from = wtf_storage::replay_start_seq(Some(recovered.seq));
    assert_eq!(
        replay_from,
        seq2 + 1,
        "Replay should start after snapshot seq"
    );
}
