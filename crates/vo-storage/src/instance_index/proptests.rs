//! Property-based tests for instance index key encoding/decoding.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;
use proptest::prelude::*;

/// Strategy for generating a valid `InstanceStatus` (uniform over 6 variants).
fn arb_instance_status() -> impl Strategy<Value = InstanceStatus> {
    (1u8..=6u8).prop_map(|b| InstanceStatus::from_byte(b).unwrap())
}

/// Strategy for generating a valid `InstanceId` from non-nil bytes.
fn arb_instance_id_bytes() -> impl Strategy<Value = [u8; 16]> {
    proptest::array::uniform16(proptest::num::u8::ANY)
        .prop_filter("non-nil ULID (u128 != 0)", |bytes| {
            u128::from_be_bytes(*bytes) != 0
        })
}

proptest! {
    // ---- P01: encode → decode round-trip (all valid inputs) ----

    #[test]
    fn proptest_encode_decode_round_trip(
        status in arb_instance_status(),
        ts in proptest::num::u64::ANY,
        id_bytes in arb_instance_id_bytes(),
    ) {
        let id = InstanceId::from_bytes(id_bytes);
        let timestamp = TimestampMs::try_from(ts).unwrap();
        let key = encode_instance_index_key(status, timestamp, &id).unwrap();
        let entry = decode_instance_index_key(&key).unwrap();
        prop_assert_eq!(entry.instance_id, id);
        prop_assert_eq!(entry.status, status);
        prop_assert_eq!(entry.created_at, timestamp);
    }

    // ---- P02: Encoded key is always exactly 25 bytes ----

    #[test]
    fn proptest_encoded_key_is_always_25_bytes(
        status in arb_instance_status(),
        ts in proptest::num::u64::ANY,
        id_bytes in arb_instance_id_bytes(),
    ) {
        let id = InstanceId::from_bytes(id_bytes);
        let timestamp = TimestampMs::try_from(ts).unwrap();
        let key = encode_instance_index_key(status, timestamp, &id).unwrap();
        prop_assert_eq!(key.len(), 25);
    }

    // ---- P03: Status byte is always in [0x01..=0x06] ----

    #[test]
    fn proptest_status_byte_is_in_valid_range(
        status in arb_instance_status(),
    ) {
        let byte = status.to_byte();
        prop_assert!((0x01..=0x06).contains(&byte));
    }

    // ---- P04: from_byte(to_byte(s)) == Some(s) for all variants ----

    #[test]
    fn proptest_from_byte_to_byte_round_trip(
        status in arb_instance_status(),
    ) {
        prop_assert_eq!(InstanceStatus::from_byte(status.to_byte()), Some(status));
    }

    // ---- P05: from_byte rejects all invalid bytes ----

    #[test]
    fn proptest_from_byte_rejects_invalid_bytes(
        byte in (0u8..=0xFF_u8).prop_filter("not valid status byte", |b| !(1..=6).contains(b)),
    ) {
        prop_assert_eq!(InstanceStatus::from_byte(byte), None);
    }

    // ---- P06: Key ordering preserves chronological order within same status ----

    #[test]
    fn proptest_key_ordering_preserves_chronological_order(
        status in arb_instance_status(),
        t1 in 0u64..u64::MAX,
        id_bytes in arb_instance_id_bytes(),
    ) {
        let t2 = t1 + 1; // t2 > t1
        let id = InstanceId::from_bytes(id_bytes);
        let ts1 = TimestampMs::try_from(t1).unwrap();
        let ts2 = TimestampMs::try_from(t2).unwrap();
        let key1 = encode_instance_index_key(status, ts1, &id).unwrap();
        let key2 = encode_instance_index_key(status, ts2, &id).unwrap();
        prop_assert!(key1 < key2, "key1 should sort before key2 (t1={t1} < t2={t2})");
    }
}
