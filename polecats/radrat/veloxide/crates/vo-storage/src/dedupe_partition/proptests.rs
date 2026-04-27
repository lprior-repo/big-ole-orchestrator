#![allow(clippy::unwrap_used)]
use proptest::prelude::*;

proptest! {
    /// INV-DD-PROP-001: DedupeEntry serde round-trip preserves equality.
    #[test]
    fn dedupe_entry_serde_roundtrip(
        key in "[a-zA-Z0-9_-]{1,100}",
        instance_id in "[a-zA-Z0-9_-]{1,100}",
        expires_at in 1u64..=u64::MAX,
    ) {
        let entry = super::DedupeEntry::new(key, instance_id, expires_at).unwrap();
        let json = serde_json::to_string(&entry).unwrap();
        let recovered: super::DedupeEntry = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(entry, recovered);
    }

    /// INV-DD-PROP-002: encode/decode dedupe key round-trip.
    #[test]
    fn encode_decode_dedupe_key_roundtrip(
        key in "[a-zA-Z0-9_-]{1,256}"
    ) {
        let dk = super::DedupeKey::parse(&key).unwrap();
        let bytes = super::encode_dedupe_key(&dk);
        let recovered = super::decode_dedupe_key(&bytes).unwrap();
        prop_assert_eq!(dk.as_str(), recovered.as_str());
    }

    /// INV-DD-PROP-003: is_expired is monotonic — if expired at T, expired at T+1.
    #[test]
    fn is_expired_monotonic(
        key in "[a-zA-Z0-9]{1,50}",
        iid in "[a-zA-Z0-9]{1,50}",
        expires_at in 1u64..=u64::MAX - 1,
    ) {
        let entry = super::DedupeEntry::new(key, iid, expires_at).unwrap();
        if entry.is_expired(expires_at) {
            prop_assert!(entry.is_expired(expires_at + 1));
        }
    }
}
