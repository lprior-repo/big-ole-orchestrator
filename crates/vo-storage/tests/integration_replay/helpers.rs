//! Shared helpers for integration_replay tests.

use vo_storage::query::epoch_prefix_generator;
use vo_storage::query::lineage_prefix_generator;
use vo_types::{EventEnvelope, InstanceId};

pub fn make_envelope_json(seq: u64, instance_id: &str) -> Vec<u8> {
    serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": 1000 + seq,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1"},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

pub fn make_bad_envelope_json() -> Vec<u8> {
    b"not valid json".to_vec()
}

pub fn make_unsupported_version_envelope_json(instance_id: &str) -> Vec<u8> {
    serde_json::json!({
        "version": 99,
        "instance_id": instance_id,
        "sequence": 1,
        "timestamp_ms": 1000,
        "payload": {},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

pub fn make_envelope_json_with_version(seq: u64, instance_id: &str, version: u8) -> Vec<u8> {
    serde_json::json!({
        "version": version,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": 1000 + seq,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1"},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

pub fn make_envelope_json_with_timestamp(seq: u64, instance_id: &str, timestamp_ms: u64) -> Vec<u8> {
    serde_json::json!({
        "version": 1,
        "instance_id": instance_id,
        "sequence": seq,
        "timestamp_ms": timestamp_ms,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-1"},
        "metadata": {}
    })
    .to_string()
    .into_bytes()
}

pub fn insert_event(partition: &fjall::Keyspace, instance_id: &str, seq: u64, value: &[u8]) {
    let mut key = instance_id.as_bytes().to_vec();
    key.extend_from_slice(&seq.to_be_bytes());
    partition.insert(&key, value).unwrap();
}

pub fn setup_keyspace() -> (tempfile::TempDir, fjall::Database) {
    let folder = tempfile::tempdir().expect("temp dir");
    let db = fjall::Database::builder(folder.path()).open().expect("database");
    db.keyspace("events", || fjall::KeyspaceCreateOptions::default())
        .expect("partition");
    (folder, db)
}

pub fn parse_instance_id(s: &str) -> InstanceId {
    InstanceId::parse(s).expect("valid instance ID")
}

pub fn parse_envelope(bytes: &[u8]) -> EventEnvelope {
    EventEnvelope::from_bytes(bytes).expect("valid test envelope")
}

pub fn insert_lineage_event(
    partition: &fjall::Keyspace,
    lineage_id: &str,
    epoch: u64,
    seq: u64,
    value: &[u8],
) {
    let lineage_prefix = lineage_prefix_generator(lineage_id).unwrap();
    let epoch_bytes = epoch.to_be_bytes();
    let mut key = lineage_prefix;
    key.extend_from_slice(&epoch_bytes);
    key.extend_from_slice(&seq.to_be_bytes());
    partition.insert(&key, value).unwrap();
}