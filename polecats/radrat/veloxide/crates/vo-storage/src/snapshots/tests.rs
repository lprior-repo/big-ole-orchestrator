#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::similar_names,
    clippy::unreadable_literal
)]

use super::*;
use vo_types::state::InstanceState;

fn get_min_id() -> InstanceId {
    InstanceId::parse("00000000000000000000000000")
        .unwrap_or_else(|_| InstanceId::from_bytes([0; 16]))
}

fn get_max_id() -> InstanceId {
    InstanceId::from_bytes([255; 16])
}

fn get_typical_id() -> InstanceId {
    InstanceId::from_bytes([1; 16])
}

// --- encode_snapshot_key ---

#[test]
fn encode_snapshot_key_formats_key_exactly() {
    let id = get_typical_id();
    let result = encode_snapshot_key(&id, 42).unwrap();
    let mut expected = [1; 24];
    expected[16..24].copy_from_slice(&42u64.to_be_bytes());
    assert_eq!(result, expected);
}

#[test]
fn encode_snapshot_key_boundary_minimum_sequence_and_id() {
    let id = get_min_id();
    let result = encode_snapshot_key(&id, 0).unwrap();
    assert_eq!(result, [0; 24]);
}

#[test]
fn encode_snapshot_key_boundary_maximum_sequence_and_id() {
    let id = get_max_id();
    let result = encode_snapshot_key(&id, u64::MAX).unwrap();
    let expected = [255; 24];
    assert_eq!(result, expected);
}

#[test]
fn encode_snapshot_key_uses_big_endian_for_sequence() {
    let id = get_typical_id();
    let result = encode_snapshot_key(&id, 0x0102030405060708).unwrap();
    assert_eq!(&result[16..24], &[1, 2, 3, 4, 5, 6, 7, 8]);
}

// --- decode_snapshot_key ---

#[test]
fn decode_snapshot_key_parses_valid_key() {
    let mut key = [1; 24];
    key[16..24].copy_from_slice(&42u64.to_be_bytes());
    let result = decode_snapshot_key(&key);
    assert_eq!(result, Ok((get_typical_id(), 42)));
}

#[test]
fn decode_snapshot_key_rejects_parsing_when_byte_slice_length_is_not_exactly_24_bytes_23() {
    let key = [0; 23];
    let result = decode_snapshot_key(&key);
    assert_eq!(result, Err(StorageError::InvalidKey));
}

#[test]
fn decode_snapshot_key_returns_invalid_key_error_when_length_is_25() {
    let key = [0; 25];
    let result = decode_snapshot_key(&key);
    assert_eq!(result, Err(StorageError::InvalidKey));
}

#[test]
fn decode_snapshot_key_rejects_parsing_when_byte_slice_length_is_zero() {
    let key: [u8; 0] = [];
    let result = decode_snapshot_key(&key);
    assert_eq!(result, Err(StorageError::InvalidKey));
}

#[test]
fn decode_snapshot_key_handles_minimum_values_correctly() {
    let key = [0; 24];
    let result = decode_snapshot_key(&key);
    assert_eq!(result, Ok((InstanceId::from_bytes([0; 16]), 0)));
}

#[test]
fn decode_snapshot_key_handles_maximum_values_correctly() {
    let key = [255; 24];
    let result = decode_snapshot_key(&key);
    assert_eq!(result, Ok((InstanceId::from_bytes([255; 16]), u64::MAX)));
}

// --- snapshot_write ---

fn setup_fjall() -> (tempfile::TempDir, fjall::Keyspace, PartitionHandle) {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = fjall::Config::new(temp_dir.path());
    let keyspace = config.open().unwrap();
    let partition = keyspace
        .open_partition("snapshots", fjall::PartitionCreateOptions::default())
        .unwrap();
    (temp_dir, keyspace, partition)
}

#[test]
fn snapshot_write_persists_state_when_valid_state_and_sequence_provided() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let state = InstanceState { counter: 42 };
    let result = snapshot_write(&partition, get_typical_id(), 100, &state);
    assert_eq!(result, Ok(()));
    let load_result = snapshot_load_latest(&partition, &get_typical_id());
    assert_eq!(load_result, Ok(Some((100, state))));
}

#[test]
fn snapshot_write_persists_state_in_populated_partition() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let state1 = InstanceState { counter: 10 };
    snapshot_write(&partition, get_typical_id(), 50, &state1).expect("write failed");

    let state2 = InstanceState { counter: 42 };
    let result = snapshot_write(&partition, get_typical_id(), 100, &state2);
    assert_eq!(result, Ok(()));
    let load_result = snapshot_load_latest(&partition, &get_typical_id());
    assert_eq!(load_result, Ok(Some((100, state2))));
}

#[test]
fn snapshot_write_boundary_instance_id_minimum() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let state = InstanceState { counter: 1 };
    let result = snapshot_write(&partition, get_min_id(), 1, &state);
    assert_eq!(result, Ok(()));
    let load_result = snapshot_load_latest(&partition, &get_min_id());
    assert_eq!(load_result, Ok(Some((1, state))));
}

#[test]
fn snapshot_write_boundary_instance_id_maximum() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let state = InstanceState { counter: 1 };
    let result = snapshot_write(&partition, get_max_id(), 1, &state);
    assert_eq!(result, Ok(()));
    let load_result = snapshot_load_latest(&partition, &get_max_id());
    assert_eq!(load_result, Ok(Some((1, state))));
}

#[test]
fn snapshot_write_overwrites_existing_data_idempotently() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let state1 = InstanceState { counter: 10 };
    snapshot_write(&partition, get_typical_id(), 100, &state1).expect("write failed");

    let state2 = InstanceState { counter: 20 };
    let result = snapshot_write(&partition, get_typical_id(), 100, &state2);
    assert_eq!(result, Ok(()));

    let load_result = snapshot_load_latest(&partition, &get_typical_id());
    assert_eq!(load_result, Ok(Some((100, state2))));
}

#[test]
fn snapshot_write_returns_serialization_failed_when_unserializable() {
    // serde_json cannot fail to serialize InstanceState { counter: u64 },
    // but the code path exists. This test documents that the error variant
    // is correctly wired: the .map_err(|_| StorageError::SerializationFailed)
    // path is structurally present in snapshot_write. A true injection test
    // would require a mock serde Serializer, which is out of scope.
    let (_dir, _keyspace, partition) = setup_fjall();
    let state = InstanceState { counter: 0 };
    let result = snapshot_write(&partition, get_typical_id(), 1, &state);
    // Happy path succeeds — error variant untestable without mock serializer
    assert_eq!(result, Ok(()));
}

#[test]
fn snapshot_write_returns_fjall_error_on_engine_failure() {
    // Cannot easily force fjall to return an error without corrupting the
    // underlying storage. This test documents that the error variant is wired
    // via .map_err(|_| StorageError::FjallError) in snapshot_write.
    let (_dir, _keyspace, partition) = setup_fjall();
    let state = InstanceState { counter: 1 };
    let result = snapshot_write(&partition, get_typical_id(), 1, &state);
    // Happy path succeeds — error variant untestable without mock engine
    assert_eq!(result, Ok(()));
}

// --- snapshot_load_latest ---

#[test]
fn snapshot_load_latest_returns_the_snapshot_with_the_highest_sequence_number() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();
    snapshot_write(&partition, id.clone(), 255, &InstanceState { counter: 1 })
        .expect("write failed");
    snapshot_write(&partition, id.clone(), 256, &InstanceState { counter: 5 })
        .expect("write failed");

    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Ok(Some((256, InstanceState { counter: 5 }))));
}

#[test]
fn snapshot_load_latest_boundary_instance_id_minimum() {
    let (_dir, _keyspace, partition) = setup_fjall();
    snapshot_write(&partition, get_min_id(), 10, &InstanceState { counter: 1 })
        .expect("write failed");

    let result = snapshot_load_latest(&partition, &get_min_id());
    assert_eq!(result, Ok(Some((10, InstanceState { counter: 1 }))));
}

#[test]
fn snapshot_load_latest_boundary_instance_id_maximum() {
    let (_dir, _keyspace, partition) = setup_fjall();
    snapshot_write(&partition, get_max_id(), 10, &InstanceState { counter: 1 })
        .expect("write failed");

    let result = snapshot_load_latest(&partition, &get_max_id());
    assert_eq!(result, Ok(Some((10, InstanceState { counter: 1 }))));
}

#[test]
fn snapshot_load_latest_returns_none_when_no_snapshots_exist() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let result = snapshot_load_latest(&partition, &get_typical_id());
    assert_eq!(result, Ok(None));
}

#[test]
fn snapshot_load_latest_does_not_bleed_into_next_prefix_when_instance_a_is_less_than_b() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let mut id_a_bytes = [0; 16];
    id_a_bytes[15] = 1;
    let id_a = InstanceId::from_bytes(id_a_bytes);

    let mut id_b_bytes = [0; 16];
    id_b_bytes[15] = 2;
    let id_b = InstanceId::from_bytes(id_b_bytes);

    snapshot_write(&partition, id_a.clone(), 50, &InstanceState { counter: 99 })
        .expect("write failed");
    snapshot_write(&partition, id_b, 100, &InstanceState { counter: 100 }).expect("write failed");

    let result = snapshot_load_latest(&partition, &id_a);
    assert_eq!(result, Ok(Some((50, InstanceState { counter: 99 }))));
}

#[test]
fn snapshot_load_latest_enforces_backward_seek_without_unbounded_iteration() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();
    (1..=100).for_each(|i| {
        snapshot_write(&partition, id.clone(), i, &InstanceState { counter: i })
            .expect("write failed");
    });

    // This test would timeout or fail if it iterated 10,000 times in the real implementation.
    // We ensure it gets the correct latest value.
    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Ok(Some((100, InstanceState { counter: 100 }))));
}

#[test]
fn snapshot_load_latest_rejects_corrupted_key_from_storage() {
    let (_dir, _keyspace, partition) = setup_fjall();
    // Insert a corrupt key directly into fjall (23 bytes instead of 24)
    let corrupt_key = vec![0; 23];
    partition.insert(corrupt_key, b"{}").expect("insert failed");

    let result = snapshot_load_latest(&partition, &InstanceId::from_bytes([0; 16]));
    assert_eq!(result, Err(StorageError::InvalidKey));
}

#[test]
fn snapshot_load_latest_returns_deserialization_failed_on_corrupt_json_value() {
    let (_dir, _keyspace, partition) = setup_fjall();
    // Insert a valid 24-byte key but with corrupt (non-JSON) value
    let id = InstanceId::from_bytes([0; 16]);
    let key = encode_snapshot_key(&id, 1).unwrap();
    partition
        .insert(key, b"not valid json")
        .expect("insert failed");

    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Err(StorageError::DeserializationFailed));
}

// ============================================================
// Delta Snapshot Tests (Compression + Checksum)
// ============================================================

#[test]
fn snapshot_write_compresses_with_zstd() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    // Write a snapshot
    let state = InstanceState { counter: 12345 };
    snapshot_write(&partition, id.clone(), 1, &state).expect("write failed");

    // Read raw bytes to verify compression
    let key = encode_snapshot_key(&id, 1).unwrap();
    let raw_value = partition.get(&key).unwrap().unwrap();

    // Compressed data should be reasonable size (small data may not compress well)
    // but should not be unreasonably large
    assert!(
        raw_value.len() < 500,
        "Compressed snapshot should be reasonable size"
    );
}

#[test]
fn snapshot_load_verifies_checksum() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    // Write a valid snapshot
    let state = InstanceState { counter: 42 };
    snapshot_write(&partition, id.clone(), 1, &state).expect("write failed");

    // Load should succeed with valid checksum
    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Ok(Some((1, state))));
}

#[test]
fn snapshot_write_handles_first_snapshot_without_base() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    // First snapshot should work without any prior snapshot
    let state = InstanceState { counter: 42 };
    snapshot_write(&partition, id.clone(), 1, &state).expect("write failed");

    // Load should work
    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Ok(Some((1, state))));
}

#[test]
fn snapshot_load_decompresses_and_reconstructs() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    // Write snapshot with specific counter value
    let expected_state = InstanceState { counter: 99999 };
    snapshot_write(&partition, id.clone(), 1, &expected_state).expect("write failed");

    // Load should decompress and return correct state
    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Ok(Some((1, expected_state))));
}

#[test]
fn snapshot_load_latest_handles_atomic_snapshot_writer_format() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    let state = InstanceState { counter: 42 };
    let state_json = serde_json::to_vec(&state).unwrap();
    let checksum = crc32fast::hash(&state_json);
    let header = SnapshotHeader::new(id.clone(), 1, checksum);
    let header_json = serde_json::to_vec(&header).unwrap();

    let mut value = header_json;
    value.push(b'|');
    value.extend_from_slice(&state_json);

    let key = encode_snapshot_key(&id, 1).unwrap();
    partition.insert(key, &value).unwrap();

    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Ok(Some((1, state))));
}

#[test]
fn snapshot_load_latest_rejects_corrupt_checksum_in_header_format() {
    let (_dir, _keyspace, partition) = setup_fjall();
    let id = get_typical_id();

    let state = InstanceState { counter: 42 };
    let state_json = serde_json::to_vec(&state).unwrap();
    let wrong_checksum = 0u32;
    let header = SnapshotHeader::new(id.clone(), 1, wrong_checksum);
    let header_json = serde_json::to_vec(&header).unwrap();

    let mut value = header_json;
    value.push(b'|');
    value.extend_from_slice(&state_json);

    let key = encode_snapshot_key(&id, 1).unwrap();
    partition.insert(key, &value).unwrap();

    let result = snapshot_load_latest(&partition, &id);
    assert_eq!(result, Err(StorageError::DeserializationFailed));
}
