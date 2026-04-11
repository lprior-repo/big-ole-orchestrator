#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for signal types in vo-types.
//!
//! Attack vectors:
//! - WAITKEY: empty, max-length boundary, serde bypass, unicode normalization,
//!   null bytes, whitespace-only, control characters
//! - WAITRECORD: invariant enforcement, serde round-trip with boundary values,
//!   TimestampMs boundary
//! - SIGNALADDRESS: scope/epoch invariant, hash consistency, Display format
//! - BUFFERPOLICY: exhaustive match, is_buffering correctness
//! - SIGNALDELIVERY: is_terminal/is_pending mutual exclusion
//! - DEDUPEKEY: hash collision resistance, empty field edge cases

use std::collections::HashSet;

use rstest::rstest;
use vo_types::{
    BufferPolicy, Epoch, IdempotencyKey, InstanceId, LineageScope, SignalAddress, SignalDedupeKey,
    SignalDelivery, TimestampMs, WaitKey, WaitRecord,
};

fn valid_instance_id() -> InstanceId {
    InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID for test setup")
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 1: WAITKEY — Empty string rejected with correct error type
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_empty_rejected() {
    let result = WaitKey::parse("");
    assert!(result.is_err(), "BUG: empty WaitKey should be rejected");
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("WaitKey"),
        "BUG: error message should mention WaitKey, got: {msg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 2: WAITKEY — Exact 256-char boundary accepted
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_exact_256_accepted() {
    let input = "k".repeat(256);
    let result = WaitKey::parse(&input);
    assert!(result.is_ok(), "BUG: 256-char WaitKey should be accepted");
    assert_eq!(result.unwrap().as_str().len(), 256);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 3: WAITKEY — 257-char boundary rejected
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_257_rejected() {
    let input = "k".repeat(257);
    let result = WaitKey::parse(&input);
    assert!(result.is_err(), "BUG: 257-char WaitKey should be rejected");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 4: WAITKEY — Unicode chars count correctly (multi-byte = 1 char)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_unicode_256_multibyte_chars_accepted() {
    let input = "日".repeat(256);
    let result = WaitKey::parse(&input);
    assert!(
        result.is_ok(),
        "BUG: 256 multi-byte unicode chars should be accepted"
    );
}

#[test]
fn attack_waitkey_unicode_257_multibyte_chars_rejected() {
    let input = "日".repeat(257);
    let result = WaitKey::parse(&input);
    assert!(
        result.is_err(),
        "BUG: 257 multi-byte unicode chars should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 5: WAITKEY — Serde round-trip with boundary values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_serde_roundtrip_256_chars() {
    let original = WaitKey::parse(&"x".repeat(256)).expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: WaitKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn attack_waitkey_serde_roundtrip_single_char() {
    let original = WaitKey::parse("a").expect("valid");
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: WaitKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn attack_waitkey_serde_rejects_empty_string() {
    let json = r#""""#;
    let result: Result<WaitKey, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "BUG: empty string should fail serde deserialization"
    );
}

#[test]
fn attack_waitkey_serde_rejects_257_chars() {
    let json = format!(r#""{}""#, "y".repeat(257));
    let result: Result<WaitKey, _> = serde_json::from_str(&json);
    assert!(
        result.is_err(),
        "BUG: 257-char string should fail serde deserialization"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 6: WAITKEY — Control characters and null bytes accepted
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_null_byte_accepted() {
    let input = "key\x00with\x00nulls";
    let result = WaitKey::parse(input);
    assert!(
        result.is_ok(),
        "BUG: null bytes should be accepted (WaitKey is opaque)"
    );
    assert_eq!(result.unwrap().as_str(), "key\x00with\x00nulls");
}

#[test]
fn attack_waitkey_control_characters_accepted() {
    let input = "key\t\n\rwith\x01\x02controls";
    let result = WaitKey::parse(input);
    assert!(result.is_ok(), "BUG: control chars should be accepted");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 7: WAITKEY — TryFrom empty String rejected
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_try_from_empty_string_rejected() {
    let result = WaitKey::try_from(String::new());
    assert!(
        result.is_err(),
        "BUG: TryFrom<String> with empty should fail"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 8: WAITKEY — From<WaitKey> for String preserves exact content
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_into_string_preserves_content() {
    let original = "test-key-with-special-chars-\x00\x01";
    let key = WaitKey::parse(original).expect("valid");
    let back: String = key.into();
    assert_eq!(back, original);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 9: WAITRECORD — Serde round-trip with all buffer policies
// ═══════════════════════════════════════════════════════════════════════════════

#[rstest]
#[case(BufferPolicy::Reject)]
#[case(BufferPolicy::BufferOne)]
#[case(BufferPolicy::BufferMany)]
fn attack_waitrecord_serde_roundtrip_all_policies(#[case] policy: BufferPolicy) {
    let id = valid_instance_id();
    let key = WaitKey::parse("serde-wait").expect("valid");
    let ts = TimestampMs::try_from(42u64).expect("valid");
    let original = WaitRecord::new(id, key, policy, ts).expect("valid inputs");

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: WaitRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
    assert_eq!(restored.buffer_policy(), policy);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 10: WAITRECORD — TimestampMs boundary values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitrecord_with_timestamp_zero() {
    let id = valid_instance_id();
    let key = WaitKey::parse("ts-zero").expect("valid");
    let ts = TimestampMs::try_from(0u64).expect("valid");
    let record = WaitRecord::new(id, key, BufferPolicy::Reject, ts).expect("valid");
    assert_eq!(record.registered_at().as_u64(), 0);
}

#[test]
fn attack_waitrecord_with_timestamp_u64_max() {
    let id = valid_instance_id();
    let key = WaitKey::parse("ts-max").expect("valid");
    let ts = TimestampMs::try_from(u64::MAX).expect("valid");
    let record = WaitRecord::new(id, key, BufferPolicy::BufferOne, ts).expect("valid");
    assert_eq!(record.registered_at().as_u64(), u64::MAX);
}

#[test]
fn attack_waitrecord_with_timestamp_one() {
    let id = valid_instance_id();
    let key = WaitKey::parse("ts-one").expect("valid");
    let ts = TimestampMs::try_from(1u64).expect("valid");
    let record = WaitRecord::new(id, key, BufferPolicy::BufferMany, ts).expect("valid");
    assert_eq!(record.registered_at().as_u64(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 11: SIGNALADDRESS — EpochLocal with Epoch::ZERO
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaladdress_epoch_local_zero_epoch() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("zero-epoch").expect("valid");

    let addr = SignalAddress::epoch_local(lineage, Epoch::ZERO, instance, key);
    assert!(addr.is_epoch_local());
    assert_eq!(addr.epoch_id(), Some(Epoch::ZERO));
    assert!(!addr.is_lineage_wide());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 12: SIGNALADDRESS — LineageWide has no epoch
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaladdress_lineage_wide_no_epoch() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("wide").expect("valid");

    let addr = SignalAddress::lineage_wide(lineage, instance, key);
    assert!(addr.is_lineage_wide());
    assert!(addr.epoch_id().is_none());
    assert!(!addr.is_epoch_local());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 13: SIGNALADDRESS — Hash consistency (same inputs = same hash)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaladdress_hash_consistency() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("hash-test").expect("valid");

    let a = SignalAddress::lineage_wide(lineage.clone(), instance.clone(), key.clone());
    let b = SignalAddress::lineage_wide(lineage, instance, key);

    let mut set = HashSet::new();
    set.insert(a.clone());
    assert!(set.contains(&b), "same inputs should produce same hash");
    set.insert(b);
    assert_eq!(
        set.len(),
        1,
        "identical addresses should not duplicate in HashSet"
    );
}

#[test]
fn attack_signaladdress_different_scope_different_hash() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("scope-hash").expect("valid");

    let wide = SignalAddress::lineage_wide(lineage.clone(), instance.clone(), key.clone());
    let local = SignalAddress::epoch_local(lineage, Epoch::new(1), instance, key);

    assert_ne!(wide, local, "different scopes should be unequal");
    let mut set = HashSet::new();
    set.insert(wide);
    set.insert(local);
    assert_eq!(
        set.len(),
        2,
        "different scopes should produce different hashes"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 14: SIGNALADDRESS — Serde round-trip for both scopes
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaladdress_serde_roundtrip_lineage_wide() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("serde-wide").expect("valid");
    let original = SignalAddress::lineage_wide(lineage, instance, key);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SignalAddress = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn attack_signaladdress_serde_roundtrip_epoch_local() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("serde-local").expect("valid");
    let original = SignalAddress::epoch_local(lineage, Epoch::new(u64::MAX), instance, key);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SignalAddress = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 15: SIGNALADDRESS — Display format contains key fields
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaladdress_display_contains_identifiers() {
    let lineage = valid_instance_id();
    let instance = valid_instance_id();
    let key = WaitKey::parse("display-test").expect("valid");

    let wide = SignalAddress::lineage_wide(lineage.clone(), instance.clone(), key.clone());
    let display = format!("{wide}");
    assert!(
        display.contains("lineage-wide"),
        "BUG: lineage-wide display should contain 'lineage-wide'"
    );

    let local = SignalAddress::epoch_local(lineage, Epoch::new(42), instance, key);
    let display = format!("{local}");
    assert!(
        display.contains("epoch=42"),
        "BUG: epoch-local display should contain 'epoch=42'"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 16: BUFFERPOLICY — Exhaustive match never misses variants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_bufferpolicy_exhaustive_match_proves_closed_enum() {
    for policy in [
        BufferPolicy::Reject,
        BufferPolicy::BufferOne,
        BufferPolicy::BufferMany,
    ] {
        match policy {
            BufferPolicy::Reject => assert!(!policy.is_buffering()),
            BufferPolicy::BufferOne => assert!(policy.is_buffering()),
            BufferPolicy::BufferMany => assert!(policy.is_buffering()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 17: BUFFERPOLICY — Serde round-trip all variants
// ═══════════════════════════════════════════════════════════════════════════════

#[rstest]
#[case(BufferPolicy::Reject)]
#[case(BufferPolicy::BufferOne)]
#[case(BufferPolicy::BufferMany)]
fn attack_bufferpolicy_serde_roundtrip(#[case] policy: BufferPolicy) {
    let json = serde_json::to_string(&policy).expect("serialize");
    let restored: BufferPolicy = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(policy, restored);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 18: BUFFERPOLICY — Serde rejects unknown variant
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_bufferpolicy_serde_rejects_unknown_variant() {
    let json = r#""BufferSometimes""#;
    let result: Result<BufferPolicy, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "BUG: unknown BufferPolicy variant should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 19: SIGNALDELIVERY — is_terminal and is_pending are mutually exclusive
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaldelivery_terminal_pending_mutual_exclusion() {
    for delivery in [
        SignalDelivery::Accepted,
        SignalDelivery::Rejected,
        SignalDelivery::Buffered,
    ] {
        assert_eq!(
            delivery.is_terminal(),
            !delivery.is_pending(),
            "BUG: is_terminal and is_pending must be mutually exclusive for {delivery:?}"
        );
    }
}

#[test]
fn attack_signaldelivery_exhaustive_match() {
    for delivery in [
        SignalDelivery::Accepted,
        SignalDelivery::Rejected,
        SignalDelivery::Buffered,
    ] {
        match delivery {
            SignalDelivery::Accepted => assert!(delivery.is_terminal()),
            SignalDelivery::Rejected => assert!(delivery.is_terminal()),
            SignalDelivery::Buffered => assert!(delivery.is_pending()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 20: SIGNALDELIVERY — Serde rejects unknown variant
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaldelivery_serde_rejects_unknown_variant() {
    let json = r#""MaybeDelivered""#;
    let result: Result<SignalDelivery, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "BUG: unknown SignalDelivery variant should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 21: SIGNALDELIVERY — Copy/Clone correctness
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaldelivery_clone_preserves_semantics() {
    let original = SignalDelivery::Buffered;
    let cloned = original.clone();
    assert_eq!(original, cloned);
    assert_eq!(original.is_pending(), cloned.is_pending());
    assert_eq!(original.is_terminal(), cloned.is_terminal());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 22: DEDUPEKEY — Hash independence across fields
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_dedupekey_different_lineage_different_hash() {
    let id1 = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid");
    let id2 = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid");
    let key = WaitKey::parse("same-key").expect("valid");
    let cmd = IdempotencyKey::parse("cmd-1").expect("valid");

    let a = SignalDedupeKey::new(id1, key.clone(), cmd.clone());
    let b = SignalDedupeKey::new(id2, key, cmd);

    assert_ne!(a, b, "different lineage_id should produce different keys");

    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b);
    assert_eq!(set.len(), 2);
}

#[test]
fn attack_dedupekey_different_wait_key_different_hash() {
    let id = valid_instance_id();
    let key1 = WaitKey::parse("key-a").expect("valid");
    let key2 = WaitKey::parse("key-b").expect("valid");
    let cmd = IdempotencyKey::parse("cmd-1").expect("valid");

    let a = SignalDedupeKey::new(id.clone(), key1, cmd.clone());
    let b = SignalDedupeKey::new(id, key2, cmd);

    assert_ne!(a, b, "different wait_key should produce different keys");
}

#[test]
fn attack_dedupekey_different_command_different_hash() {
    let id = valid_instance_id();
    let key = WaitKey::parse("same-key").expect("valid");
    let cmd1 = IdempotencyKey::parse("cmd-a").expect("valid");
    let cmd2 = IdempotencyKey::parse("cmd-b").expect("valid");

    let a = SignalDedupeKey::new(id.clone(), key.clone(), cmd1);
    let b = SignalDedupeKey::new(id, key, cmd2);

    assert_ne!(a, b, "different command_id should produce different keys");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 23: DEDUPEKEY — Serde round-trip
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_dedupekey_serde_roundtrip() {
    let id = valid_instance_id();
    let key = WaitKey::parse("serde-dk").expect("valid");
    let cmd = IdempotencyKey::parse("cmd-serde").expect("valid");
    let original = SignalDedupeKey::new(id, key, cmd);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SignalDedupeKey = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 24: LINEAGESCOPE — Exhaustive match proves closed enum
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_lineagescope_exhaustive_match() {
    for scope in [LineageScope::EpochLocal, LineageScope::LineageWide] {
        match scope {
            LineageScope::EpochLocal => {
                assert!(scope.is_epoch_local());
                assert!(!scope.is_lineage_wide());
            }
            LineageScope::LineageWide => {
                assert!(scope.is_lineage_wide());
                assert!(!scope.is_epoch_local());
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 25: LINEAGESCOPE — is_epoch_local and is_lineage_wide are mutually exclusive
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_lineagescope_mutual_exclusion() {
    for scope in [LineageScope::EpochLocal, LineageScope::LineageWide] {
        assert_eq!(
            scope.is_epoch_local(),
            !scope.is_lineage_wide(),
            "BUG: is_epoch_local and is_lineage_wide must be mutually exclusive for {scope:?}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 26: WAITKEY — Hash and Eq for HashSet membership
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_hash_eq_for_hashset() {
    let key1 = WaitKey::parse("same-key").expect("valid");
    let key2 = WaitKey::parse("same-key").expect("valid");
    let key3 = WaitKey::parse("different-key").expect("valid");

    let mut set = HashSet::new();
    set.insert(key1);
    assert!(
        set.contains(&key2),
        "identical keys should match in HashSet"
    );
    assert!(!set.contains(&key3), "different keys should not match");
    set.insert(key2);
    assert_eq!(set.len(), 1, "identical keys should not duplicate");
    set.insert(key3);
    assert_eq!(set.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 27: WAITKEY — Hash and Eq for BTreeSet membership
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitkey_eq_for_sorted_grouping() {
    let a = WaitKey::parse("aaa").expect("valid");
    let b = WaitKey::parse("bbb").expect("valid");
    let c = WaitKey::parse("aaa").expect("valid");

    assert_ne!(a, b, "different keys should not be equal");
    assert_eq!(a, c, "same string should be equal");
    assert_ne!(b, a, "inequality should be symmetric");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 28: SIGNALADDRESS — Same instance, different lineage → different
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_signaladdress_different_lineage_different_address() {
    let lineage1 = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid");
    let lineage2 = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid");
    let instance = valid_instance_id();
    let key = WaitKey::parse("same").expect("valid");

    let a = SignalAddress::lineage_wide(lineage1, instance.clone(), key.clone());
    let b = SignalAddress::lineage_wide(lineage2, instance, key);

    assert_ne!(
        a, b,
        "different lineage_id should produce different addresses"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 29: WAITRECORD — Accessors return correct types and values
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_waitrecord_accessor_types() {
    let id = valid_instance_id();
    let key = WaitKey::parse("types-test").expect("valid");
    let record = WaitRecord::new(
        id.clone(),
        key.clone(),
        BufferPolicy::BufferMany,
        TimestampMs::try_from(12345u64).expect("valid"),
    )
    .expect("valid");

    let _: &InstanceId = record.instance_id();
    let _: &WaitKey = record.wait_key();
    let _: BufferPolicy = record.buffer_policy();
    let _: TimestampMs = record.registered_at();

    assert_eq!(record.instance_id().as_str(), id.as_str());
    assert_eq!(record.wait_key().as_str(), "types-test");
}
