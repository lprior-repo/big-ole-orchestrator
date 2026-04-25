//! Red Queen tests: Encode/Decode edge cases.

use vo_storage::instance_index::{decode_instance_index_key, encode_instance_index_key, InstanceStatus};

use crate::instance_index_red_queen::helpers::*;

// ---------------------------------------------------------------------------
// RQ-ED01: Every InstanceStatus variant encodes and decodes correctly
// ---------------------------------------------------------------------------

#[test]
fn rq_all_six_status_variants_encode_decode_round_trip() {
    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(12345);

    (InstanceStatus::all_variants()).into_iter().for_each(|status| {
        let key = encode_instance_index_key(*status, ts, &id).unwrap();
        let entry = decode_instance_index_key(&key).unwrap();
        assert_eq!(
            entry.status, *status,
            "Status {:?} failed round-trip",
            status
        );
        assert_eq!(entry.created_at, ts);
        assert_eq!(entry.instance_id, id);
    });
}

// ---------------------------------------------------------------------------
// RQ-ED02: Key bytes are exactly the contract-specified layout
// ---------------------------------------------------------------------------

#[test]
fn rq_key_layout_matches_contract_specification_for_all_statuses() {
    let id_bytes = [0xAB; 16];
    let id = vo_types::InstanceId::from_bytes(id_bytes);
    let ts_value = 0x0102030405060708u64;
    let ts = make_test_timestamp(ts_value);

    (InstanceStatus::all_variants()).into_iter().for_each(|status| {
        let key = encode_instance_index_key(*status, ts, &id).unwrap();

        assert_eq!(key[0], status.to_byte());
        assert_eq!(&key[1..9], &ts_value.to_be_bytes());
        assert_eq!(&key[9..25], &id_bytes);
    });
}

// ---------------------------------------------------------------------------
// RQ-ED03: Encode key with different InstanceIds produces different keys
// ---------------------------------------------------------------------------

#[test]
fn rq_different_instance_ids_same_status_and_ts_produce_different_keys() {
    let id1 = vo_types::InstanceId::from_bytes([0x01; 16]);
    let id2 = vo_types::InstanceId::from_bytes([0x02; 16]);
    let ts = make_test_timestamp(1000);

    let key1 = encode_instance_index_key(InstanceStatus::Pending, ts, &id1).unwrap();
    let key2 = encode_instance_index_key(InstanceStatus::Pending, ts, &id2).unwrap();

    assert_ne!(key1, key2, "Different IDs must produce different keys");
    assert_eq!(&key1[0..9], &key2[0..9], "Status and timestamp should be identical");
    assert_ne!(&key1[9..25], &key2[9..25], "Instance ID portion should differ");
}

// ---------------------------------------------------------------------------
// RQ-ED04: Decode handles crafted key with valid status but extreme values
// ---------------------------------------------------------------------------

#[test]
fn rq_decode_handles_extreme_timestamp_and_id_values() {
    let mut max_key = [0xFF; 25];
    max_key[0] = 0x06;
    let entry = decode_instance_index_key(&max_key).unwrap();
    assert_eq!(entry.status, InstanceStatus::Cancelled);
    assert_eq!(entry.created_at, make_test_timestamp(u64::MAX));

    let mut min_key = [0x00; 25];
    min_key[0] = 0x01;
    let entry = decode_instance_index_key(&min_key).unwrap();
    assert_eq!(entry.status, InstanceStatus::Pending);
    assert_eq!(entry.created_at, make_test_timestamp(0));
}
