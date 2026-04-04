#![allow(clippy::unwrap_used)]
use super::*;
use std::collections::HashMap;

fn test_instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn test_step_id() -> StepId {
    StepId::parse("step-1").unwrap()
}

fn test_fence_token(v: u64) -> FenceToken {
    FenceToken::new(v).unwrap()
}

// ========================================================================
// LeaseEntry Construction
// ========================================================================

#[test]
fn lease_entry_constructs_with_valid_fields() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1000);
    assert!(entry.is_ok());
    let e = entry.unwrap();
    assert_eq!(e.instance_id(), "iid");
    assert_eq!(e.step_id(), "sid");
    assert_eq!(e.fence_token(), 1);
    assert_eq!(e.expires_at(), 1000);
}

#[test]
fn lease_entry_rejects_empty_instance_id() {
    let result = LeaseEntry::new("".to_string(), "sid".to_string(), 1, 1000);
    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn lease_entry_rejects_empty_step_id() {
    let result = LeaseEntry::new("iid".to_string(), "".to_string(), 1, 1000);
    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn lease_entry_rejects_zero_fence_token() {
    let result = LeaseEntry::new("iid".to_string(), "sid".to_string(), 0, 1000);
    assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
}

#[test]
fn lease_entry_is_expired_returns_true_when_past_expiry() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1000).unwrap();
    assert!(entry.is_expired(1000));
    assert!(entry.is_expired(2000));
}

#[test]
fn lease_entry_is_expired_returns_false_before_expiry() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 1, 1000).unwrap();
    assert!(!entry.is_expired(999));
}

#[test]
fn lease_entry_serde_roundtrip() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 5, 9999).unwrap();
    let json = serde_json::to_string(&entry).unwrap();
    let recovered: LeaseEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, entry);
}

// ========================================================================
// Error Display
// ========================================================================

#[test]
fn error_lease_already_held_displays_ids() {
    let err = LeaseStoreError::LeaseAlreadyHeld {
        instance_id: "iid".to_string(),
        step_id: "sid".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("iid") && msg.contains("sid"));
}

#[test]
fn error_not_found_displays_ids() {
    let err = LeaseStoreError::NotFound {
        instance_id: "iid".to_string(),
        step_id: "sid".to_string(),
    };
    assert!(err.to_string().contains("not found"));
}

#[test]
fn error_stale_fence_displays_tokens() {
    let err = LeaseStoreError::StaleFence {
        expected: "5".to_string(),
        actual: "3".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("5") && msg.contains("3"));
}

#[test]
fn error_storage_displays_reason() {
    let err = LeaseStoreError::Storage {
        reason: "io error".to_string(),
    };
    assert!(err.to_string().contains("io error"));
}

#[test]
fn error_codec_displays_reason() {
    let err = LeaseStoreError::Codec {
        reason: "bad data".to_string(),
    };
    assert!(err.to_string().contains("bad data"));
}

// ========================================================================
// Calc Layer — Key Encode/Decode
// ========================================================================

#[test]
fn encode_lease_key_produces_delimited_bytes() {
    let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let sid = StepId::parse("step-1").unwrap();
    let key = encode_lease_key(&iid, &sid);
    let key_str = std::str::from_utf8(&key).unwrap();
    assert!(key_str.contains("01H5JYV4XHGSR2F8KZ9BWNRFMA::step-1"));
}

#[test]
fn decode_lease_key_recovers_ids() {
    let iid = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let sid = StepId::parse("step-1").unwrap();
    let key = encode_lease_key(&iid, &sid);
    let (recovered_iid, recovered_sid) = decode_lease_key(&key).unwrap();
    assert_eq!(recovered_iid, iid);
    assert_eq!(recovered_sid, sid);
}

#[test]
fn decode_lease_key_returns_error_for_missing_delimiter() {
    let result = decode_lease_key(b"nodelimiter");
    assert!(matches!(result, Err(LeaseStoreError::Codec { .. })));
}

// ========================================================================
// Calc Layer — Entry Encode/Decode
// ========================================================================

#[test]
fn encode_decode_lease_entry_roundtrip() {
    let entry = LeaseEntry::new("iid".to_string(), "sid".to_string(), 7, 5000).unwrap();
    let bytes = encode_lease_entry(&entry).unwrap();
    let recovered = decode_lease_entry(&bytes).unwrap();
    assert_eq!(recovered, entry);
}

#[test]
fn decode_lease_entry_returns_error_for_invalid_json() {
    let result = decode_lease_entry(b"not-json");
    assert!(matches!(result, Err(LeaseStoreError::Codec { .. })));
}
