//! Unit tests for instance index key encoding and decoding.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use super::*;

// ---- Test helpers ----

fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

fn make_test_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

// ---- B07: Encoder produces exactly 25 bytes ----

#[test]
fn encode_instance_index_key_returns_25_byte_array_when_inputs_valid() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(1000);
    let result = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    assert_eq!(result.len(), 25);
}

// ---- B08: Encoded key starts with status byte ----

#[test]
fn encode_instance_index_key_places_status_byte_at_position_zero() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(1000);
    let key = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();
    assert_eq!(key[0], 0x02);
}

// ---- B09: Encoded key contains created_at as big-endian u64 at [1..9] ----

#[test]
fn encode_instance_index_key_encodes_created_at_as_big_endian_u64_at_offset_1() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(1000);
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    assert_eq!(&key[1..9], &1000u64.to_be_bytes());
}

// ---- B10: Encoded key contains instance_id bytes at [9..25] ----

#[test]
fn encode_instance_index_key_encodes_instance_id_bytes_at_offset_9() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(1000);
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    assert_eq!(&key[9..25], &[0x01u8; 16]);
}

// ---- B11: Concrete example — Pending, ts=1000, id=[0x01; 16] ----

#[test]
fn encode_instance_index_key_returns_contract_example_bytes_when_pending_ts1000() {
    let id = InstanceId::from_bytes([0x01; 16]);
    let ts = make_test_timestamp(1000);
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let expected: [u8; 25] = [
        0x01, // status = Pending
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xE8, // created_at = 1000 BE (8 bytes)
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, // instance_id (16 bytes)
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, // (continued)
    ];
    assert_eq!(key, expected);
}

// ---- B12: Concrete example — Failed, ts=0, id=[0xFF; 16] ----

#[test]
fn encode_instance_index_key_returns_contract_example_bytes_when_failed_ts0() {
    let id = InstanceId::from_bytes([0xFF; 16]);
    let ts = make_test_timestamp(0);
    let key = encode_instance_index_key(InstanceStatus::Failed, ts, &id).unwrap();
    let expected: [u8; 25] = [
        0x05, // status = Failed
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // created_at = 0 BE
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // instance_id
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // (continued)
    ];
    assert_eq!(key, expected);
}

// ---- B13: Encoder is deterministic ----

#[test]
fn encode_instance_index_key_is_deterministic_when_called_twice_with_same_inputs() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(12345);
    let first = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();
    let second = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();
    assert_eq!(first, second);
}

// ---- B43: Encoder handles nil/zero InstanceId boundary ----

#[test]
fn encode_instance_index_key_handles_nil_instance_id_boundary() {
    let id = InstanceId::from_bytes([0x00; 16]);
    let ts = make_test_timestamp(1000);
    let result = encode_instance_index_key(InstanceStatus::Pending, ts, &id);
    match result {
        Ok(key) => {
            assert_eq!(&key[9..25], &[0x00u8; 16]);
        }
        Err(StorageError::CorruptKey) => {
            // Type system rejects nil ULID during to_bytes() — this is acceptable
        }
        Err(other) => {
            panic!("Unexpected error variant: {other:?}");
        }
    }
}

// ---- B44: Encoder produces correct status byte for remaining variants ----

#[test]
fn encode_instance_index_key_places_0x03_at_position_zero_when_paused() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(500);
    let key = encode_instance_index_key(InstanceStatus::Paused, ts, &id).unwrap();
    assert_eq!(key[0], 0x03);
}

#[test]
fn encode_instance_index_key_places_0x04_at_position_zero_when_completed() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(500);
    let key = encode_instance_index_key(InstanceStatus::Completed, ts, &id).unwrap();
    assert_eq!(key[0], 0x04);
}

#[test]
fn encode_instance_index_key_places_0x06_at_position_zero_when_cancelled() {
    let id = make_test_instance_id(0x01);
    let ts = make_test_timestamp(500);
    let key = encode_instance_index_key(InstanceStatus::Cancelled, ts, &id).unwrap();
    assert_eq!(key[0], 0x06);
}

// ---- B14: Decoder returns correct InstanceIndexEntry for valid key ----

#[test]
fn decode_instance_index_key_returns_correct_entry_when_key_is_valid_25_bytes() {
    let id = InstanceId::from_bytes([0x01; 16]);
    let ts = make_test_timestamp(1000);
    let key = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let result = decode_instance_index_key(&key).unwrap();
    assert_eq!(result.status, InstanceStatus::Pending);
    assert_eq!(result.created_at, ts);
    assert_eq!(result.instance_id, id);
}

// ---- B15: Decoder rejects short input (24 bytes) ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_input_is_24_bytes() {
    let input = [0x01u8; 24];
    assert_eq!(
        decode_instance_index_key(&input),
        Err(StorageError::CorruptKey)
    );
}

// ---- B16: Decoder rejects long input (26 bytes) ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_input_is_26_bytes() {
    let input = [0x01u8; 26];
    assert_eq!(
        decode_instance_index_key(&input),
        Err(StorageError::CorruptKey)
    );
}

// ---- B17: Decoder rejects empty input ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_input_is_empty() {
    assert_eq!(
        decode_instance_index_key(&[]),
        Err(StorageError::CorruptKey)
    );
}

// ---- B18: Decoder rejects zero status byte ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_status_byte_is_zero() {
    let mut key = [0x00u8; 25];
    key[0] = 0x00; // already zero, but explicit
    assert_eq!(
        decode_instance_index_key(&key),
        Err(StorageError::CorruptKey)
    );
}

// ---- B19: Decoder rejects status byte 0x07 ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_status_byte_is_0x07() {
    let mut key = [0x01u8; 25];
    key[0] = 0x07;
    assert_eq!(
        decode_instance_index_key(&key),
        Err(StorageError::CorruptKey)
    );
}

// ---- B20: Decoder rejects status byte 0xFF ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_status_byte_is_0xff() {
    let mut key = [0x01u8; 25];
    key[0] = 0xFF;
    assert_eq!(
        decode_instance_index_key(&key),
        Err(StorageError::CorruptKey)
    );
}

// ---- B47: Decoder rejects 1-byte input ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_input_is_1_byte() {
    assert_eq!(
        decode_instance_index_key(&[0x01]),
        Err(StorageError::CorruptKey)
    );
}

// ---- B48: Decoder rejects oversized input (1000 bytes) ----

#[test]
fn decode_instance_index_key_returns_corrupt_key_when_input_is_1000_bytes() {
    assert_eq!(
        decode_instance_index_key(&[0x01; 1000]),
        Err(StorageError::CorruptKey)
    );
}

// ---- B21: Encode → Decode round-trip ----

#[test]
fn encode_then_decode_returns_original_components_when_inputs_valid() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(999_999);
    let status = InstanceStatus::Completed;
    let key = encode_instance_index_key(status, ts, &id).unwrap();
    let entry = decode_instance_index_key(&key).unwrap();
    assert_eq!(entry.instance_id, id);
    assert_eq!(entry.status, status);
    assert_eq!(entry.created_at, ts);
}
