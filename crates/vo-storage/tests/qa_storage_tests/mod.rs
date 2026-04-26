//! QA tests for vo-storage: Fjall persistence, event store, snapshots, blobs.
//!
//! All tests use real Fjall instances in temp directories. No mocks.

mod fjall_persistence;
mod event_store;
mod snapshots;
mod blob_store;
mod partitions;
mod content_address;
mod blob_record;
mod batch_writes;
mod prefix_scans;

// ── Shared helpers ──────────────────────────────────────────────────────────

use vo_storage::blob_store::{BlobRecord, BlobStore, BlobStoreError, ContentAddress};
use vo_storage::codec::encode_event_key;
use vo_storage::fs_store::FsBlobStore;
use vo_storage::partitions::{
    create_partition_layout, open_all_partitions, ALL_PARTITIONS, BLOB_PARTITIONS, COLD_PARTITIONS,
    HOT_PARTITIONS,
};
use vo_storage::snapshots::{
    compact_snapshots, encode_snapshot_key, snapshot_load_latest, snapshot_write,
    AtomicSnapshotWriter, SnapshotPolicy,
};
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn open_partition(name: &str) -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("fjall open");
    let ks = db
        .keyspace(name, || fjall::KeyspaceCreateOptions::default())
        .expect("partition open");
    (dir, db, ks)
}

fn open_fjall() -> (tempfile::TempDir, fjall::Database) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = fjall::Database::builder(dir.path())
        .open()
        .expect("fjall open");
    (dir, db)
}

fn make_instance_id() -> InstanceId {
    InstanceId::from_bytes([0x01; 16])
}

fn make_instance_state(counter: u64) -> InstanceState {
    InstanceState { counter }
}

fn make_envelope(instance_id: &InstanceId, sequence: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms: 1000 + sequence,
        payload: serde_json::json!({"type": "TestEvent", "seq": sequence}),
        metadata: EventMetadata::default(),
    }
}

fn encode_event_seq(id: &InstanceId, seq: u64) -> [u8; 24] {
    let sn = vo_types::SequenceNumber::try_from(seq).unwrap();
    encode_event_key(id, &sn).unwrap()
}
