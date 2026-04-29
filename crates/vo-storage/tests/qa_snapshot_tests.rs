//! QA tests for vo-storage: Snapshots.
//!
//! All tests use real Fjall instances in temp directories. No mocks.

use vo_storage::snapshots::{
    compact_snapshots, encode_snapshot_key, snapshot_load_latest, snapshot_write,
    AtomicSnapshotWriter,
};
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

    let (decoded_id, decoded_seq) =
        vo_storage::snapshots::decode_snapshot_key(&key).expect("decode");
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
    let policy = vo_storage::snapshots::SnapshotPolicy::EveryNEvents(5);

    assert!(!policy.should_snapshot(0));
    assert!(!policy.should_snapshot(4));
    assert!(policy.should_snapshot(5));
    assert!(!policy.should_snapshot(6));
    assert!(policy.should_snapshot(10));
}

#[test]
fn snapshot_policy_disabled_never_triggers() {
    let policy = vo_storage::snapshots::SnapshotPolicy::Disabled;

    assert!(!policy.should_snapshot(5));
    assert!(!policy.should_snapshot(100));
    assert!(!policy.should_snapshot(1_000_000));
}
