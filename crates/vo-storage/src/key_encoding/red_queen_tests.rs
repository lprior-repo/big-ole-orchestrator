//! Red Queen adversarial tests for canonical key encoding (ADR-020).
//!
//! These tests verify:
//! - Collision resistance across key types
//! - Sort order preservation (lexicographic ordering matches numeric ordering)
//! - Encoding/decoding roundtrips
//! - Unicode and edge case handling
//! - Prefix scan safety (no ambiguous prefixes)

use proptest::prelude::*;
use vo_types::{InstanceId, SequenceNumber, StepId};

use crate::key_encoding::{
    decode_dedupe_key, decode_effect_key, decode_event_key, decode_instance_id, decode_lease_key,
    decode_length_prefixed, decode_sequence_number, decode_step_id, decode_timer_key,
    decode_u16_be, decode_u64_be, encode_dedupe_key, encode_effect_key, encode_event_key,
    encode_instance_id, encode_instance_index_key_for_status, encode_lease_key,
    encode_length_prefixed, encode_sequence_number, encode_step_id, encode_timer_key,
    encode_u16_be, encode_u64_be, encode_u64_be as encode_ts, get_dedupe_key_prefix,
    get_event_key_prefix, get_lease_key_prefix_for_instance, get_timer_key_prefix_for_time,
    KeyEncodingError,
};

fn min_instance_id() -> InstanceId {
    InstanceId::parse("00000000000000000000000001").unwrap()
}

fn max_instance_id() -> InstanceId {
    InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap()
}

fn arb_step_id() -> impl Strategy<Value = StepId> {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"
        .chars()
        .collect();
    prop::sample::subsequence(chars, 1..50).prop_map(|chars| {
        let s: String = chars.into_iter().collect();
        StepId::parse(&s).unwrap()
    })
}

fn arb_sequence_number() -> impl Strategy<Value = SequenceNumber> {
    any::<u64>().prop_map(|n| SequenceNumber::try_from(n).unwrap())
}

fn arb_timestamp() -> impl Strategy<Value = u64> {
    any::<u64>()
}

#[test]
fn red_queen_u64_be_no_collision_across_full_range() {
    let mut seen = std::collections::HashSet::new();
    let samples: Vec<u64> = (0..10000)
        .chain([u64::MAX, u64::MAX - 1, u64::MAX - 2, u64::MAX - 3])
        .collect();

    for val in samples {
        let encoded = encode_u64_be(val);
        assert!(
            seen.insert(encoded.to_vec()),
            "Collision detected for u64 value {}: {:?}",
            val,
            encoded
        );
    }
}

#[test]
fn red_queen_u64_be_lexicographic_matches_numeric() {
    let pairs: Vec<(u64, u64)> = vec![
        (0, 1),
        (1, 2),
        (u64::MAX - 1, u64::MAX),
        (0, u64::MAX),
        (100, 101),
        (0x7FFF_FFFF_FFFF_FFFF, 0x8000_0000_0000_0000),
    ];

    for (a, b) in pairs {
        let enc_a = encode_u64_be(a);
        let enc_b = encode_u64_be(b);
        assert!(
            enc_a < enc_b,
            "Lexicographic ordering failed: {} < {} but {:?} >= {:?}",
            a,
            b,
            enc_a,
            enc_b
        );
    }
}

#[test]
fn red_queen_u64_be_roundtrip_exhaustive() {
    let test_values = [
        0u64,
        1,
        42,
        127,
        128,
        255,
        256,
        1000,
        u16::MAX as u64,
        u32::MAX as u64,
        u64::MAX - 1,
        u64::MAX,
    ];

    for val in test_values {
        let encoded = encode_u64_be(val);
        let decoded = decode_u64_be(&encoded).unwrap();
        assert_eq!(
            decoded, val,
            "Roundtrip failed for {}: encoded to {:?}, decoded to {}",
            val, encoded, decoded
        );
    }
}

#[test]
fn red_queen_length_prefixed_no_collision_different_lengths() {
    let inputs = vec![
        b"a".to_vec(),
        b"aa".to_vec(),
        b"aaa".to_vec(),
        b"aaaa".to_vec(),
        b"ab".to_vec(),
        b"abc".to_vec(),
        b"abcd".to_vec(),
    ];

    let mut seen = std::collections::HashSet::new();
    for input in inputs {
        let encoded = encode_length_prefixed(&input);
        assert!(
            seen.insert(encoded.clone()),
            "Collision detected for {:?}: {:?}",
            input,
            encoded
        );
    }
}

#[test]
fn red_queen_length_prefixed_truncated_rejected() {
    let test_cases = vec![
        vec![0],
        vec![0, 5],
        vec![0, 5, b'h'],
        vec![0, 5, b'h', b'e'],
        vec![],
    ];

    for truncated in test_cases {
        let result = decode_length_prefixed(&truncated);
        assert!(
            result.is_err(),
            "Should reject truncated data: {:?}",
            truncated
        );
    }
}

#[test]
fn red_queen_instance_id_roundtrip() {
    let ids = vec![
        min_instance_id(),
        max_instance_id(),
        InstanceId::parse("00000000000000000000000002").unwrap(),
        InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZY").unwrap(),
    ];

    for id in ids {
        let encoded = encode_instance_id(&id).unwrap();
        let decoded = decode_instance_id(&encoded).unwrap();
        assert_eq!(decoded, id, "InstanceId roundtrip failed for {}", id);
    }
}

#[test]
fn red_queen_instance_id_no_collision_across_ids() {
    let ids = vec![
        min_instance_id(),
        max_instance_id(),
        InstanceId::parse("00000000000000000000000002").unwrap(),
        InstanceId::parse("0000000000000000000000000A").unwrap(),
        InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZY").unwrap(),
    ];

    let mut seen = std::collections::HashSet::new();
    for id in ids {
        let encoded = encode_instance_id(&id).unwrap();
        assert!(
            seen.insert(encoded.to_vec()),
            "Collision detected for InstanceId {}: {:?}",
            id,
            encoded
        );
    }
}

#[test]
fn red_queen_event_key_prefix_scan_no_collision() {
    let id1 = InstanceId::parse("00000000000000000000000001").unwrap();
    let id2 = InstanceId::parse("00000000000000000000000002").unwrap();

    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let seq2 = SequenceNumber::try_from(2u64).unwrap();

    let key1 = encode_event_key(&id1, seq1);
    let key2 = encode_event_key(&id1, seq2);
    let key3 = encode_event_key(&id2, seq1);
    let key4 = encode_event_key(&id2, seq2);

    let prefix1 = get_event_key_prefix(&id1);
    let prefix2 = get_event_key_prefix(&id2);

    assert!(
        key1.starts_with(&prefix1) && key2.starts_with(&prefix1),
        "Event keys should start with instance prefix"
    );
    assert!(
        key3.starts_with(&prefix2) && key4.starts_with(&prefix2),
        "Event keys should start with instance prefix"
    );
    assert!(
        key1 < key3,
        "Different instance IDs should produce ordered keys"
    );
    assert!(
        key1 < key2,
        "Same instance, different sequences should be ordered"
    );

    let all_keys = vec![key1.clone(), key2.clone(), key3.clone(), key4.clone()];
    let mut sorted = all_keys.clone();
    sorted.sort();
    assert_eq!(all_keys, sorted, "Event keys should be sortable");
}

#[test]
fn red_queen_event_key_lexicographic_ordering() {
    let id = min_instance_id();

    let test_cases: Vec<(u64, u64)> = vec![(1, 2), (u64::MAX - 1, u64::MAX), (100, 200)];

    for (seq_a, seq_b) in test_cases {
        let key_a = encode_event_key(&id, SequenceNumber::try_from(seq_a).unwrap());
        let key_b = encode_event_key(&id, SequenceNumber::try_from(seq_b).unwrap());
        assert!(
            key_a < key_b,
            "Lexicographic should match numeric: seq {} < seq {} but key {:?} >= {:?}",
            seq_a,
            seq_b,
            key_a,
            key_b
        );
    }
}

#[test]
fn red_queen_timer_key_chronological_ordering() {
    let id = min_instance_id();

    let test_cases: Vec<(u64, u64)> = vec![
        (0, 1),
        (1, 2),
        (u64::MAX - 1, u64::MAX),
        (1000, 1001),
        (0x7FFF_FFFF_FFFF_FFFF, 0x8000_0000_0000_0000),
    ];

    for (ts_a, ts_b) in test_cases {
        let key_a = encode_timer_key(ts_a, &id);
        let key_b = encode_timer_key(ts_b, &id);
        assert!(
            key_a < key_b,
            "Timer keys should be chronologically ordered: ts {} < ts {} but key {:?} >= {:?}",
            ts_a,
            ts_b,
            key_a,
            key_b
        );
    }
}

#[test]
fn red_queen_timer_key_prefix_scan() {
    let id = min_instance_id();
    let ts = 1000u64;

    let key = encode_timer_key(ts, &id);
    let prefix = get_timer_key_prefix_for_time(ts);

    assert!(
        key.starts_with(&prefix),
        "Timer key should start with timestamp prefix"
    );
}

#[test]
fn red_queen_lease_key_prefix_scan_safety() {
    let id1 = min_instance_id();
    let id2 = InstanceId::parse("00000000000000000000000002").unwrap();

    let step1 = StepId::parse("step-a").unwrap();
    let step2 = StepId::parse("step-b").unwrap();

    let key1 = encode_lease_key(&id1, &step1);
    let key2 = encode_lease_key(&id1, &step2);
    let key3 = encode_lease_key(&id2, &step1);

    let prefix1 = get_lease_key_prefix_for_instance(&id1);
    let prefix2 = get_lease_key_prefix_for_instance(&id2);

    assert!(
        key1.starts_with(&prefix1) && key2.starts_with(&prefix1),
        "Lease keys for same instance should share prefix"
    );
    assert!(
        key3.starts_with(&prefix2),
        "Lease key for different instance should have different prefix"
    );

    let all_keys = vec![key1.clone(), key2.clone(), key3.clone()];
    let mut sorted = all_keys.clone();
    sorted.sort();
    assert_eq!(all_keys, sorted, "Lease keys should be sortable");
}

#[test]
fn red_queen_lease_key_no_collision() {
    let id1 = min_instance_id();
    let id2 = InstanceId::parse("00000000000000000000000002").unwrap();

    let step1 = StepId::parse("a").unwrap();
    let step2 = StepId::parse("b").unwrap();

    let key1 = encode_lease_key(&id1, &step1);
    let key2 = encode_lease_key(&id1, &step2);
    let key3 = encode_lease_key(&id2, &step1);

    assert_ne!(
        key1, key2,
        "Different step IDs should produce different keys"
    );
    assert_ne!(
        key1, key3,
        "Different instance IDs should produce different keys"
    );
}

#[test]
fn red_queen_lease_key_roundtrip() {
    let test_cases = vec![
        (
            InstanceId::parse("00000000000000000000000001").unwrap(),
            StepId::parse("step-1").unwrap(),
        ),
        (
            InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap(),
            StepId::parse("a").unwrap(),
        ),
    ];

    for (id, step) in test_cases {
        let encoded = encode_lease_key(&id, &step);
        let (decoded_id, decoded_step) = decode_lease_key(&encoded).unwrap();
        assert_eq!(decoded_id, id, "InstanceId roundtrip failed for lease key");
        assert_eq!(decoded_step, step, "StepId roundtrip failed for lease key");
    }
}

#[test]
fn red_queen_dedupe_key_roundtrip() {
    let keys = vec![
        "simple",
        "a",
        "with-special-chars-123",
        "very-long-idempotency-key-that-is-quite-long",
        "key with spaces",
        "unicode: \u{4e2d}\u{6587}",
    ];

    for key_str in keys {
        let encoded = encode_dedupe_key(key_str);
        let decoded = decode_dedupe_key(&encoded).unwrap();
        assert_eq!(
            decoded, key_str,
            "Dedupe key roundtrip failed for '{}'",
            key_str
        );
    }
}

#[test]
fn red_queen_dedupe_key_prefix_equals_full_key() {
    let key_str = "short-key";
    let encoded = encode_dedupe_key(key_str);
    let prefix = get_dedupe_key_prefix(key_str);
    assert_eq!(
        prefix, encoded,
        "Dedupe prefix should equal full key for short keys"
    );
}

#[test]
fn red_queen_effect_key_differs_from_event_key() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let event_key = encode_event_key(&id, seq);
    let effect_key = encode_effect_key(&id, seq);

    assert_ne!(event_key, effect_key, "Effect and event keys should differ");
    assert_eq!(
        &effect_key[0..24],
        &event_key[0..24],
        "Effect and event keys should share prefix (instance + sequence)"
    );
    assert_eq!(effect_key[24], 0xFF, "Effect key should have 0xFF marker");
}

#[test]
fn red_queen_instance_index_key_ordering() {
    let id = min_instance_id();

    let key1 = encode_instance_index_key_for_status(1, 1000, &id);
    let key2 = encode_instance_index_key_for_status(2, 1000, &id);
    let key3 = encode_instance_index_key_for_status(1, 2000, &id);

    assert!(
        key1 < key2,
        "Different statuses should produce ordered keys"
    );
    assert!(
        key1 < key3,
        "Different timestamps should produce ordered keys"
    );
}

#[test]
fn red_queen_step_id_roundtrip() {
    let steps = vec![
        StepId::parse("step-1").unwrap(),
        StepId::parse("a").unwrap(),
        StepId::parse("my-very-long-step-id-12345").unwrap(),
    ];

    for step in steps {
        let encoded = encode_step_id(&step);
        let decoded = decode_step_id(&encoded).unwrap();
        assert_eq!(decoded, step, "StepId roundtrip failed for {}", step);
    }
}

#[test]
fn red_queen_step_id_rejects_corrupt_data() {
    let test_cases = vec![
        vec![],
        vec![0, 5, b'h', b'e'],
        vec![0, 5, b'h', b'e', b'l', b'l'],
    ];

    for data in test_cases {
        let result = decode_step_id(&data);
        assert!(
            result.is_err(),
            "Should reject corrupt step data: {:?}",
            data
        );
    }
}

#[test]
fn red_queen_sequence_number_roundtrip() {
    let values = vec![1u64, 42, u64::MAX - 1, u64::MAX];

    for val in values {
        let seq = SequenceNumber::try_from(val).unwrap();
        let encoded = encode_sequence_number(seq);
        let decoded = decode_sequence_number(&encoded).unwrap();
        assert_eq!(decoded, seq, "SequenceNumber roundtrip failed for {}", val);
    }
}

#[test]
fn red_queen_unicode_in_dedupe_keys() {
    let unicode_keys = vec![
        "hello",
        "\u{4e2d}\u{6587}",
        "\u{1F600}",
        "mix\u{4e2d}\u{6587}text",
        "emoji: \u{1F600}\u{1F601}",
        "rtl: \u{0627}\u{0644}\u{0639}\u{0634}\u{0631}\u{0628}\u{064a}\u{0629}",
    ];

    for key_str in unicode_keys {
        let encoded = encode_dedupe_key(key_str);
        let decoded = decode_dedupe_key(&encoded);
        assert!(
            decoded.is_ok(),
            "Should handle unicode key '{}': {:?}",
            key_str,
            encoded
        );
        if let Ok(decoded) = decoded {
            assert_eq!(
                decoded, key_str,
                "Unicode roundtrip failed for '{}'",
                key_str
            );
        }
    }
}

#[test]
fn red_queen_u16_be_roundtrip() {
    let values = vec![0u16, 1, 42, 256, 0x7FFF, 0x8000, 0xFFFF];

    for val in values {
        let encoded = encode_u16_be(val);
        let decoded = decode_u16_be(&encoded).unwrap();
        assert_eq!(decoded, val, "u16 roundtrip failed for {}", val);
    }
}

#[test]
fn red_queen_prefix_scan_boundary_no_collision() {
    let id1 = InstanceId::parse("00000000000000000000000001").unwrap();
    let id2 = InstanceId::parse("00000000000000000000000002").unwrap();

    let prefix1 = get_event_key_prefix(&id1);
    let prefix2 = get_event_key_prefix(&id2);

    assert!(
        !prefix1.starts_with(&prefix2) && !prefix2.starts_with(&prefix1),
        "Different instance prefixes should not overlap"
    );

    let key1 = encode_event_key(&id1, SequenceNumber::try_from(1u64).unwrap());
    let key2 = encode_event_key(&id2, SequenceNumber::try_from(1u64).unwrap());

    assert!(
        !key1.starts_with(&prefix2) && !key2.starts_with(&prefix1),
        "Keys should not match wrong prefix"
    );
}

#[test]
fn red_queen_lease_key_adversarial_prefix_collision() {
    let id = min_instance_id();

    let step1 = StepId::parse("a").unwrap();
    let step2 = StepId::parse("b").unwrap();
    let step3 = StepId::parse("c").unwrap();

    let key1 = encode_lease_key(&id, &step1);
    let key2 = encode_lease_key(&id, &step2);

    assert_ne!(
        key1, key2,
        "Lease keys with different step content should not collide"
    );
}

#[test]
fn red_queen_all_partitions_have_unique_prefixes() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let ts = 1000u64;

    let event_key = encode_event_key(&id, seq);
    let timer_key = encode_timer_key(ts, &id);
    let lease_key = encode_lease_key(&id, &StepId::parse("step-1").unwrap());
    let dedupe_key = encode_dedupe_key("test");
    let effect_key = encode_effect_key(&id, seq);

    let mut keys = vec![
        ("event", event_key),
        ("timer", timer_key),
        ("lease", lease_key),
        ("dedupe", dedupe_key),
        ("effect", effect_key),
    ];

    keys.sort_by(|a, b| a.1.cmp(&b.1));
    keys.dedup_by(|a, b| a.1 == b.1);

    assert_eq!(
        keys.len(),
        5,
        "All partition keys should be unique, found duplicates"
    );
}

#[test]
fn red_queen_max_length_idempotency_key() {
    let max_key = "a".repeat(1024);
    let encoded = encode_dedupe_key(&max_key);
    let decoded = decode_dedupe_key(&encoded).unwrap();
    assert_eq!(
        decoded, max_key,
        "Max length idempotency key roundtrip should work"
    );
}

#[test]
fn red_queen_u64_max_value_encoding() {
    let val = u64::MAX;
    let encoded = encode_u64_be(val);
    let decoded = decode_u64_be(&encoded).unwrap();
    assert_eq!(decoded, val);

    let val = u64::MAX - 1;
    let encoded = encode_u64_be(val);
    let decoded = decode_u64_be(&encoded).unwrap();
    assert_eq!(decoded, val);
}

#[test]
fn red_queen_event_key_boundary_sequences() {
    let id = min_instance_id();

    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();
    let seq_max_minus = SequenceNumber::try_from(u64::MAX - 1).unwrap();

    let key_max = encode_event_key(&id, seq_max);
    let key_max_minus = encode_event_key(&id, seq_max_minus);

    assert!(
        key_max_minus < key_max,
        "Max boundary should preserve ordering"
    );
}

#[test]
fn red_queen_decode_error_propagation() {
    assert!(matches!(
        decode_u64_be(&[0u8; 7]),
        Err(KeyEncodingError::InvalidLength { .. })
    ));
    assert!(matches!(
        decode_u16_be(&[0u8; 1]),
        Err(KeyEncodingError::InvalidLength { .. })
    ));
    assert!(matches!(
        decode_instance_id(&[0u8; 15]),
        Err(KeyEncodingError::InvalidLength { .. })
    ));
    assert!(matches!(
        decode_event_key(&[0u8; 23]),
        Err(KeyEncodingError::InvalidLength { .. })
    ));
    assert!(matches!(
        decode_timer_key(&[0u8; 23]),
        Err(KeyEncodingError::InvalidLength { .. })
    ));
}
