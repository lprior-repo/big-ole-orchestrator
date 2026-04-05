#![allow(clippy::unwrap_used)]
//! Mutation-killer tests for dedupe_partition contract (vel-7ffu).
//!
//! These tests use unique values that cannot be confused with hardcoded mutations.
//! A mutation returning "", "xyzzy", 0, or 1 will ALWAYS fail these tests.

use super::*;

// ========================================================================
// KILLER: dedupe_key getter returns the EXACT string passed to constructor.
// Mutations: returning "" or "xyzzy" instead of actual key will fail.
// ========================================================================

#[test]
fn dedupe_entry_dedupe_key_getter_returns_exact_value() {
    let unique_key = "🦀rustacean-test-key-7ffu".to_string();
    let e = DedupeEntry::new(unique_key.clone(), "instance".to_string(), 5000).unwrap();
    assert_eq!(e.dedupe_key(), unique_key.as_str());
    assert_ne!(e.dedupe_key(), "");
    assert_ne!(e.dedupe_key(), "xyzzy");
    assert_eq!(e.dedupe_key().len(), unique_key.len());
}

// ========================================================================
// KILLER: instance_id getter returns the EXACT string passed to constructor.
// Mutations: returning "" or "xyzzy" instead of actual instance_id will fail.
// ========================================================================

#[test]
fn dedupe_entry_instance_id_getter_returns_exact_value() {
    let unique_iid = "iid-chicken-instance-7ffu".to_string();
    let e = DedupeEntry::new("key".to_string(), unique_iid.clone(), 5000).unwrap();
    assert_eq!(e.instance_id(), unique_iid.as_str());
    assert_ne!(e.instance_id(), "");
    assert_ne!(e.instance_id(), "xyzzy");
    assert_eq!(e.instance_id().len(), unique_iid.len());
}

// ========================================================================
// KILLER: expires_at getter returns the EXACT u64 passed to constructor.
// Mutations: returning 0 or 1 instead of actual expires_at will fail.
// ========================================================================

#[test]
fn dedupe_entry_expires_at_getter_returns_exact_value() {
    let unique_expires: u64 = 1_700_000_000_000u64;
    let e = DedupeEntry::new("key".to_string(), "iid".to_string(), unique_expires).unwrap();
    assert_eq!(e.expires_at(), unique_expires);
    assert_ne!(e.expires_at(), 0);
    assert_ne!(e.expires_at(), 1);
    assert!(e.expires_at() > 1_000_000_000_000u64);
}

// ========================================================================
// KILLER: is_expired boundary - entry expires AT the expires_at timestamp.
// Mutation: changing >= to < would make this fail.
// ========================================================================

#[test]
fn dedupe_entry_is_expired_at_exact_boundary_timestamp() {
    let ts: u64 = 42;
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), ts).unwrap();
    assert!(
        entry.is_expired(ts),
        "is_expired({ts}) must return true when expires_at == {ts}",
    );
}

// ========================================================================
// KILLER: is_expired returns false ONE MILLISECOND before expiry.
// Mutation: changing >= to < would make this fail.
// ========================================================================

#[test]
fn dedupe_entry_is_expired_returns_false_one_ms_before() {
    let ts: u64 = 42;
    let entry = DedupeEntry::new("key".to_string(), "iid".to_string(), ts).unwrap();
    let before = ts - 1;
    assert!(
        !entry.is_expired(before),
        "is_expired({before}) must return false when now = {before}",
    );
}

// ========================================================================
// KILLER: encode_dedupe_key produces correct bytes for multi-byte UTF-8 key.
// Mutations returning wrong bytes would fail this.
// ========================================================================

#[test]
fn encode_dedupe_key_produces_exact_bytes_for_multibyte_key() {
    let key = DedupeKey::parse("日本語テストキー").unwrap();
    let bytes = encode_dedupe_key(&key);
    assert_eq!(bytes, "日本語テストキー".as_bytes());
    assert!(
        bytes.len() > 1,
        "Multibyte key must encode to more than 1 byte"
    );
}
