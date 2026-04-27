//! RED-QUEEN coevolutionary storage tests (ve-0v3jz)
//!
//! Adversarial tests for:
//! - Fjall partition recovery after close/reopen
//! - Compaction under load (high write volume)
//! - Snapshot conflict detection and resolution

#![allow(clippy::unwrap_used)]

use tempfile::tempdir;
use vo_storage::partitions::{create_partition_layout, open_all_partitions};
use vo_storage::snapshots::{
    compact_snapshots, encode_snapshot_key, get_all_snapshot_sequences, snapshot_write,
    AtomicSnapshotWriter,
};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn iid(seed: u8) -> InstanceId {
    InstanceId::from_bytes([seed; 16])
}

#[test]
fn red_queen_partition_recovery_all_13_keyspaces_survive_reopen() {
    let dir = tempdir().unwrap();
    let layout = create_partition_layout(dir.path()).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    assert_eq!(partitions.len(), 13, "must open all 13 partitions");

    let snap = layout
        .db()
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let id = iid(0xAA);
    let key = encode_snapshot_key(&id, 42).unwrap();
    snap.insert(key, b"payload").unwrap();

    drop(snap);
    drop(partitions);
    drop(layout);

    let layout2 = create_partition_layout(dir.path()).unwrap();
    let snap2 = layout2
        .db()
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    assert_eq!(
        snap2.get(key).unwrap().map(|v| v.to_vec()),
        Some(b"payload".to_vec())
    );
}

#[test]
fn red_queen_partition_recovery_empty_db_opens_cleanly() {
    let dir = tempdir().unwrap();
    let layout = create_partition_layout(dir.path()).unwrap();
    let partitions = open_all_partitions(&layout).unwrap();
    for (name, ks) in &partitions {
        assert!(
            ks.iter().next().is_none(),
            "{name} should be empty on fresh open"
        );
    }
}

fn state(counter: u64) -> InstanceState {
    InstanceState { counter }
}

#[test]
fn red_queen_compaction_under_load_removes_old_snapshots() {
    let dir = tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let partition = db
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let id = iid(0xBB);

    for seq in 1..=50u64 {
        snapshot_write(&partition, id.clone(), seq, &state(seq)).unwrap();
    }

    let deleted = compact_snapshots(&partition, &id, 5).unwrap();
    assert_eq!(deleted, 45, "should delete all but last 5");

    let remaining = get_all_snapshot_sequences(&partition, &id).unwrap();
    assert_eq!(remaining.len(), 5);
    assert_eq!(remaining[0], 50, "highest sequence must survive");
}

#[test]
fn red_queen_compaction_under_load_concurrent_writers() {
    let dir = tempdir().unwrap();
    let db = std::sync::Arc::new(fjall::Database::builder(dir.path()).open().unwrap());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));

    let handles: Vec<_> = (0..4u8)
        .map(|seed| {
            let db = std::sync::Arc::clone(&db);
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                let ks = db
                    .keyspace("events", fjall::KeyspaceCreateOptions::default)
                    .unwrap();
                barrier.wait();
                for i in 0..200u32 {
                    let key = format!("rq-{seed}-{i}");
                    ks.insert(key.as_bytes(), seed.to_le_bytes()).unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let ks = db
        .keyspace("events", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    assert_eq!(ks.iter().count(), 800, "all 4×200 writes must survive");
}

#[test]
fn red_queen_snapshot_conflict_last_writer_wins() {
    let dir = tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let writer = AtomicSnapshotWriter::new(&db).unwrap();
    let id = iid(0xCC);

    writer
        .write_snapshot_atomic(id.clone(), 10, &state(10))
        .unwrap();
    writer.write_snapshot_atomic(id, 10, &state(999)).unwrap();

    let ks = db
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let key = encode_snapshot_key(&iid(0xCC), 10).unwrap();
    let val = ks.get(key).unwrap().unwrap();
    assert!(
        val.iter().any(|&b| b == b'|'),
        "must have header|payload format"
    );
}

#[test]
fn red_queen_snapshot_conflict_multi_instance_isolation() {
    let dir = tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let writer = AtomicSnapshotWriter::new(&db).unwrap();
    let id_a = iid(0xDD);
    let id_b = iid(0xEE);

    writer
        .write_snapshot_atomic(id_a.clone(), 1, &state(111))
        .unwrap();
    writer
        .write_snapshot_atomic(id_b.clone(), 1, &state(222))
        .unwrap();

    let ks = db
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let seqs_a = get_all_snapshot_sequences(&ks, &id_a).unwrap();
    let seqs_b = get_all_snapshot_sequences(&ks, &id_b).unwrap();
    assert_eq!(seqs_a, vec![1], "instance A has its own snapshot");
    assert_eq!(seqs_b, vec![1], "instance B has its own snapshot");
}

#[test]
fn red_queen_snapshot_conflict_compact_preserves_latest() {
    let dir = tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let ks = db
        .keyspace("snapshots", fjall::KeyspaceCreateOptions::default)
        .unwrap();
    let id = iid(0xFF);

    for seq in 1..=10u64 {
        snapshot_write(&ks, id.clone(), seq, &state(seq * 10)).unwrap();
    }
    compact_snapshots(&ks, &id, 1).unwrap();

    let remaining = get_all_snapshot_sequences(&ks, &id).unwrap();
    assert_eq!(remaining, vec![10], "only latest snapshot survives");
}
