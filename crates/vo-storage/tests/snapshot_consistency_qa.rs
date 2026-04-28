//! QA: Snapshot creation consistency verification (ve-qqqfq)

#![allow(clippy::unwrap_used)]

use tempfile::tempdir;
use vo_storage::snapshots::{
    snapshot_load_latest_with_compat, AtomicSnapshotWriter, CompatSnapshotLoad,
    CURRENT_SNAPSHOT_VERSION,
};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn make_id() -> InstanceId {
    InstanceId::from_bytes([1u8; 16])
}

fn open_db(dir: &tempfile::TempDir) -> fjall::Database {
    fjall::Database::builder(dir.path()).open().unwrap()
}

#[test]
fn atomic_snapshot_write_then_readback_preserves_state() {
    let dir = tempdir().unwrap();
    let db = open_db(&dir);
    let id = make_id();
    let original = InstanceState { counter: 99 };

    // Write via AtomicSnapshotWriter (header format)
    let writer = AtomicSnapshotWriter::new(&db).unwrap();
    writer
        .write_snapshot_atomic(id.clone(), 10, &original)
        .unwrap();

    // Read back via compat loader
    let partition = db
        .keyspace("snapshots", || fjall::KeyspaceCreateOptions::default())
        .unwrap();
    let loaded = snapshot_load_latest_with_compat(
        &partition,
        &id,
        CURRENT_SNAPSHOT_VERSION,
        CURRENT_SNAPSHOT_VERSION,
    )
    .unwrap();

    match loaded {
        Some(CompatSnapshotLoad::Loaded { sequence, state }) => {
            assert_eq!(sequence, 10);
            assert_eq!(state, original);
        }
        Some(CompatSnapshotLoad::Discarded { sequence, reason }) => {
            panic!("snapshot at seq {sequence} was discarded: {reason:?}");
        }
        None => panic!("expected snapshot to be loaded, got None"),
    }
}

#[test]
fn atomic_snapshot_survives_database_restart() {
    let dir = tempdir().unwrap();
    let id = make_id();
    let original = InstanceState { counter: 12345 };

    // Phase 1: write snapshot
    {
        let db = open_db(&dir);
        let writer = AtomicSnapshotWriter::new(&db).unwrap();
        writer
            .write_snapshot_atomic(id.clone(), 42, &original)
            .unwrap();
    }

    // Phase 2: reopen database and read back
    {
        let db = open_db(&dir);
        let partition = db
            .keyspace("snapshots", || fjall::KeyspaceCreateOptions::default())
            .unwrap();
        let loaded = snapshot_load_latest_with_compat(
            &partition,
            &id,
            CURRENT_SNAPSHOT_VERSION,
            CURRENT_SNAPSHOT_VERSION,
        )
        .unwrap();
        match loaded {
            Some(CompatSnapshotLoad::Loaded { sequence, state }) => {
                assert_eq!(sequence, 42);
                assert_eq!(state, original);
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }
}
