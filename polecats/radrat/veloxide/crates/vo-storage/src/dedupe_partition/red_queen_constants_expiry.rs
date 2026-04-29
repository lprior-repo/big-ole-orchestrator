//! Red Queen adversarial tests: constants and expiry semantics.

use crate::dedupe_partition::*;

// ========================================================================
// DIMENSION: constant-value — DEDUPE_PARTITION must be exactly "dedupe"
// ========================================================================

#[test]
fn red_queen_constant_is_exactly_dedupe() {
    assert_eq!(DEDUPE_PARTITION, "dedupe");
}

#[test]
fn red_queen_constant_is_not_empty() {
    assert!(!DEDUPE_PARTITION.is_empty());
}

#[test]
fn red_queen_constant_len_is_6() {
    assert_eq!(DEDUPE_PARTITION.len(), 6);
}

// ========================================================================
// DIMENSION: expiry-semantics — edge case: expires_at = 0, now_ms = 0
// ========================================================================

#[test]
fn red_queen_expired_at_zero_boundary() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), 0).unwrap();
    assert!(entry.is_expired(0));
}

#[test]
fn red_queen_expired_at_zero_boundary_with_nonzero_now() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), 0).unwrap();
    assert!(entry.is_expired(1));
}

#[test]
fn red_queen_not_expired_at_u64_max_minus_one() {
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), u64::MAX).unwrap();
    assert!(!entry.is_expired(u64::MAX - 1));
}
