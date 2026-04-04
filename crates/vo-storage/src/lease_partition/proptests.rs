#![allow(clippy::unwrap_used)]
use super::*;
use proptest::prelude::*;

proptest! {
    /// INV-LP-PROP-001: LeaseEntry serde round-trip preserves equality.
    #[test]
    fn lease_entry_serde_roundtrip(
        iid in "[a-zA-Z0-9]{1,50}",
        sid in "[a-zA-Z0-9]{1,50}",
        fence in 1u64..=u64::MAX,
        expires in 1u64..=u64::MAX,
    ) {
        let entry = LeaseEntry::new(iid, sid, fence, expires).unwrap();
        let json = serde_json::to_string(&entry).unwrap();
        let recovered: LeaseEntry = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(entry, recovered);
    }

    /// INV-LP-PROP-002: encode/decode lease key round-trip.
    #[test]
    fn encode_decode_lease_key_roundtrip() {
        let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let sid = StepId::parse("step-proptest").unwrap();
        let key = encode_lease_key(&iid, &sid);
        let (recovered_iid, recovered_sid) = decode_lease_key(&key).unwrap();
        prop_assert_eq!(recovered_iid, iid);
        prop_assert_eq!(recovered_sid, sid);
    }

    /// INV-LP-PROP-003: is_expired is monotonic.
    #[test]
    fn is_expired_monotonic(
        iid in "[a-zA-Z0-9]{1,50}",
        sid in "[a-zA-Z0-9]{1,50}",
        fence in 1u64..=u64::MAX,
        expires in 1u64..=u64::MAX - 1,
    ) {
        let entry = LeaseEntry::new(iid, sid, fence, expires).unwrap();
        if entry.is_expired(expires) {
            prop_assert!(entry.is_expired(expires + 1));
        }
    }
}
