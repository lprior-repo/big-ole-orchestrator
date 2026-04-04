#![allow(clippy::unwrap_used)]
use super::*;

/// K-01: Verify LeaseEntry::new rejects zero fence_token.
#[kani::proof]
fn verify_lease_entry_rejects_zero_fence() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), 0, 1000);
    assert!(result.is_err());
}

/// K-02: Verify encode/decode lease key round-trip.
#[kani::proof]
fn verify_encode_decode_lease_key_roundtrip() {
    let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let sid = StepId::parse("step-kani").unwrap();
    let key = encode_lease_key(&iid, &sid);
    let (recovered_iid, recovered_sid) = decode_lease_key(&key).unwrap();
    assert_eq!(recovered_iid, iid);
    assert_eq!(recovered_sid, sid);
}
