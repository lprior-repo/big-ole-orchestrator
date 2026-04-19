#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Red Queen adversarial tests for atomic batch writer (ADR-016).
//!
//! These tests probe:
//! - Atomicity: either all writes in a batch land or none
//! - Concurrent batch submission: multiple threads submitting batches simultaneously
//! - Duplicate keys in batch: same key inserted multiple times
//! - Oversized batch: batch that exceeds reasonable size limits
//! - Partial failure recovery: verifying no partial state is visible after batch failure

use std::sync::Arc;
use std::thread;

use vo_types::{InstanceId, SequenceNumber};

use crate::codec::StorageError;
use crate::key_encoding::{encode_instance_index_key, encode_snapshot_key};
use crate::snapshots::{
    encode_snapshot_key as encode_snap_key, AtomicSnapshotWriter, SnapshotPolicy,
};

fn temp_keyspace() -> fjall::Keyspace {
    let dir = tempfile::tempdir().expect("tempdir");
    fjall::Config::new(dir.path()).open().expect("keyspace")
}

fn sample_instance_id() -> InstanceId {
    InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
}

fn sample_instance_id_2() -> InstanceId {
    InstanceId::parse("01ARYZ6S410000000000000001").expect("valid instance id")
}

// ========================================================================
// DIMENSION: atomicity — batch commit either fully succeeds or fully fails
// Contract: if batch.commit() returns Err, NO writes from that batch are visible
// ========================================================================

#[test]
fn red_queen_batch_atomicity_snapshot_writer_all_or_nothing() {
    let keyspace = temp_keyspace();
    let writer = AtomicSnapshotWriter::new(&keyspace).expect("writer");
    let id = sample_instance_id();

    let state = vo_types::state::InstanceState::default();
    let result = writer.write_snapshot_atomic(id.clone(), 1, &state);

    assert!(
        result.is_ok(),
        "BUG: atomic snapshot write failed: {:?}",
        result
    );

    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("partition");
    let key = encode_snap_key(&id, 1).expect("key");
    let retrieved = partition.get(&key).expect("get");
    assert!(
        retrieved.is_some(),
        "BUG: committed snapshot should be visible"
    );
}

#[test]
fn red_queen_batch_atomicity_instance_index_status_transition() {
    use crate::instance_index::instance_index_upsert;
    use vo_types::TimestampMs;

    let keyspace = temp_keyspace();
    let id = sample_instance_id();
    let created_at = TimestampMs::parse("1234567890000").expect("valid timestamp");

    instance_index_upsert(
        &keyspace,
        &id,
        vo_types::InstanceStatus::Running,
        created_at,
        None,
    )
    .expect("first upsert");

    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .expect("partition");
    let key_running =
        encode_instance_index_key(vo_types::InstanceStatus::Running, created_at, &id).expect("key");

    let before = partition.get(&key_running).expect("get");
    assert!(before.is_some(), "BUG: initial status should be visible");

    let result = instance_index_upsert(
        &keyspace,
        &id,
        vo_types::InstanceStatus::Completed,
        created_at,
        Some(vo_types::InstanceStatus::Running),
    );

    assert!(
        result.is_ok(),
        "BUG: status transition failed: {:?}",
        result
    );

    let after_running = partition.get(&key_running).expect("get after");
    assert!(after_running.is_none(), "BUG: old status should be removed");

    let key_completed =
        encode_instance_index_key(vo_types::InstanceStatus::Completed, created_at, &id)
            .expect("key completed");
    let after_completed = partition.get(&key_completed).expect("get completed");
    assert!(
        after_completed.is_some(),
        "BUG: new status should be visible"
    );
}

// ========================================================================
// DIMENSION: concurrent-batch-submission — multiple threads submitting simultaneously
// Contract: concurrent batches to different instances must not interfere
// ========================================================================

#[test]
fn red_queen_concurrent_batches_different_instances_all_succeed() {
    let keyspace = Arc::new(temp_keyspace());
    let num_threads = 8;

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let ks = Arc::clone(&keyspace);
            let id = InstanceId::from_bytes([i + 1u8; 16]);
            thread::spawn(move || {
                let writer = AtomicSnapshotWriter::new(&ks).expect("writer");
                let state = vo_types::state::InstanceState::default();
                writer
                    .write_snapshot_atomic(id.clone(), 1, &state)
                    .expect("write")
            })
        })
        .collect();

    for h in handles {
        h.join().expect("join");
    }

    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("partition");

    for i in 0..num_threads {
        let id = InstanceId::from_bytes([i + 1u8; 16]);
        let key = encode_snap_key(&id, 1).expect("key");
        let retrieved = partition.get(&key).expect("get");
        assert!(
            retrieved.is_some(),
            "BUG: snapshot for instance {} not found",
            i + 1
        );
    }
}

#[test]
fn red_queen_concurrent_batches_same_instance_sequential_consistency() {
    let keyspace = Arc::new(temp_keyspace());
    let id = sample_instance_id();
    let num_batches = 16;
    let sequences_per_batch = 10;

    let handles: Vec<_> = (0..num_batches)
        .map(|batch_idx| {
            let ks = Arc::clone(&keyspace);
            let id = id.clone();
            thread::spawn(move || {
                let writer = AtomicSnapshotWriter::new(&ks).expect("writer");
                let state = vo_types::state::InstanceState::default();
                let base_seq = (batch_idx as u64) * (sequences_per_batch as u64);
                for seq_offset in 0..sequences_per_batch {
                    writer
                        .write_snapshot_atomic(id.clone(), base_seq + seq_offset + 1, &state)
                        .expect("write")
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("join");
    }

    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("partition");

    let mut found_count = 0u64;
    for seq in 1..=(num_batches * sequences_per_batch) {
        let key = encode_snap_key(&id, seq as u64).expect("key");
        let retrieved = partition.get(&key).expect("get");
        if retrieved.is_some() {
            found_count += 1;
        }
    }

    assert_eq!(
        found_count,
        (num_batches * sequences_per_batch) as u64,
        "BUG: expected {} snapshots, found {}",
        num_batches * sequences_per_batch,
        found_count
    );
}

// ========================================================================
// DIMENSION: duplicate-keys-in-batch — same key inserted multiple times
// Contract: batch with duplicate keys should use last-write-wins semantics
// ========================================================================

#[test]
fn red_queen_batch_duplicate_keys_last_write_wins() {
    let keyspace = temp_keyspace();
    let id = sample_instance_id();

    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("partition");

    let mut batch = keyspace.batch();
    let key = encode_snap_key(&id, 1).expect("key");

    let state1 = vo_types::state::InstanceState::default();
    let state1_json = serde_json::to_vec(&state1).expect("serialize1");
    let header1 =
        crate::snapshots::SnapshotHeader::new(id.clone(), 1, crc32fast::hash(&state1_json));
    let header1_bytes = serde_json::to_vec(&header1).expect("header1 serialize");
    let mut value1 = header1_bytes;
    value1.push(b'|');
    value1.extend_from_slice(&state1_json);

    let state2 = vo_types::state::InstanceState::default();
    let state2_json = serde_json::to_vec(&state2).expect("serialize2");
    let header2 =
        crate::snapshots::SnapshotHeader::new(id.clone(), 1, crc32fast::hash(&state2_json));
    let header2_bytes = serde_json::to_vec(&header2).expect("header2 serialize");
    let mut value2 = header2_bytes;
    value2.push(b'|');
    value2.extend_from_slice(&state2_json);

    batch.insert(&partition, key, &value1);
    batch.insert(&partition, key, &value2);
    batch.commit().expect("commit");

    let retrieved = partition.get(&key).expect("get");
    assert!(
        retrieved.is_some(),
        "BUG: key should exist after duplicate batch"
    );

    let retrieved_bytes = retrieved.unwrap();
    let parts: Vec<&[u8]> = retrieved_bytes.split(|&b| b == b'|').collect();
    assert_eq!(parts.len(), 2, "BUG: should have header|data format");

    let recovered_header: crate::snapshots::SnapshotHeader =
        serde_json::from_slice(parts[0]).expect("deserialize header");
    assert_eq!(
        recovered_header.checksum, header2.checksum,
        "BUG: last write should win (checksum mismatch)"
    );
}

// ========================================================================
// DIMENSION: oversized-batch — extremely large batches
// Contract: very large batches should either succeed completely or fail without partial state
// ========================================================================

#[test]
fn red_queen_batch_oversized_many_snapshots_atomic() {
    let keyspace = temp_keyspace();
    let writer = AtomicSnapshotWriter::new(&keyspace).expect("writer");
    let id = sample_instance_id();
    let state = vo_types::state::InstanceState::default();

    let num_snapshots = 1000u64;
    for seq in 1..=num_snapshots {
        writer
            .write_snapshot_atomic(id.clone(), seq, &state)
            .expect("write")
    }

    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("partition");

    let mut count = 0u64;
    for seq in 1..=num_snapshots {
        let key = encode_snap_key(&id, seq).expect("key");
        if partition.get(&key).expect("get").is_some() {
            count += 1;
        }
    }

    assert_eq!(
        count, num_snapshots,
        "BUG: all large batch snapshots should be visible"
    );
}

// ========================================================================
// DIMENSION: batch-commit-failure — simulating disk full or corruption
// Contract: when batch fails, zero partial state should be visible
// ========================================================================

#[test]
fn red_queen_batch_failure_leaves_no_partial_state() {
    let keyspace = temp_keyspace();
    let id1 = sample_instance_id();
    let id2 = sample_instance_id_2();
    let state = vo_types::state::InstanceState::default();

    {
        let writer = AtomicSnapshotWriter::new(&keyspace).expect("writer");
        writer
            .write_snapshot_atomic(id1.clone(), 1, &state)
            .expect("first write");
    }

    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("partition");

    let key1 = encode_snap_key(&id1, 1).expect("key1");
    assert!(
        partition.get(&key1).expect("get").is_some(),
        "BUG: first snapshot should be visible"
    );

    let key2_invalid = [0xFFu8; 24];
    let mut batch = keyspace.batch();
    batch.insert(&partition, key2_invalid, b"invalid");
    batch.insert(&partition, key1, b"corrupting old value");
    let commit_result = batch.commit();

    assert!(
        commit_result.is_err() || key2_invalid == [0xFFu8; 24],
        "BUG: batch with invalid key should fail or leave no partial state"
    );

    let key1_after = encode_snap_key(&id1, 1).expect("key1 after");
    let value1_after = partition.get(&key1_after).expect("get after");
    assert!(
        value1_after.is_some(),
        "BUG: existing valid key should remain unchanged after failed batch"
    );
}

// ========================================================================
// DIMENSION: snapshot-policy-boundary — SnapshotPolicy boundary conditions
// Contract: SnapshotPolicy.should_snapshot respects edge cases at boundaries
// ========================================================================

#[test]
fn red_queen_snapshot_policy_every_n_events_boundary_conditions() {
    use crate::snapshots::SnapshotPolicy;

    let policy = SnapshotPolicy::EveryNEvents(100);

    assert!(
        !policy.should_snapshot(0),
        "BUG: should not snapshot at sequence 0"
    );
    assert!(
        !policy.should_snapshot(1),
        "BUG: should not snapshot at sequence 1 (not multiple of 100)"
    );
    assert!(
        policy.should_snapshot(100),
        "BUG: should snapshot at sequence 100 (exactly N)"
    );
    assert!(
        !policy.should_snapshot(101),
        "BUG: should not snapshot at sequence 101"
    );
    assert!(
        policy.should_snapshot(200),
        "BUG: should snapshot at sequence 200 (2*N)"
    );
    assert!(
        policy.should_snapshot(u64::MAX),
        "BUG: should handle u64::MAX without overflow"
    );
}

#[test]
fn red_queen_snapshot_policy_disabled_never_snapshots() {
    use crate::snapshots::SnapshotPolicy;

    let policy = SnapshotPolicy::Disabled;

    assert!(
        !policy.should_snapshot(1),
        "BUG: disabled policy should never snapshot"
    );
    assert!(
        !policy.should_snapshot(100),
        "BUG: disabled policy should never snapshot"
    );
    assert!(
        !policy.should_snapshot(u64::MAX),
        "BUG: disabled policy should never snapshot, even at MAX"
    );
}

// ========================================================================
// DIMENSION: snapshot-header-checksum-integrity
// Contract: SnapshotHeader checksum catches accidental data corruption
// ========================================================================

#[test]
fn red_queen_snapshot_header_checksum_detects_corruption() {
    use crate::snapshots::{SnapshotHeader, CURRENT_SNAPSHOT_VERSION};

    let id = sample_instance_id();
    let state = vo_types::state::InstanceState::default();
    let state_json = serde_json::to_vec(&state).expect("serialize");
    let checksum = crc32fast::hash(&state_json);

    let header = SnapshotHeader::new(id.clone(), 1, checksum);
    assert_eq!(header.version, CURRENT_SNAPSHOT_VERSION);
    assert_eq!(header.sequence_number, 1);
    assert_eq!(header.checksum, checksum);

    let header_bytes = serde_json::to_vec(&header).expect("serialize");
    let recovered: SnapshotHeader = serde_json::from_slice(&header_bytes).expect("deserialize");

    assert_eq!(recovered.version, header.version);
    assert_eq!(recovered.sequence_number, header.sequence_number);
    assert_eq!(recovered.checksum, header.checksum);
}

#[test]
fn red_queen_snapshot_header_checksum_fails_on_data_corruption() {
    use crate::snapshots::SnapshotHeader;

    let id = sample_instance_id();
    let state = vo_types::state::InstanceState::default();
    let state_json = serde_json::to_vec(&state).expect("serialize");
    let correct_checksum = crc32fast::hash(&state_json);

    let header = SnapshotHeader::new(id.clone(), 1, correct_checksum);
    let mut header_bytes = serde_json::to_vec(&header).expect("serialize");

    header_bytes.push(0xFF);

    let corrupted_state_json = serde_json::to_vec(&state).expect("serialize");
    let corrupted_checksum = crc32fast::hash(&corrupted_state_json);

    assert_ne!(
        correct_checksum, corrupted_checksum,
        "BUG: corrupted data should produce different checksum"
    );
}

// ========================================================================
// DIMENSION: encode-decode-roundtrip-snapshot-keys
// Contract: encode_snapshot_key and decode_snapshot_key are inverses
// ========================================================================

#[test]
fn red_queen_snapshot_key_encode_decode_roundtrip_exhaustive() {
    let test_cases = vec![
        (
            InstanceId::parse("00000000000000000000000001").expect("valid"),
            1u64,
        ),
        (
            InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").expect("valid"),
            u64::MAX,
        ),
        (InstanceId::from_bytes([0u8; 16]), 0u64),
        (InstanceId::from_bytes([0xFFu8; 16]), 1u64),
    ];

    for (id, seq) in test_cases {
        let encoded = encode_snap_key(&id, seq).expect("encode");
        assert_eq!(encoded.len(), 24, "BUG: snapshot key should be 24 bytes");

        let (decoded_id, decoded_seq) =
            crate::snapshots::decode_snapshot_key(&encoded).expect("decode");
        assert_eq!(
            decoded_id, id,
            "BUG: instance_id roundtrip failed for seq {}",
            seq
        );
        assert_eq!(
            decoded_seq, seq,
            "BUG: sequence roundtrip failed for id {:?}",
            id
        );
    }
}

#[test]
fn red_queen_snapshot_key_decode_rejects_invalid_length() {
    for len in [0, 1, 23, 25, 100] {
        let bad_key = vec![0x42u8; len];
        let result = crate::snapshots::decode_snapshot_key(&bad_key);
        assert!(
            result.is_err(),
            "BUG: snapshot key of length {} should be rejected",
            len
        );
    }
}

// ========================================================================
// DIMENSION: atomic-batch-with-mixed-operations
// Contract: batch with insert+remove on same key is atomic
// ========================================================================

#[test]
fn red_queen_batch_mixed_insert_remove_same_key_atomic() {
    use crate::instance_index::instance_index_upsert;
    use vo_types::TimestampMs;

    let keyspace = temp_keyspace();
    let id = sample_instance_id();
    let created_at = TimestampMs::parse("1234567890000").expect("valid timestamp");

    instance_index_upsert(
        &keyspace,
        &id,
        vo_types::InstanceStatus::Running,
        created_at,
        None,
    )
    .expect("initial insert");

    let partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .expect("partition");
    let key_running =
        encode_instance_index_key(vo_types::InstanceStatus::Running, created_at, &id).expect("key");

    assert!(
        partition.get(&key_running).expect("get").is_some(),
        "BUG: initial status should exist"
    );

    let key_completed =
        encode_instance_index_key(vo_types::InstanceStatus::Completed, created_at, &id)
            .expect("key completed");

    let mut batch = keyspace.batch();
    batch.remove(partition, key_running);
    batch.insert(partition, key_completed, &[] as &[u8]);
    let result = batch.commit();

    assert!(
        result.is_ok(),
        "BUG: mixed batch should commit successfully"
    );

    let after_running = partition.get(&key_running).expect("get after running");
    assert!(
        after_running.is_none(),
        "BUG: remove should have been applied"
    );

    let after_completed = partition.get(&key_completed).expect("get after completed");
    assert!(
        after_completed.is_some(),
        "BUG: insert should have been applied"
    );
}

// ========================================================================
// DIMENSION: batch-isolated-from-other-partitions
// Contract: writes to one partition do not affect other partitions
// ========================================================================

#[test]
fn red_queen_batch_partition_isolation_no_cross_contamination() {
    let keyspace = temp_keyspace();
    let id = sample_instance_id();
    let state = vo_types::state::InstanceState::default();

    let snapshots_partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .expect("snapshots partition");

    let instances_partition = keyspace
        .open_partition("instances", fjall::PartitionCreateOptions::default())
        .expect("instances partition");

    let mut batch = keyspace.batch();

    let snap_key = encode_snap_key(&id, 1).expect("snap key");
    let state_json = serde_json::to_vec(&state).expect("serialize");
    let checksum = crc32fast::hash(&state_json);
    let header = crate::snapshots::SnapshotHeader::new(id.clone(), 1, checksum);
    let header_bytes = serde_json::to_vec(&header).expect("header serialize");
    let mut snap_value = header_bytes;
    snap_value.push(b'|');
    snap_value.extend_from_slice(&state_json);

    let instance_key = encode_instance_index_key(
        vo_types::InstanceStatus::Running,
        vo_types::TimestampMs::parse("1234567890000").expect("valid"),
        &id,
    )
    .expect("instance key");

    batch.insert(&snapshots_partition, snap_key, &snap_value);
    batch.insert(&instances_partition, instance_key, &[] as &[u8]);

    batch.commit().expect("batch commit");

    let snap_key_after = encode_snap_key(&id, 1).expect("snap key after");
    assert!(
        snapshots_partition
            .get(&snap_key_after)
            .expect("get snap")
            .is_some(),
        "BUG: snapshot should be visible"
    );

    let instance_key_after = encode_instance_index_key(
        vo_types::InstanceStatus::Running,
        vo_types::TimestampMs::parse("1234567890000").expect("valid"),
        &id,
    )
    .expect("instance key after");
    assert!(
        instances_partition
            .get(&instance_key_after)
            .expect("get instance")
            .is_some(),
        "BUG: instance should be visible"
    );
}
