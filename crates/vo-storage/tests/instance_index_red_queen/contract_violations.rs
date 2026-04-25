//! Red Queen tests: Contract Violation Attempts.
//!
//! Tests that the instance index rejects invalid inputs at the key decoding layer.

use vo_storage::codec::StorageError;
use vo_storage::instance_index::{
    decode_instance_index_key, encode_instance_index_key, instance_index_upsert, scan_all_instances,
    InstanceStatus,
};

use crate::instance_index_red_queen::helpers::*;

// ---------------------------------------------------------------------------
// RQ-CV01: Decode rejects every invalid status byte in [0x00, 0x07..=0xFF]
// ---------------------------------------------------------------------------

#[test]
fn rq_decode_rejects_every_invalid_status_byte_exhaustively() {
    use vo_storage::instance_index::decode_instance_index_key;

    (0u8..=0xFF).into_iter().for_each(|byte| {
        let mut key = [0x01u8; 25]; // valid 25-byte key template
        key[0] = byte;
        let result = decode_instance_index_key(&key);
        if (0x01..=0x06).contains(&byte) {
            let Ok(_val) = result else {
                panic!("Status byte 0x{byte:02X} should be valid, got {result:?}");
            };
        } else {
            assert_eq!(
                result,
                Err(StorageError::CorruptKey),
                "Status byte 0x{byte:02X} should be rejected"
            );
        }
    });
}

// ---------------------------------------------------------------------------
// RQ-CV02: Decode rejects every invalid length from 0..=50 (except 25)
// ---------------------------------------------------------------------------

#[test]
fn rq_decode_rejects_every_invalid_length_from_0_to_50() {
    use vo_storage::instance_index::decode_instance_index_key;

    (0usize..=50).into_iter().for_each(|len| {
        let key = vec![0x01u8; len];
        let result = decode_instance_index_key(&key);
        if len == 25 {
            let Ok(_val) = result else {
                panic!("Length 25 should be valid, got {result:?}");
            };
        } else {
            assert_eq!(
                result,
                Err(StorageError::CorruptKey),
                "Length {len} should be rejected"
            );
        }
    });
}

// ---------------------------------------------------------------------------
// RQ-CV03: Nil UUID (all-zero bytes) round-trip through encode/decode
// ---------------------------------------------------------------------------

#[test]
fn rq_nil_uuid_encode_decode_behavior_is_consistent() {
    let nil_id = vo_types::InstanceId::from_bytes([0x00; 16]);
    let ts = make_test_timestamp(1000);

    let encode_result = encode_instance_index_key(InstanceStatus::Pending, ts, &nil_id);

    match encode_result {
        Ok(key) => {
            let entry =
                decode_instance_index_key(&key).expect("Decode must succeed if encode succeeded");
            assert_eq!(entry.instance_id, nil_id);
            assert_eq!(entry.status, InstanceStatus::Pending);
            assert_eq!(entry.created_at, ts);
            assert_eq!(key.len(), 25);
            assert_eq!(&key[9..25], &[0x00u8; 16]);
        }
        Err(StorageError::CorruptKey) => {
            // Acceptable: type system rejects nil ULID during to_bytes()
        }
        Err(other) => {
            panic!("Unexpected error variant for nil UUID: {other:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// RQ-CV04: Nil UUID upsert behavior (if encoding succeeds)
// ---------------------------------------------------------------------------

#[test]
fn rq_nil_uuid_upsert_either_succeeds_consistently_or_fails_with_corrupt_key() {
    let (_dir, database) = make_test_keyspace();
    let nil_id = vo_types::InstanceId::from_bytes([0x00; 16]);
    let ts = make_test_timestamp(500);

    let result = instance_index_upsert(&database, &nil_id, InstanceStatus::Pending, ts, None);

    match result {
        Ok(()) => {
            let all = collect_scan_ok(scan_all_instances(&database));
            assert_eq!(
                all.len(),
                1,
                "Nil UUID upsert succeeded but scan found != 1 entry"
            );
            assert_eq!(all[0].instance_id, nil_id);
        }
        Err(StorageError::CorruptKey) => {
            let all = collect_scan_ok(scan_all_instances(&database));
            assert_eq!(all.len(), 0, "Failed upsert should leave no entries");
        }
        Err(other) => {
            panic!("Unexpected error variant for nil UUID upsert: {other:?}");
        }
    }
}
