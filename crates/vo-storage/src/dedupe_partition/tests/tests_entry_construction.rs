#![allow(clippy::unwrap_used)]
//! Unit tests for `DedupeEntry` construction and expiry behavior.

use super::*;

// ========================================================================
// DedupeEntry Construction
// ========================================================================

#[test]
fn dedupe_entry_constructs_with_valid_fields() {
    let e = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    assert_eq!(e.dedupe_key(), "key-1");
    assert_eq!(e.instance_id(), "instance-1");
    assert_eq!(e.expires_at(), 1000);
}

#[test]
fn dedupe_entry_rejects_empty_dedupe_key() {
    let result = DedupeEntry::new(String::new(), "instance-1".to_string(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_entry_rejects_empty_instance_id() {
    let result = DedupeEntry::new("key-1".to_string(), String::new(), 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_entry_constructs_with_zero_expiry_boundary() {
    let result = DedupeEntry::new("key-0".to_string(), "instance-0".to_string(), 0);
    assert_eq!(
        result,
        Ok(DedupeEntry {
            dedupe_key: "key-0".to_string(),
            instance_id: "instance-0".to_string(),
            expires_at: 0,
        })
    );
}

// ========================================================================
// DedupeEntry Expiry Behavior
// ========================================================================

#[test]
fn dedupe_entry_is_expired_returns_true_when_past_expiry() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    assert!(entry.is_expired(1000));
    assert!(entry.is_expired(2000));
}

#[test]
fn dedupe_entry_is_expired_returns_false_before_expiry() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    assert!(!entry.is_expired(999));
}

#[test]
fn dedupe_entry_is_not_expired_before_u64_max_boundary() {
    let entry =
        DedupeEntry::new("key-max".to_string(), "instance-max".to_string(), u64::MAX).unwrap();
    assert!(!entry.is_expired(u64::MAX - 1));
}

#[test]
fn dedupe_entry_is_expired_at_u64_max_boundary() {
    let entry =
        DedupeEntry::new("key-max".to_string(), "instance-max".to_string(), u64::MAX).unwrap();
    assert!(entry.is_expired(u64::MAX));
}

#[test]
fn dedupe_entry_expires_at_returns_u64_max_when_set() {
    let e = DedupeEntry::new("key".to_string(), "iid".to_string(), u64::MAX).unwrap();
    assert_eq!(e.expires_at(), u64::MAX);
    assert_ne!(e.expires_at(), 0);
    assert_ne!(e.expires_at(), 1);
}

#[test]
fn dedupe_entry_is_expired_at_u64_max_timestamp() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), u64::MAX).unwrap();
    assert!(entry.is_expired(u64::MAX));
}

#[test]
fn dedupe_entry_is_expired_returns_false_at_u64_max_minus_one() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), u64::MAX).unwrap();
    assert!(!entry.is_expired(u64::MAX - 1));
}

// ========================================================================
// DedupeEntry Serde
// ========================================================================

#[test]
fn dedupe_entry_serde_roundtrip() {
    let entry = DedupeEntry::new("key-1".to_string(), "instance-1".to_string(), 1000).unwrap();
    let json = serde_json::to_string(&entry).unwrap();
    let recovered: DedupeEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, entry);
}

// ========================================================================
// AdmissionResult
// ========================================================================

#[test]
fn admission_result_admitted_equality() {
    let a = AdmissionResult::Admitted;
    let b = AdmissionResult::Admitted;
    assert_eq!(a, b);
}

#[test]
fn admission_result_duplicate_equality() {
    let a = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    let b = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn admission_result_admitted_ne_duplicate() {
    let a = AdmissionResult::Admitted;
    let b = AdmissionResult::Duplicate {
        instance_id: "inst-1".to_string(),
    };
    assert_ne!(a, b);
}

// ========================================================================
// Error Display
// ========================================================================

#[test]
fn error_storage_displays_reason() {
    let err = DedupeStoreError::Storage {
        reason: "disk full".to_string(),
    };
    assert_eq!(err.to_string(), "dedupe storage error: disk full");
}

#[test]
fn error_codec_displays_reason() {
    let err = DedupeStoreError::Codec {
        reason: "bad json".to_string(),
    };
    assert_eq!(err.to_string(), "dedupe codec error: bad json");
}

#[test]
fn error_invalid_argument_displays_message() {
    let err = DedupeStoreError::InvalidArgument;
    assert_eq!(err.to_string(), "invalid dedupe argument");
}

#[test]
fn error_storage_display_contains_reason_string() {
    let err = DedupeStoreError::Storage {
        reason: "disk-full-7ffu-🦀".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("disk-full-7ffu-🦀"));
    assert!(display.starts_with("dedupe storage error:"));
    assert!(!display.is_empty());
}

#[test]
fn error_codec_display_contains_reason_string() {
    let err = DedupeStoreError::Codec {
        reason: "bad-json-7ffu".to_string(),
    };
    let display = err.to_string();
    assert!(display.contains("bad-json-7ffu"));
    assert!(display.starts_with("dedupe codec error:"));
}

#[test]
fn error_invalid_argument_display_is_exact_string() {
    let err = DedupeStoreError::InvalidArgument;
    let display = err.to_string();
    assert_eq!(display, "invalid dedupe argument");
    assert!(!display.is_empty());
}

// ========================================================================
// DedupeRetentionRecord Construction
// ========================================================================

#[test]
fn dedupe_retention_record_constructs_with_valid_fields() {
    let r = DedupeRetentionRecord::new("key-1".to_string(), "instance-1".to_string(), 1000, 2000)
        .unwrap();
    assert_eq!(r.dedupe_key(), "key-1");
    assert_eq!(r.instance_id(), "instance-1");
    assert_eq!(r.terminal_state_at(), 1000);
    assert_eq!(r.retention_expires_at(), 2000);
}

#[test]
fn dedupe_retention_record_rejects_empty_dedupe_key() {
    let result = DedupeRetentionRecord::new(String::new(), "instance-1".to_string(), 1000, 2000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_retention_record_rejects_empty_instance_id() {
    let result = DedupeRetentionRecord::new("key-1".to_string(), String::new(), 1000, 2000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_retention_record_rejects_terminal_state_after_expiry() {
    let result =
        DedupeRetentionRecord::new("key-1".to_string(), "instance-1".to_string(), 2000, 1000);
    assert_eq!(result, Err(DedupeStoreError::InvalidArgument));
}

#[test]
fn dedupe_retention_record_accepts_terminal_state_equals_expiry() {
    let result =
        DedupeRetentionRecord::new("key-1".to_string(), "instance-1".to_string(), 1000, 1000);
    assert!(result.is_ok());
}

// ========================================================================
// DedupeRetentionRecord Expiry Behavior
// ========================================================================

#[test]
fn dedupe_retention_record_is_retention_expired_returns_true_when_past_expiry() {
    let r = DedupeRetentionRecord::new("key-1".to_string(), "instance-1".to_string(), 1000, 2000)
        .unwrap();
    assert!(r.is_retention_expired(2000));
    assert!(r.is_retention_expired(3000));
}

#[test]
fn dedupe_retention_record_is_retention_expired_returns_false_before_expiry() {
    let r = DedupeRetentionRecord::new("key-1".to_string(), "instance-1".to_string(), 1000, 2000)
        .unwrap();
    assert!(!r.is_retention_expired(1999));
}

#[test]
fn dedupe_retention_record_is_not_expired_before_u64_max_boundary() {
    let r = DedupeRetentionRecord::new(
        "key-max".to_string(),
        "instance-max".to_string(),
        u64::MAX - 1,
        u64::MAX,
    )
    .unwrap();
    assert!(!r.is_retention_expired(u64::MAX - 1));
}

#[test]
fn dedupe_retention_record_is_expired_at_u64_max_boundary() {
    let r = DedupeRetentionRecord::new(
        "key-max".to_string(),
        "instance-max".to_string(),
        u64::MAX - 1,
        u64::MAX,
    )
    .unwrap();
    assert!(r.is_retention_expired(u64::MAX));
}

// ========================================================================
// DedupeRetentionRecord Serde
// ========================================================================

#[test]
fn dedupe_retention_record_serde_roundtrip() {
    let r = DedupeRetentionRecord::new("key-1".to_string(), "instance-1".to_string(), 1000, 2000)
        .unwrap();
    let json = serde_json::to_string(&r).unwrap();
    let recovered: DedupeRetentionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, r);
}

// ========================================================================
// DedupeRetentionRecord compute_retention_expiry
// ========================================================================

#[test]
fn dedupe_retention_record_compute_retention_expiry_adds_period_to_terminal_state() {
    let expiry = DedupeRetentionRecord::compute_retention_expiry(1000, 500);
    assert_eq!(expiry, 1500);
}

#[test]
fn dedupe_retention_record_compute_retention_expiry_saturates_at_u64_max() {
    let expiry = DedupeRetentionRecord::compute_retention_expiry(u64::MAX, 1000);
    assert_eq!(expiry, u64::MAX);
}
