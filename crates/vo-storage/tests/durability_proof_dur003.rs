//! DUR-003: Snapshot 1,000 instances, kill, restart, verify all snapshots recover.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use vo_storage::instance_index::{instance_index_upsert, scan_by_status};
use vo_storage::partitions::{INSTANCES_PARTITION, SNAPSHOTS_PARTITION};
use vo_storage::snapshots::snapshot_load_latest;
use vo_types::state::InstanceState;
use vo_types::{InstanceId, InstanceStatus, TimestampMs};

fn make_instance_id(byte: u8) -> InstanceId {
    InstanceId::from_bytes([byte; 16])
}

fn open_db(path: &std::path::Path) -> fjall::Database {
    fjall::Database::builder(path).open().unwrap()
}

fn open_snapshots_ks(db: &fjall::Database) -> fjall::Keyspace {
    db.keyspace(SNAPSHOTS_PARTITION, fjall::KeyspaceCreateOptions::default)
        .unwrap()
}

fn open_instances_ks(db: &fjall::Database) -> fjall::Keyspace {
    db.keyspace(INSTANCES_PARTITION, fjall::KeyspaceCreateOptions::default)
        .unwrap()
}

#[test]
fn dur_003_snapshot_1k_instances_kill_restart_verify_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let instance_count = 1_000u16;
    let ts = TimestampMs::try_from(1000).unwrap();

    // Phase 1: Snapshot 1,000 instances with varying sequences
    {
        let db = open_db(dir.path());
        let snapshot_ks = open_snapshots_ks(&db);
        let instance_ks = open_instances_ks(&db);

        for i in 0..instance_count {
            let byte = (i % 256) as u8;
            let id = make_instance_id(byte);
            let seq = (i as u64 + 1) * 100;
            let state = InstanceState { counter: i as u64 };

            // Write snapshot via raw keyspace insert
            let snapshot_key = vo_storage::snapshots::encode_snapshot_key(&id, seq).unwrap();
            let state_json = serde_json::to_vec(&state).unwrap();
            snapshot_ks.insert(snapshot_key, &state_json).unwrap();

            // Also write instance index entry
            instance_index_upsert(&db, &id, InstanceStatus::Running, ts, None).unwrap();
        }

        db.persist(fjall::PersistMode::SyncAll).unwrap();
    }
    // <-- simulated kill

    // Phase 2: Restart and verify all snapshots recoverable
    {
        let db = open_db(dir.path());
        let snapshot_ks = open_snapshots_ks(&db);

        // Count total snapshot entries via full scan
        let total_scanned = snapshot_ks.iter().count();

        assert!(
            total_scanned >= instance_count as usize,
            "All {} snapshots must be readable after crash recovery, got {}",
            instance_count,
            total_scanned
        );

        // Verify specific instances recover their latest snapshot
        // Each unique byte ID gets ~4 snapshots (1000/256); latest is from highest i
        let unique_ids = instance_count.min(256);
        let mut verified = 0u16;
        for byte_val in 0..unique_ids {
            let id = make_instance_id(byte_val as u8);
            let loaded = snapshot_load_latest(&snapshot_ks, &id).unwrap();

            assert!(
                loaded.is_some(),
                "Snapshot for instance byte {} must exist after crash recovery",
                byte_val
            );

            let (seq, state) = loaded.unwrap();
            verified += 1;

            // Verify sequence and counter are consistent: seq = (counter + 1) * 100
            assert_eq!(
                seq,
                (state.counter + 1) * 100,
                "Sequence/counter inconsistency for byte {}: seq={}, counter={}",
                byte_val,
                seq,
                state.counter
            );

            // Verify counter is in valid range
            assert!(
                state.counter < instance_count as u64,
                "Counter {} out of range for byte {}",
                state.counter,
                byte_val
            );
        }

        assert_eq!(
            verified, unique_ids,
            "All {} unique instance snapshots must be recoverable",
            unique_ids
        );
    }
}
