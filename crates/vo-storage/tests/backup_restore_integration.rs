#![allow(clippy::unwrap_used)]

use fjall::{Database, Keyspace, KeyspaceCreateOptions};
use tempfile::tempdir;
use vo_storage::snapshots::{
    compact_snapshots, get_all_snapshot_sequences, snapshot_load_latest, snapshot_write,
    AtomicSnapshotWriter,
};
use vo_types::state::InstanceState;
use vo_types::InstanceId;

fn get_typical_id() -> InstanceId {
    InstanceId::from_bytes([1; 16])
}

fn get_other_id() -> InstanceId {
    InstanceId::from_bytes([2; 16])
}

fn setup_fjall() -> (tempfile::TempDir, fjall::Database, fjall::Keyspace) {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = Database::builder(temp_dir.path()).open().unwrap();
    let partition = db
        .keyspace("snapshots", || KeyspaceCreateOptions::default())
        .unwrap();
    (temp_dir, db, partition)
}

// ---------------------------------------------------------------------------
// Backup Tests
// ---------------------------------------------------------------------------

#[test]
fn backup_single_snapshot_round_trip() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();
    let state = InstanceState { counter: 42 };

    snapshot_write(&partition, id.clone(), 1, &state).unwrap();

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, Some((1, state)));
}

#[test]
fn backup_multiple_snapshots_loads_latest() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    snapshot_write(&partition, id.clone(), 1, &InstanceState { counter: 10 }).unwrap();
    snapshot_write(&partition, id.clone(), 2, &InstanceState { counter: 20 }).unwrap();
    snapshot_write(&partition, id.clone(), 3, &InstanceState { counter: 30 }).unwrap();

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, Some((3, InstanceState { counter: 30 })));
}

#[test]
fn backup_atomic_snapshot_with_checksum() {
    let (_dir, keyspace, partition) = setup_fjall();
    let id = get_typical_id();
    let state = InstanceState { counter: 99 };

    let writer = AtomicSnapshotWriter::new(&keyspace).unwrap();
    writer.write_snapshot_atomic(id.clone(), 1, &state).unwrap();

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, Some((1, state)));
}

#[test]
fn backup_detects_checksum_corruption() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    let state = InstanceState { counter: 42 };
    let state_json = serde_json::to_vec(&state).unwrap();
    let wrong_checksum = 0u32;
    let header = vo_storage::snapshots::SnapshotHeader::new(id.clone(), 1, wrong_checksum);
    let header_json = serde_json::to_vec(&header).unwrap();

    let mut value = header_json;
    value.push(b'|');
    value.extend_from_slice(&state_json);

    let key = vo_storage::snapshots::encode_snapshot_key(&id, 1).unwrap();
    partition.insert(key, &value).unwrap();

    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(
        result,
        Err(vo_storage::codec::StorageError::DeserializationFailed)
    );
}

#[test]
fn backup_isolated_between_instances() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id1 = get_typical_id();
    let id2 = get_other_id();

    snapshot_write(&partition, id1.clone(), 1, &InstanceState { counter: 100 }).unwrap();
    snapshot_write(&partition, id2.clone(), 1, &InstanceState { counter: 200 }).unwrap();

    let result1 = snapshot_load_latest(&partition, &id1).unwrap();
    let result2 = snapshot_load_latest(&partition, &id2).unwrap();

    assert_eq!(result1, Some((1, InstanceState { counter: 100 })));
    assert_eq!(result2, Some((1, InstanceState { counter: 200 })));
}

#[test]
fn backup_large_counter_value() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();
    let state = InstanceState { counter: u64::MAX };

    snapshot_write(&partition, id.clone(), 1, &state).unwrap();

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, Some((1, state)));
}

#[test]
fn backup_sequence_number_boundary() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    snapshot_write(&partition, id.clone(), 0, &InstanceState { counter: 0 }).unwrap();
    snapshot_write(
        &partition,
        id.clone(),
        u64::MAX,
        &InstanceState { counter: u64::MAX },
    )
    .unwrap();

    let result_min = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(
        result_min,
        Some((u64::MAX, InstanceState { counter: u64::MAX }))
    );
}

// ---------------------------------------------------------------------------
// Restore Tests
// ---------------------------------------------------------------------------

#[test]
fn restore_from_backup_after_crash() {
    let dir = tempdir().unwrap();
    let id = get_typical_id();

    {
        let db = Database::builder(dir.path()).open().unwrap();
        let partition = db
            .keyspace("snapshots", || KeyspaceCreateOptions::default())
            .unwrap();
        snapshot_write(&partition, id.clone(), 50, &InstanceState { counter: 1234 }).unwrap();
    }

    {
        let keyspace = Database::builder(dir.path()).open().unwrap();
        let partition = keyspace
            .keyspace("snapshots", || KeyspaceCreateOptions::default())
            .unwrap();
        let result = snapshot_load_latest(&partition, &id).unwrap();
        assert_eq!(result, Some((50, InstanceState { counter: 1234 })));
    }
}

#[test]
fn restore_selects_latest_sequence() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    for i in 1..=100 {
        snapshot_write(
            &partition,
            id.clone(),
            i,
            &InstanceState {
                counter: i as u64 * 10,
            },
        )
        .unwrap();
    }

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, Some((100, InstanceState { counter: 1000 })));
}

#[test]
fn restore_returns_none_for_empty_partition() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, None);
}

// ---------------------------------------------------------------------------
// Backup + Restore Integration Tests
// ---------------------------------------------------------------------------

#[test]
fn full_backup_restore_workflow() {
    let dir = tempdir().unwrap();
    let id = get_typical_id();

    let backup_sequence;
    let backup_state;
    {
        let keyspace = Database::builder(dir.path()).open().unwrap();
        let partition = keyspace
            .keyspace("snapshots", || KeyspaceCreateOptions::default())
            .unwrap();

        for i in 1..=50 {
            snapshot_write(&partition, id.clone(), i, &InstanceState { counter: i * 2 }).unwrap();
        }

        backup_sequence = 50;
        backup_state = InstanceState { counter: 100 };

        keyspace.persist(fjall::PersistMode::SyncAll).unwrap();
    }

    {
        let keyspace = Database::builder(dir.path()).open().unwrap();
        let partition = keyspace
            .keyspace("snapshots", || KeyspaceCreateOptions::default())
            .unwrap();

        let result = snapshot_load_latest(&partition, &id).unwrap().unwrap();
        assert_eq!(result.0, backup_sequence);
        assert_eq!(result.1, backup_state);
    }
}

#[test]
fn backup_restore_with_compaction() {
    let dir = tempdir().unwrap();
    let keyspace = Database::builder(dir.path()).open().unwrap();
    let partition = keyspace
        .keyspace("snapshots", || KeyspaceCreateOptions::default())
        .unwrap();
    let id = get_typical_id();

    for i in 1..=10 {
        snapshot_write(&partition, id.clone(), i, &InstanceState { counter: i }).unwrap();
    }

    keyspace.persist(fjall::PersistMode::SyncAll).unwrap();
    partition.major_compact().unwrap();

    let result = snapshot_load_latest(&partition, &id).unwrap();
    assert_eq!(result, Some((10, InstanceState { counter: 10 })));
}

#[test]
fn backup_compaction_preserves_latest_snapshots() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    for i in 1..=20 {
        snapshot_write(&partition, id.clone(), i, &InstanceState { counter: i }).unwrap();
    }

    compact_snapshots(&partition, &id, 5).unwrap();

    let sequences = get_all_snapshot_sequences(&partition, &id).unwrap();
    assert_eq!(sequences.len(), 5);
    assert_eq!(sequences[0], 20);
}

#[test]
fn backup_restore_multiple_instances_independent() {
    let dir = tempdir().unwrap();
    let id1 = get_typical_id();
    let id2 = get_other_id();

    {
        let keyspace = Database::builder(dir.path()).open().unwrap();
        let partition = keyspace
            .keyspace("snapshots", || KeyspaceCreateOptions::default())
            .unwrap();

        snapshot_write(&partition, id1.clone(), 10, &InstanceState { counter: 111 }).unwrap();
        snapshot_write(&partition, id2.clone(), 20, &InstanceState { counter: 222 }).unwrap();

        keyspace.persist(fjall::PersistMode::SyncAll).unwrap();
    }

    {
        let keyspace = Database::builder(dir.path()).open().unwrap();
        let partition = keyspace
            .keyspace("snapshots", || KeyspaceCreateOptions::default())
            .unwrap();

        let result1 = snapshot_load_latest(&partition, &id1).unwrap();
        let result2 = snapshot_load_latest(&partition, &id2).unwrap();

        assert_eq!(result1, Some((10, InstanceState { counter: 111 })));
        assert_eq!(result2, Some((20, InstanceState { counter: 222 })));
    }
}

#[test]
fn get_all_snapshot_sequences_returns_sorted_descending() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    snapshot_write(&partition, id.clone(), 5, &InstanceState { counter: 5 }).unwrap();
    snapshot_write(&partition, id.clone(), 1, &InstanceState { counter: 1 }).unwrap();
    snapshot_write(&partition, id.clone(), 10, &InstanceState { counter: 10 }).unwrap();
    snapshot_write(&partition, id.clone(), 3, &InstanceState { counter: 3 }).unwrap();

    let sequences = get_all_snapshot_sequences(&partition, &id).unwrap();
    assert_eq!(sequences, vec![10, 5, 3, 1]);
}

#[test]
fn get_all_snapshot_sequences_empty_for_no_snapshots() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    let sequences = get_all_snapshot_sequences(&partition, &id).unwrap();
    assert_eq!(sequences, Vec::<u64>::new());
}

#[test]
fn get_all_snapshot_sequences_isolated_to_instance() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id1 = get_typical_id();
    let id2 = get_other_id();

    snapshot_write(&partition, id1.clone(), 1, &InstanceState { counter: 1 }).unwrap();
    snapshot_write(&partition, id1.clone(), 2, &InstanceState { counter: 2 }).unwrap();
    snapshot_write(&partition, id2.clone(), 10, &InstanceState { counter: 10 }).unwrap();

    let sequences1 = get_all_snapshot_sequences(&partition, &id1).unwrap();
    let sequences2 = get_all_snapshot_sequences(&partition, &id2).unwrap();

    assert_eq!(sequences1, vec![2, 1]);
    assert_eq!(sequences2, vec![10]);
}

#[test]
fn compact_snapshots_deletes_old_ones() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    for i in 1..=10 {
        snapshot_write(&partition, id.clone(), i, &InstanceState { counter: i }).unwrap();
    }

    let deleted = compact_snapshots(&partition, &id, 3).unwrap();
    assert_eq!(deleted, 7);

    let sequences = get_all_snapshot_sequences(&partition, &id).unwrap();
    assert_eq!(sequences, vec![10, 9, 8]);
}

#[test]
fn compact_snapshots_no_op_when_keep_last_n_greater_than_count() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    snapshot_write(&partition, id.clone(), 1, &InstanceState { counter: 1 }).unwrap();
    snapshot_write(&partition, id.clone(), 2, &InstanceState { counter: 2 }).unwrap();

    let deleted = compact_snapshots(&partition, &id, 10).unwrap();
    assert_eq!(deleted, 0);

    let sequences = get_all_snapshot_sequences(&partition, &id).unwrap();
    assert_eq!(sequences, vec![2, 1]);
}
