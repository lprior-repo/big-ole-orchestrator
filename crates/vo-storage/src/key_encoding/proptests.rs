//! Property-based tests for canonical key encoding (ADR-020).
//!
//! These tests verify key ordering invariants using exhaustive proptest strategies:
//! - Event key lexicographic ordering matches sequence number ordering
//! - Timer key ordering preserves chronological fire-at ordering
//! - Lease key ordering by instance then step
//! - Dedupe key ordering invariants
//! - Effect key shares prefix with event key but sorts after
//! - Prefix scan correctness

#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use vo_types::{InstanceId, SequenceNumber, StepId};

use crate::key_encoding::{
    decode_dedupe_key, decode_effect_key, decode_event_key, decode_lease_key, decode_timer_key,
    encode_dedupe_key, encode_effect_key, encode_event_key, encode_instance_index_key_for_status,
    encode_lease_key, encode_timer_key, get_dedupe_key_prefix, get_event_key_prefix,
    get_lease_key_prefix_for_instance, get_timer_key_prefix_for_time,
};

fn arb_instance_id() -> impl Strategy<Value = InstanceId> {
    proptest::array::uniform16(proptest::num::u8::ANY).prop_map(InstanceId::from_bytes)
}

fn arb_sequence_number() -> impl Strategy<Value = SequenceNumber> {
    any::<u64>().prop_map(|n| SequenceNumber::try_from(n).unwrap())
}

fn arb_step_id() -> impl Strategy<Value = StepId> {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"
        .chars()
        .collect();
    proptest::collection::vec(proptest::sample::subsequence(chars, 1..50), 1..=1).prop_map(
        |chars| {
            let s: String = chars.into_iter().flatten().collect();
            StepId::parse(&s).unwrap()
        },
    )
}

proptest! {
    // ========================================================================
    // EVENT KEY INVARIANTS
    // ========================================================================

    /// EV-PROP-001: Event key encode/decode roundtrip preserves identity.
    #[test]
    fn event_key_roundtrip(id in arb_instance_id(), seq in arb_sequence_number()) {
        let key = encode_event_key(&id, seq);
        let (decoded_id, decoded_seq) = decode_event_key(&key).unwrap();
        prop_assert_eq!(decoded_id, id);
        prop_assert_eq!(decoded_seq, seq);
    }

    /// EV-PROP-002: Event key ordering preserves sequence number ordering
    /// (same instance, increasing sequence => increasing key).
    #[test]
    fn event_key_ordering_preserves_sequence(
        id in arb_instance_id(),
        seq1 in any::<u64>(),
        seq2 in any::<u64>(),
    ) {
        prop_assume!(seq1 < seq2);
        let key1 = encode_event_key(&id, SequenceNumber::try_from(seq1).unwrap());
        let key2 = encode_event_key(&id, SequenceNumber::try_from(seq2).unwrap());
        prop_assert!(key1 < key2, "seq {} < seq {} but key1 >= key2", seq1, seq2);
    }

    /// EV-PROP-003: Event key prefix scan correctness — key starts with instance prefix.
    #[test]
    fn event_key_prefix_scan(id in arb_instance_id(), seq in arb_sequence_number()) {
        let key = encode_event_key(&id, seq);
        let prefix = get_event_key_prefix(&id);
        prop_assert!(key.starts_with(&prefix), "event key should start with instance prefix");
    }

    /// EV-PROP-004: Different instances with same sequence ordered by instance bytes.
    /// Uses fixed IDs [0...] and [1...] which are known to be byte-ordered.
    #[test]
    fn event_key_instance_dominates_sequence(seq1 in any::<u64>(), seq2 in any::<u64>()) {
        let id1 = InstanceId::from_bytes([0u8; 16]);
        let id2 = InstanceId::from_bytes([1u8; 16]);
        let key1 = encode_event_key(&id1, SequenceNumber::try_from(seq1).unwrap());
        let key2 = encode_event_key(&id2, SequenceNumber::try_from(seq2).unwrap());
        prop_assert!(key1 < key2, "id [0...] < id [1...] should give key1 < key2 regardless of sequences");
    }

    /// EV-PROP-005: Event key length is always exactly 24 bytes.
    #[test]
    fn event_key_fixed_length(id in arb_instance_id(), seq in arb_sequence_number()) {
        let key = encode_event_key(&id, seq);
        prop_assert_eq!(key.len(), 24, "event key should be 24 bytes");
    }

    // ========================================================================
    // TIMER KEY INVARIANTS
    // ========================================================================

    /// TM-PROP-001: Timer key encode/decode roundtrip preserves identity.
    #[test]
    fn timer_key_roundtrip(id in arb_instance_id(), fire_at_ms in any::<u64>()) {
        let key = encode_timer_key(fire_at_ms, &id);
        let (decoded_ts, decoded_id) = decode_timer_key(&key).unwrap();
        prop_assert_eq!(decoded_ts, fire_at_ms);
        prop_assert_eq!(decoded_id, id);
    }

    /// TM-PROP-002: Timer key ordering preserves chronological ordering
    /// (earlier fire_at => smaller key).
    #[test]
    fn timer_key_ordering_preserves_chronology(
        id in arb_instance_id(),
        ts1 in any::<u64>(),
        ts2 in any::<u64>(),
    ) {
        prop_assume!(ts1 < ts2);
        let key1 = encode_timer_key(ts1, &id);
        let key2 = encode_timer_key(ts2, &id);
        prop_assert!(key1 < key2, "ts {} < ts {} but key1 >= key2", ts1, ts2);
    }

    /// TM-PROP-003: Timer key prefix scan correctness — key starts with timestamp prefix.
    #[test]
    fn timer_key_prefix_scan(id in arb_instance_id(), fire_at_ms in any::<u64>()) {
        let key = encode_timer_key(fire_at_ms, &id);
        let prefix = get_timer_key_prefix_for_time(fire_at_ms);
        prop_assert!(key.starts_with(&prefix), "timer key should start with timestamp prefix");
    }

    /// TM-PROP-004: Timer key length is always exactly 24 bytes.
    #[test]
    fn timer_key_fixed_length(id in arb_instance_id(), fire_at_ms in any::<u64>()) {
        let key = encode_timer_key(fire_at_ms, &id);
        prop_assert_eq!(key.len(), 24, "timer key should be 24 bytes");
    }

    /// TM-PROP-005: Same timestamp, different instances ordered by instance bytes.
    /// Uses fixed IDs [0...] and [1...] which are known to be byte-ordered.
    #[test]
    fn timer_key_instance_ordering_at_same_timestamp(fire_at_ms in any::<u64>()) {
        let id1 = InstanceId::from_bytes([0u8; 16]);
        let id2 = InstanceId::from_bytes([1u8; 16]);
        let key1 = encode_timer_key(fire_at_ms, &id1);
        let key2 = encode_timer_key(fire_at_ms, &id2);
        prop_assert!(key1 < key2, "id [0...] < id [1...] should give key1 < key2 at same timestamp");
    }

    // ========================================================================
    // LEASE KEY INVARIANTS
    // ========================================================================

    /// LS-PROP-001: Lease key encode/decode roundtrip preserves identity.
    #[test]
    fn lease_key_roundtrip(id in arb_instance_id(), step in arb_step_id()) {
        let key = encode_lease_key(&id, &step);
        let (decoded_id, decoded_step) = decode_lease_key(&key).unwrap();
        prop_assert_eq!(decoded_id, id);
        prop_assert_eq!(decoded_step, step);
    }

    /// LS-PROP-002: Lease key prefix scan correctness — key starts with instance prefix.
    #[test]
    fn lease_key_prefix_scan(id in arb_instance_id(), step in arb_step_id()) {
        let key = encode_lease_key(&id, &step);
        let prefix = get_lease_key_prefix_for_instance(&id);
        prop_assert!(key.starts_with(&prefix), "lease key should start with instance prefix");
    }

    /// LS-PROP-003: Lease key ordering — same instance, ordered by step string.
    #[test]
    fn lease_key_step_ordering(id in arb_instance_id(), step1 in arb_step_id(), step2 in arb_step_id()) {
        prop_assume!(step1.as_str() < step2.as_str());
        let key1 = encode_lease_key(&id, &step1);
        let key2 = encode_lease_key(&id, &step2);
        prop_assert!(key1 < key2, "step1.as_str() < step2.as_str() should give key1 < key2 for same instance");
    }

    /// LS-PROP-004: Lease keys for different instances differ even with same step.
    /// Uses fixed IDs [0...] and [1...] which are known to be byte-ordered.
    #[test]
    fn lease_key_instance_is_differentiator(step in arb_step_id()) {
        let id1 = InstanceId::from_bytes([0u8; 16]);
        let id2 = InstanceId::from_bytes([1u8; 16]);
        let key1 = encode_lease_key(&id1, &step);
        let key2 = encode_lease_key(&id2, &step);
        prop_assert_ne!(key1, key2, "different instances should produce different lease keys");
        prop_assert!(key1 < key2, "id [0...] < id [1...] should give key1 < key2 for same step");
    }

    // ========================================================================
    // DEDUPE KEY INVARIANTS
    // ========================================================================

    /// DD-PROP-001: Dedupe key encode/decode roundtrip preserves identity.
    #[test]
    fn dedupe_key_roundtrip(key in "[a-zA-Z0-9_-]{0,256}") {
        let encoded = encode_dedupe_key(&key);
        let decoded = decode_dedupe_key(&encoded).unwrap();
        prop_assert_eq!(decoded, key);
    }

    /// DD-PROP-002: Dedupe key prefix equals full key for short keys.
    #[test]
    fn dedupe_key_prefix_equals_full_key(key in "[a-zA-Z0-9_-]{0,100}") {
        let encoded = encode_dedupe_key(&key);
        let prefix = get_dedupe_key_prefix(&key);
        prop_assert_eq!(prefix, encoded, "dedupe prefix should equal full key");
    }

    /// DD-PROP-003: Lexicographic ordering of dedupe keys matches string ordering.
    #[test]
    fn dedupe_key_ordering_matches_string_ordering(key1 in "[a-zA-Z0-9_-]{0,100}", key2 in "[a-zA-Z0-9_-]{0,100}") {
        prop_assume!(key1 < key2);
        let enc1 = encode_dedupe_key(&key1);
        let enc2 = encode_dedupe_key(&key2);
        prop_assert!(enc1 < enc2, "key1 < key2 should give enc1 < enc2");
    }

    // ========================================================================
    // EFFECT KEY INVARIANTS
    // ========================================================================

    /// EF-PROP-001: Effect key encode/decode roundtrip preserves identity.
    #[test]
    fn effect_key_roundtrip(id in arb_instance_id(), seq in arb_sequence_number()) {
        let key = encode_effect_key(&id, seq);
        let (decoded_id, decoded_seq) = decode_effect_key(&key).unwrap();
        prop_assert_eq!(decoded_id, id);
        prop_assert_eq!(decoded_seq, seq);
    }

    /// EF-PROP-002: Effect key length is always exactly 25 bytes.
    #[test]
    fn effect_key_fixed_length(id in arb_instance_id(), seq in arb_sequence_number()) {
        let key = encode_effect_key(&id, seq);
        prop_assert_eq!(key.len(), 25, "effect key should be 25 bytes");
    }

    /// EF-PROP-003: Effect key shares 24-byte prefix with event key, then has 0xFF.
    #[test]
    fn effect_key_shares_prefix_with_event_key(id in arb_instance_id(), seq in arb_sequence_number()) {
        let event_key = encode_event_key(&id, seq);
        let effect_key = encode_effect_key(&id, seq);
        prop_assert_eq!(&effect_key[0..24], &event_key[0..24], "effect and event share 24-byte prefix");
        prop_assert_eq!(effect_key[24], 0xFF, "effect key byte 24 should be 0xFF");
        prop_assert!(event_key < effect_key, "event key should sort before effect key");
    }

    // ========================================================================
    // INSTANCE INDEX KEY INVARIANTS
    // ========================================================================

    /// II-PROP-001: Instance index key status byte is primary sort dimension.
    #[test]
    fn instance_index_key_status_byte_ordering(id in arb_instance_id(), ts in any::<u64>()) {
        let key1 = encode_instance_index_key_for_status(1, ts, &id);
        let key2 = encode_instance_index_key_for_status(2, ts, &id);
        prop_assert!(key1 < key2, "status 1 < status 2 should give key1 < key2");
    }

    /// II-PROP-002: Instance index key created_at is secondary sort dimension.
    #[test]
    fn instance_index_key_timestamp_ordering(id in arb_instance_id()) {
        let key1 = encode_instance_index_key_for_status(1, 1000, &id);
        let key2 = encode_instance_index_key_for_status(1, 2000, &id);
        prop_assert!(key1 < key2, "earlier timestamp should sort before later");
    }

    // ========================================================================
    // CROSS-KEY INVARIANTS
    // ========================================================================

    /// CK-PROP-001: No collision between different key types for same entity.
    #[test]
    fn no_collision_between_key_types(id in arb_instance_id(), seq in arb_sequence_number()) {
        let event_key = encode_event_key(&id, seq);
        let effect_key = encode_effect_key(&id, seq);
        let timer_key = encode_timer_key(1000, &id);
        let dedupe_key = encode_dedupe_key("test-key");
        let lease_key = encode_lease_key(&id, &StepId::parse("step-1").unwrap());
        let instance_key = encode_instance_index_key_for_status(1, 1000, &id);

        prop_assert!(event_key != effect_key);
        prop_assert!(event_key != timer_key);
        prop_assert!(event_key != dedupe_key);
        prop_assert!(event_key != lease_key);
        prop_assert!(event_key != instance_key);
    }

    /// CK-PROP-002: Event and effect keys for same entity share prefix but sort separately.
    #[test]
    fn event_effect_key_prefix_and_sort(id in arb_instance_id(), seq in arb_sequence_number()) {
        let event_key = encode_event_key(&id, seq);
        let effect_key = encode_effect_key(&id, seq);
        prop_assert!(event_key.starts_with(&event_key[0..16]), "event key starts with 16-byte instance prefix");
        prop_assert!(effect_key.starts_with(&event_key[0..16]), "effect key shares instance prefix");
        prop_assert!(event_key < effect_key, "event key sorts before effect key");
    }
}
