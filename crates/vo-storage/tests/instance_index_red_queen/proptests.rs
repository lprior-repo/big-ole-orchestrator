#![allow(clippy::unwrap_used)]
#![allow(clippy::into_iter_on_ref)]

use proptest::prelude::*;

use super::helpers::*;

fn arb_instance_status() -> impl Strategy<Value = InstanceStatus> {
    (1u8..=6u8).prop_map(|b| InstanceStatus::from_byte(b).unwrap())
}

fn arb_instance_id_bytes() -> impl Strategy<Value = [u8; 16]> {
    proptest::array::uniform16(proptest::num::u8::ANY)
        .prop_filter("non-nil ULID (u128 != 0)", |bytes| {
            u128::from_be_bytes(*bytes) != 0
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn rq_proptest_arbitrary_length_rejected(
        len in (0usize..100).prop_filter("not 25", |l| *l != 25),
        fill in proptest::num::u8::ANY,
    ) {
        let key = vec![fill; len];
        prop_assert_eq!(decode_instance_index_key(&key), Err(vo_storage::codec::StorageError::CorruptKey));
    }

    #[test]
    fn rq_proptest_upsert_then_scan_yields_one_entry(
        status in arb_instance_status(),
        ts in proptest::num::u64::ANY,
        id_bytes in arb_instance_id_bytes(),
    ) {
        let (_dir, database) = make_test_keyspace();
        let id = InstanceId::from_bytes(id_bytes);
        let timestamp = TimestampMs::try_from(ts).unwrap();

        instance_index_upsert(&database, &id, status, timestamp, None).unwrap();

        let all = collect_scan_ok(scan_all_instances(&database));
        prop_assert_eq!(all.len(), 1);
        prop_assert_eq!(all[0].status, status);
        prop_assert_eq!(all[0].created_at, timestamp);
        prop_assert_eq!(all[0].instance_id.clone(), id);
    }

    #[test]
    fn rq_proptest_status_transition_leaves_one_key(
        old_status in arb_instance_status(),
        new_status in arb_instance_status(),
        ts in proptest::num::u64::ANY,
        id_bytes in arb_instance_id_bytes(),
    ) {
        let (_dir, database) = make_test_keyspace();
        let id = InstanceId::from_bytes(id_bytes);
        let timestamp = TimestampMs::try_from(ts).unwrap();

        instance_index_upsert(&database, &id, old_status, timestamp, None).unwrap();
        instance_index_upsert(&database, &id, new_status, timestamp, Some(old_status)).unwrap();

        let all = collect_scan_ok(scan_all_instances(&database));
        prop_assert_eq!(all.len(), 1, "After transition, exactly 1 key should exist");
        prop_assert_eq!(all[0].status, new_status);
    }

    #[test]
    fn rq_proptest_key_ordering_within_status(
        status in arb_instance_status(),
        t1 in 0u64..u64::MAX,
        id_bytes in arb_instance_id_bytes(),
    ) {
        let t2 = t1 + 1;
        let id = InstanceId::from_bytes(id_bytes);
        let ts1 = TimestampMs::try_from(t1).unwrap();
        let ts2 = TimestampMs::try_from(t2).unwrap();

        let key1 = encode_instance_index_key(status, ts1, &id).unwrap();
        let key2 = encode_instance_index_key(status, ts2, &id).unwrap();

        prop_assert!(key1 < key2, "key(t1={t1}) should be < key(t2={t2})");
    }

    #[test]
    fn rq_proptest_different_statuses_different_first_byte(
        s1 in arb_instance_status(),
        s2 in arb_instance_status(),
        ts in proptest::num::u64::ANY,
        id_bytes in arb_instance_id_bytes(),
    ) {
        prop_assume!(s1 != s2);
        let id = InstanceId::from_bytes(id_bytes);
        let timestamp = TimestampMs::try_from(ts).unwrap();

        let key1 = encode_instance_index_key(s1, timestamp, &id).unwrap();
        let key2 = encode_instance_index_key(s2, timestamp, &id).unwrap();

        prop_assert_ne!(key1[0], key2[0], "Different statuses must produce different first bytes");
    }
}