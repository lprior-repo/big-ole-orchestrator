//! QA tests for vo-storage: Fjall persistence, event store, snapshots, blobs.
//!
//! All tests use real Fjall instances in temp directories. No mocks.

use vo_storage::codec::encode_event_key;
use vo_storage::fs_store::FsBlobStore;
use vo_storage::partitions::{
    create_partition_layout, open_all_partitions, ALL_PARTITIONS, BLOB_PARTITIONS,
    COLD_PARTITIONS, HOT_PARTITIONS,
};
use vo_storage::snapshots::{
    compact_snapshots, encode_snapshot_key, snapshot_load_latest, snapshot_write,
    AtomicSnapshotWriter, SnapshotPolicy,
};
use vo_storage::blob_store::{BlobStore, BlobStoreError, BlobRecord, ContentAddress};
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

// ══════════════════════════════════════════════════════════════════════════════
// Section 1: Fjall Persistence — put/get/delete on raw keyspaces
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn fjall_put_then_get_returns_stored_value() {
    let (_dir, _db, ks) = open_partition("test_put_get");

    ks.insert(b"key-1", b"value-1").expect("insert");
    let val = ks.get(b"key-1").expect("get").expect("value exists");
    assert_eq!(val.as_ref(), b"value-1");
}

#[test]
fn fjall_get_missing_key_returns_none() {
    let (_dir, _db, ks) = open_partition("test_missing");

    let val = ks.get(b"no-such-key").expect("get");
    assert!(val.is_none());
}

#[test]
fn fjall_delete_removes_key() {
    let (_dir, _db, ks) = open_partition("test_delete");

    ks.insert(b"key-del", b"val-del").expect("insert");
    assert!(ks.get(b"key-del").expect("get").is_some());

    ks.remove(b"key-del").expect("remove");
    assert!(ks.get(b"key-del").expect("get").is_none());
}

#[test]
fn fjall_overwrite_replaces_value() {
    let (_dir, _db, ks) = open_partition("test_overwrite");

    ks.insert(b"k", b"v1").expect("insert 1");
    ks.insert(b"k", b"v2").expect("insert 2");

    let val = ks.get(b"k").expect("get").expect("value");
    assert_eq!(val.as_ref(), b"v2");
}

#[test]
fn fjall_persists_across_multiple_inserts() {
    let (_dir, _db, ks) = open_partition("test_multi");

    for i in 0..100u32 {
        let key = format!("key-{i}");
        let val = format!("val-{i}");
        ks.insert(key.as_bytes(), val.as_bytes()).expect("insert");
    }

    for i in 0..100u32 {
        let key = format!("key-{i}");
        let val = format!("val-{i}");
        let stored = ks.get(key.as_bytes()).expect("get").expect("value");
        assert_eq!(stored.as_ref(), val.as_bytes(), "mismatch at {i}");
    }
}

#[test]
fn fjall_binary_key_values_roundtrip() {
    let (_dir, _db, ks) = open_partition("test_binary");

    let key: [u8; 24] = [0xFF; 24];
    let value: [u8; 128] = {
        let mut v = [0u8; 128];
        v.iter_mut().enumerate().for_each(|(i, b)| *b = (i as u8).wrapping_mul(7));
        v
    };

    ks.insert(key, value).expect("insert");
    let retrieved = ks.get(&key).expect("get").expect("value");
    assert_eq!(retrieved.as_ref(), &value);
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 2: Event Store — append/read on Fjall keyspace
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn event_key_encode_decode_roundtrip() {
    let id = make_instance_id();
    let seq = vo_types::SequenceNumber::try_from(42u64).unwrap();

    let encoded = encode_event_key(&id, &seq).expect("encode");
    assert_eq!(encoded.len(), 24);

    let (decoded_id, decoded_seq) = vo_storage::codec::decode_event_key(&encoded).expect("decode");
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn event_key_ordering_preserves_instance_then_sequence() {
    let id = make_instance_id();

    let key1 = encode_event_seq(&id, 1);
    let key2 = encode_event_seq(&id, 2);

    assert!(key1 < key2, "lower sequence should sort first");
}

#[test]
fn events_persist_on_fjall_keyspace() {
    let (_dir, _db, ks) = open_partition("events");

    let id = make_instance_id();
    let event = make_envelope(&id, 1);
    let event_bytes = serde_json::to_vec(&event).expect("serialize");
    let key = encode_event_seq(&id, 1);

    ks.insert(key, &event_bytes).expect("insert");

    let stored = ks
        .get(&key)
        .expect("get")
        .expect("event should exist");
    let restored: EventEnvelope = serde_json::from_slice(&stored).expect("deserialize");
    assert_eq!(restored.sequence, 1);
    assert_eq!(restored.instance_id, id.to_string());
}

#[test]
fn multiple_events_for_same_instance_persist() {
    let (_dir, _db, ks) = open_partition("events_multi");

    let id = make_instance_id();
    for seq in 1..=10u64 {
        let event = make_envelope(&id, seq);
        let event_bytes = serde_json::to_vec(&event).unwrap();
        let key = encode_event_seq(&id, seq);
        ks.insert(key, &event_bytes).unwrap();
    }

    let id_bytes = id.to_bytes().unwrap();
    let mut count = 0u64;
    for item in ks.prefix(&id_bytes) {
        let (k, v) = item.into_inner().expect("item");
        let (_, seq) = vo_storage::codec::decode_event_key(&k).expect("decode");
        let env: EventEnvelope = serde_json::from_slice(&v).expect("deserialize");
        assert_eq!(env.sequence, seq.as_u64());
        count += 1;
    }
    assert_eq!(count, 10);
}

#[test]
fn events_from_different_instances_dont_interfere() {
    let (_dir, _db, ks) = open_partition("events_multi_inst");

    let id1 = InstanceId::from_bytes([0x01; 16]);
    let id2 = InstanceId::from_bytes([0x02; 16]);

    let event1 = make_envelope(&id1, 1);
    let key1 = encode_event_seq(&id1, 1);
    ks.insert(key1, &serde_json::to_vec(&event1).unwrap()).unwrap();

    let event2 = make_envelope(&id2, 1);
    let key2 = encode_event_seq(&id2, 1);
    ks.insert(key2, &serde_json::to_vec(&event2).unwrap()).unwrap();

    let stored1 = ks.get(&encode_event_seq(&id1, 1)).unwrap().unwrap();
    let env1: EventEnvelope = serde_json::from_slice(&stored1).unwrap();
    assert_eq!(env1.instance_id, id1.to_string());

    let stored2 = ks.get(&encode_event_seq(&id2, 1)).unwrap().unwrap();
    let env2: EventEnvelope = serde_json::from_slice(&stored2).unwrap();
    assert_eq!(env2.instance_id, id2.to_string());
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 3: Snapshots — save/load/compact
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn snapshot_write_then_load_latest() {
    let (_dir, _db, ks) = open_partition("snapshots");

    let id = make_instance_id();
    let state = make_instance_state(42);

    snapshot_write(&ks, id.clone(), 5, &state).expect("write");

    let loaded = snapshot_load_latest(&ks, &id).expect("load");
    let (seq, loaded_state) = loaded.expect("snapshot should exist");
    assert_eq!(seq, 5);
    assert_eq!(loaded_state.counter, 42);
}

#[test]
fn snapshot_load_latest_returns_none_when_empty() {
    let (_dir, _db, ks) = open_partition("snapshots_empty");

    let id = make_instance_id();
    let loaded = snapshot_load_latest(&ks, &id).expect("load");
    assert!(loaded.is_none());
}

#[test]
fn snapshot_load_latest_returns_highest_sequence() {
    let (_dir, _db, ks) = open_partition("snapshots_multi");

    let id = make_instance_id();
    snapshot_write(&ks, id.clone(), 1, &make_instance_state(10)).unwrap();
    snapshot_write(&ks, id.clone(), 5, &make_instance_state(50)).unwrap();
    snapshot_write(&ks, id.clone(), 3, &make_instance_state(30)).unwrap();

    let (seq, state) = snapshot_load_latest(&ks, &id).unwrap().unwrap();
    assert_eq!(seq, 5);
    assert_eq!(state.counter, 50);
}

#[test]
fn snapshot_different_instances_dont_interfere() {
    let (_dir, _db, ks) = open_partition("snapshots_multi_inst");

    let id1 = InstanceId::from_bytes([0x01; 16]);
    let id2 = InstanceId::from_bytes([0x02; 16]);

    snapshot_write(&ks, id1.clone(), 10, &make_instance_state(100)).unwrap();
    snapshot_write(&ks, id2.clone(), 20, &make_instance_state(200)).unwrap();

    let (seq1, state1) = snapshot_load_latest(&ks, &id1).unwrap().unwrap();
    assert_eq!(seq1, 10);
    assert_eq!(state1.counter, 100);

    let (seq2, state2) = snapshot_load_latest(&ks, &id2).unwrap().unwrap();
    assert_eq!(seq2, 20);
    assert_eq!(state2.counter, 200);
}

#[test]
fn snapshot_compact_keeps_last_n() {
    let (_dir, _db, ks) = open_partition("snapshots_compact");

    let id = make_instance_id();
    for seq in 1..=5u64 {
        snapshot_write(&ks, id.clone(), seq, &make_instance_state(seq * 10)).unwrap();
    }

    let deleted = compact_snapshots(&ks, &id, 2).expect("compact");
    assert_eq!(deleted, 3);

    let (seq, state) = snapshot_load_latest(&ks, &id).unwrap().unwrap();
    assert_eq!(seq, 5);
    assert_eq!(state.counter, 50);
}

#[test]
fn snapshot_compact_no_op_when_under_limit() {
    let (_dir, _db, ks) = open_partition("snapshots_compact_noop");

    let id = make_instance_id();
    snapshot_write(&ks, id.clone(), 1, &make_instance_state(10)).unwrap();

    let deleted = compact_snapshots(&ks, &id, 5).expect("compact");
    assert_eq!(deleted, 0);
}

#[test]
fn snapshot_encode_decode_key_roundtrip() {
    let id = make_instance_id();
    let seq = 99u64;

    let key = encode_snapshot_key(&id, seq).expect("encode");
    assert_eq!(key.len(), 24);

    let (decoded_id, decoded_seq) = vo_storage::snapshots::decode_snapshot_key(&key).expect("decode");
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn atomic_snapshot_writer_write_and_load() {
    let (_dir, db) = open_fjall();
    let writer = AtomicSnapshotWriter::new(&db).expect("writer");

    let id = make_instance_id();
    let state = make_instance_state(77);

    writer
        .write_snapshot_atomic(id.clone(), 10, &state)
        .expect("atomic write");

    let ks = db
        .keyspace("snapshots", || fjall::KeyspaceCreateOptions::default())
        .expect("open snapshots");
    let (seq, loaded_state) = snapshot_load_latest(&ks, &id).unwrap().unwrap();
    assert_eq!(seq, 10);
    assert_eq!(loaded_state.counter, 77);
}

#[test]
fn snapshot_policy_every_n_events() {
    let policy = SnapshotPolicy::EveryNEvents(5);

    assert!(!policy.should_snapshot(0));
    assert!(!policy.should_snapshot(4));
    assert!(policy.should_snapshot(5));
    assert!(!policy.should_snapshot(6));
    assert!(policy.should_snapshot(10));
}

#[test]
fn snapshot_policy_disabled_never_triggers() {
    let policy = SnapshotPolicy::Disabled;

    assert!(!policy.should_snapshot(5));
    assert!(!policy.should_snapshot(100));
    assert!(!policy.should_snapshot(1_000_000));
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 4: Blob Storage — write/read/large blobs via FsBlobStore
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_write_read_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"hello, veloxide storage!";
    let addr = store.store(data).expect("store");

    let retrieved = store.retrieve(&addr).expect("retrieve");
    assert_eq!(retrieved, data);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_retrieve_missing_returns_content_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let missing = ContentAddress::new(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    )
    .unwrap();

    let result = store.retrieve(&missing);
    assert!(matches!(result, Err(BlobStoreError::ContentNotFound { .. })));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_duplicate_content_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"duplicate me";
    store.store(data).expect("first store");

    let result = store.store(data);
    assert!(matches!(result, Err(BlobStoreError::DuplicateContent { .. })));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_contains_works() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let addr = store.store(b"check me").expect("store");
    assert!(store.contains(&addr).expect("contains"));

    let missing = ContentAddress::new(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    assert!(!store.contains(&missing).expect("contains missing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_content_address_is_sha256() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"predictable hash input";
    let addr = store.store(data).expect("store");

    assert_eq!(addr.as_str().len(), 64);
    assert!(addr.as_str().chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_ref_count_increment_decrement() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let addr = store.store(b"refcount").expect("store");

    let count = store.increment_ref_count(&addr).expect("increment");
    assert_eq!(count, 2);

    let meta = store.get_metadata(&addr).expect("metadata");
    assert_eq!(meta.reference_count(), 2);

    let count = store.decrement_ref_count(&addr).expect("decrement");
    assert_eq!(count, 1);

    let meta = store.get_metadata(&addr).expect("metadata");
    assert_eq!(meta.reference_count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_get_metadata_returns_correct_record() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"metadata test data here";
    let addr = store.store(data).expect("store");

    let meta = store.get_metadata(&addr).expect("metadata");
    assert_eq!(meta.size_bytes(), data.len() as u64);
    assert_eq!(meta.reference_count(), 1);
    assert_eq!(meta.status(), vo_types::BlobStatus::DurablyStored);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_large_blob_1mb_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data: Vec<u8> = (0..1_048_576)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(17))
        .collect();

    let addr = store.store(&data).expect("store 1MB");
    let retrieved = store.retrieve(&addr).expect("retrieve 1MB");
    assert_eq!(retrieved.len(), data.len());
    assert_eq!(retrieved, data);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_large_blob_4mb_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data: Vec<u8> = (0..4_194_304)
        .map(|i| (i as u8).wrapping_mul(53).wrapping_add(7))
        .collect();

    let addr = store.store(&data).expect("store 4MB");
    let retrieved = store.retrieve(&addr).expect("retrieve 4MB");
    assert_eq!(retrieved.len(), 4_194_304);
    assert_eq!(retrieved, data);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_gc_collects_expired_zero_ref_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let addr = store.store(b"gc me please").expect("store");
    store.decrement_ref_count(&addr).expect("decrement to 0");

    let meta = store.get_metadata(&addr).expect("meta");
    let expired_record = BlobRecord::with_status(
        meta.content_addr().clone(),
        meta.size_bytes(),
        0,
        meta.created_at_ms(),
        Some(1),
        meta.status(),
    );
    let meta_path = dir.path().join("meta").join(format!("{}.json", addr.as_str()));
    let encoded = vo_storage::blob_store::encode_blob_record(&expired_record).expect("encode");
    tokio::fs::write(&meta_path, &encoded).await.expect("write meta");

    let collected = store.run_gc(u64::MAX).expect("gc");
    assert_eq!(collected, 1);

    assert!(!store.contains(&addr).expect("should be deleted"));
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_gc_does_not_collect_active_blobs() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    store.store(b"keep me alive").expect("store");

    let collected = store.run_gc(u64::MAX).expect("gc");
    assert_eq!(collected, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn blob_store_streaming_store_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsBlobStore::new(dir.path());

    let data = b"streaming blob content test";
    let cursor = std::io::Cursor::new(data.to_vec());
    let reader = tokio::io::BufReader::new(cursor);

    let addr = store.store_streaming(reader).expect("store streaming");
    let retrieved = store.retrieve(&addr).expect("retrieve");
    assert_eq!(retrieved, data);
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 5: Partition Layout
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn create_partition_layout_opens_fjall_database() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = create_partition_layout(dir.path()).expect("layout");
    assert!(dir.path().exists());
    let _db = layout.db();
}

#[test]
fn open_all_partitions_opens_every_defined_partition() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = create_partition_layout(dir.path()).expect("layout");

    let partitions = open_all_partitions(&layout).expect("open all");
    assert_eq!(partitions.len(), ALL_PARTITIONS.len());

    let names: Vec<&str> = partitions.iter().map(|(n, _)| *n).collect();
    for expected in ALL_PARTITIONS {
        assert!(names.contains(expected), "missing partition: {expected}");
    }
}

#[test]
fn partition_class_counts_match_constants() {
    let hot = HOT_PARTITIONS.len();
    let cold = COLD_PARTITIONS.len();
    let blob = BLOB_PARTITIONS.len();
    assert_eq!(hot + cold + blob + 1, ALL_PARTITIONS.len());
}

#[test]
fn storage_engine_opens_with_all_stores() {
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = vo_storage::partitions::StorageEngine::open(dir.path()).expect("engine open");
    let _db = engine.db();
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 6: ContentAddress validation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn content_address_rejects_wrong_length() {
    let result = ContentAddress::new("too_short");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), BlobStoreError::InvalidArgument { .. }));
}

#[test]
fn content_address_rejects_uppercase_hex() {
    let result = ContentAddress::new("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    assert!(result.is_err());
}

#[test]
fn content_address_rejects_non_hex_chars() {
    let result = ContentAddress::new("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
    assert!(result.is_err());
}

#[test]
fn content_address_accepts_valid_sha256_hex() {
    let result = ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert!(result.is_ok());
}

#[test]
fn content_address_from_bytes_roundtrip() {
    let bytes = [0xABu8; 32];
    let addr = ContentAddress::from_bytes(&bytes);
    assert_eq!(addr.as_str().len(), 64);

    let recovered = addr.as_bytes();
    assert_eq!(recovered, bytes);
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 7: BlobRecord invariants
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn blob_record_rejects_zero_ref_count() {
    let addr = ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
    let result = BlobRecord::new(addr.clone(), 100, 0, 1000, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_rejects_zero_created_at() {
    let addr = ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
    let result = BlobRecord::new(addr.clone(), 100, 1, 0, None);
    assert!(result.is_err());
}

#[test]
fn blob_record_gc_eligible_when_expired_and_zero_refs() {
    let addr = ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
    let record = BlobRecord::with_status(
        addr,
        100,
        0,
        1000,
        Some(2000),
        vo_types::BlobStatus::DurablyStored,
    );

    assert!(record.is_expired(2000));
    assert!(record.is_gc_eligible(2000));
    assert!(!record.is_gc_eligible(1999));
}

#[test]
fn blob_record_not_gc_eligible_with_refs() {
    let addr = ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
    let record = BlobRecord::with_status(
        addr,
        100,
        1,
        1000,
        Some(2000),
        vo_types::BlobStatus::DurablyStored,
    );

    assert!(record.is_expired(2000));
    assert!(!record.is_gc_eligible(2000));
}

#[test]
fn blob_record_saturating_ref_count_ops() {
    let addr = ContentAddress::new("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
    let record = BlobRecord::with_status(addr, 100, 1, 1000, None, vo_types::BlobStatus::DurablyStored);

    assert_eq!(record.increment_ref_count(), 2);
    assert_eq!(record.decrement_ref_count(), 0);
    assert_eq!(record.decrement_ref_count(), 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 8: Batch writes
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn batch_write_commits_multiple_keys_atomically() {
    let (_dir, db) = open_fjall();
    let ks = db
        .keyspace("batch_test", || fjall::KeyspaceCreateOptions::default())
        .expect("partition");

    let mut batch = db.batch();
    batch.insert(&ks, b"key-a", b"val-a");
    batch.insert(&ks, b"key-b", b"val-b");
    batch.insert(&ks, b"key-c", b"val-c");
    batch.commit().expect("commit");

    assert_eq!(ks.get(b"key-a").expect("get").expect("a").as_ref(), b"val-a");
    assert_eq!(ks.get(b"key-b").expect("get").expect("b").as_ref(), b"val-b");
    assert_eq!(ks.get(b"key-c").expect("get").expect("c").as_ref(), b"val-c");
}

#[test]
fn batch_write_with_delete_commits_both() {
    let (_dir, db) = open_fjall();
    let ks = db
        .keyspace("batch_del", || fjall::KeyspaceCreateOptions::default())
        .expect("partition");

    ks.insert(b"old", b"will-be-deleted").expect("insert");
    assert!(ks.get(b"old").expect("get").is_some());

    let mut batch = db.batch();
    batch.remove(&ks, b"old");
    batch.insert(&ks, b"new", b"fresh");
    batch.commit().expect("commit");

    assert!(ks.get(b"old").expect("get").is_none());
    assert_eq!(ks.get(b"new").expect("get").expect("new").as_ref(), b"fresh");
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 9: Prefix scans
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn prefix_scan_returns_matching_keys() {
    let (_dir, _db, ks) = open_partition("prefix_scan");

    ks.insert(b"usr:1:name", b"Alice").unwrap();
    ks.insert(b"usr:1:email", b"alice@example.com").unwrap();
    ks.insert(b"usr:2:name", b"Bob").unwrap();
    ks.insert(b"other:key", b"val").unwrap();

    let mut results: Vec<Vec<u8>> = Vec::new();
    for item in ks.prefix(b"usr:1:") {
        let (_, v) = item.into_inner().expect("item");
        results.push(v.to_vec());
    }

    assert_eq!(results.len(), 2);
}

#[test]
fn prefix_scan_returns_empty_for_no_matches() {
    let (_dir, _db, ks) = open_partition("prefix_empty");

    ks.insert(b"aaa", b"1").unwrap();
    ks.insert(b"bbb", b"2").unwrap();

    let count = ks.prefix(b"zzz").count();
    assert_eq!(count, 0);
}
