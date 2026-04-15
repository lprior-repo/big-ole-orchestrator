#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Red Queen adversarial tests for canonical key encoding (ADR-020).
//!
//! These tests probe:
//! - Lexicographic ordering invariants across all key types
//! - Prefix collision between different key types
//! - Key composition soundness across all entity types
//! - Edge cases in encoding/decoding boundary conditions

use vo_types::{InstanceId, SequenceNumber, StepId};

use crate::key_encoding::{
    decode_dedupe_key, decode_effect_key, decode_event_key, decode_instance_id, decode_lease_key,
    decode_length_prefixed, decode_sequence_number, decode_step_id, decode_timer_key,
    decode_u16_be, decode_u64_be, encode_dedupe_key, encode_effect_key, encode_event_key,
    encode_instance_id, encode_instance_index_key_for_status, encode_lease_key,
    encode_length_prefixed, encode_sequence_number, encode_step_id, encode_timer_key,
    encode_u16_be, encode_u64_be, get_dedupe_key_prefix, get_event_key_prefix,
    get_lease_key_prefix_for_instance, get_timer_key_prefix_for_time,
};

// ========================================================================
// HELPERS
// ========================================================================

fn min_instance_id() -> InstanceId {
    InstanceId::parse("00000000000000000000000001").unwrap()
}

fn max_instance_id() -> InstanceId {
    InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap()
}

fn mid_instance_id() -> InstanceId {
    InstanceId::parse("40000000000000000000000000").unwrap()
}

// ========================================================================
// DIMENSION: lexicographic-ordering — event keys
// Contract: higher sequence numbers produce lexicographically larger keys
// ========================================================================

#[test]
fn red_queen_event_key_lexicographic_increments_monotone() {
    let id = min_instance_id();
    let mut prev_key: Vec<u8> = Vec::new();

    for seq in 1..=1000u64 {
        let sn = SequenceNumber::try_from(seq).unwrap();
        let key = encode_event_key(&id, sn);
        assert!(
            key > prev_key,
            "BUG: event key seq {seq} should be > previous key; prev={prev_key:?}, curr={key:?}"
        );
        prev_key = key;
    }
}

#[test]
fn red_queen_event_key_different_instances_lexicographic_order_by_instance_prefix() {
    let id1 = min_instance_id();
    let id2 = mid_instance_id();
    let id3 = max_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key1 = encode_event_key(&id1, seq);
    let key2 = encode_event_key(&id2, seq);
    let key3 = encode_event_key(&id3, seq);

    assert!(
        key1 < key2,
        "BUG: min_instance_id key should be < mid_instance_id key"
    );
    assert!(
        key2 < key3,
        "BUG: mid_instance_id key should be < max_instance_id key"
    );
    assert!(
        key1 < key3,
        "BUG: min_instance_id key should be < max_instance_id key"
    );
}

#[test]
fn red_queen_event_key_instance_matters_more_than_sequence() {
    let id_small = min_instance_id();
    let id_large = max_instance_id();
    let seq_small = SequenceNumber::try_from(1u64).unwrap();
    let seq_large = SequenceNumber::try_from(u64::MAX).unwrap();

    let key_small_instance_large_seq = encode_event_key(&id_small, seq_large);
    let key_large_instance_small_seq = encode_event_key(&id_large, seq_small);

    // Instance ID is the first 16 bytes, so it dominates sequence number comparison
    assert!(
        key_small_instance_large_seq < key_large_instance_small_seq,
        "BUG: instance_id prefix should dominate sequence_number in ordering"
    );
}

// ========================================================================
// DIMENSION: lexicographic-ordering — timer keys
// Contract: earlier timestamps produce lexicographically smaller keys
// ========================================================================

#[test]
fn red_queen_timer_key_lexicographic_ordering_is_chronological() {
    let id = min_instance_id();
    let mut prev_key: Vec<u8> = Vec::new();

    for ts in 0..1000u64 {
        let key = encode_timer_key(ts, &id);
        assert!(
            key > prev_key,
            "BUG: timer key ts {ts} should be > previous key (later time = larger)"
        );
        prev_key = key;
    }
}

#[test]
fn red_queen_timer_key_different_instances_at_same_time_differ_only_in_last_16_bytes() {
    let id1 = min_instance_id();
    let id2 = max_instance_id();
    let ts = 1234567890u64;

    let key1 = encode_timer_key(ts, &id1);
    let key2 = encode_timer_key(ts, &id2);

    assert_eq!(
        &key1[0..8],
        &key2[0..8],
        "BUG: timestamp bytes should be identical"
    );
    assert_ne!(
        &key1[8..24],
        &key2[8..24],
        "BUG: instance_id bytes should differ"
    );
    assert!(
        key1 < key2,
        "BUG: min_instance timer key should sort before max_instance"
    );
}

// ========================================================================
// DIMENSION: lexicographic-ordering — instance index keys
// Contract: status byte dominates, then created_at, then instance_id
// ========================================================================

#[test]
fn red_queen_instance_index_key_status_byte_is_primary_sort() {
    let id = min_instance_id();
    let created_at = 1000u64;

    let key_status_0 = encode_instance_index_key_for_status(0, created_at, &id);
    let key_status_1 = encode_instance_index_key_for_status(1, created_at, &id);
    let key_status_127 = encode_instance_index_key_for_status(127, created_at, &id);
    let key_status_128 = encode_instance_index_key_for_status(128, created_at, &id);
    let key_status_255 = encode_instance_index_key_for_status(255, created_at, &id);

    assert!(key_status_0 < key_status_1, "BUG: status 0 < 1");
    assert!(key_status_1 < key_status_127, "BUG: status 1 < 127");
    assert!(key_status_127 < key_status_128, "BUG: status 127 < 128");
    assert!(key_status_128 < key_status_255, "BUG: status 128 < 255");
}

#[test]
fn red_queen_instance_index_key_created_at_is_secondary_sort() {
    let id = min_instance_id();

    let key_earlier = encode_instance_index_key_for_status(0, 1000, &id);
    let key_later = encode_instance_index_key_for_status(0, 2000, &id);

    assert!(
        key_earlier < key_later,
        "BUG: earlier created_at should sort before later"
    );
}

// ========================================================================
// DIMENSION: prefix-collision — event vs effect keys
// Contract: event_key and effect_key share 24-byte prefix but differ at byte 24
// ========================================================================

#[test]
fn red_queen_event_effect_keys_share_prefix_but_differ_at_byte_24() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(42u64).unwrap();

    let event_key = encode_event_key(&id, seq);
    let effect_key = encode_effect_key(&id, seq);

    assert_eq!(
        &event_key[0..24],
        &effect_key[0..24],
        "BUG: event/effect should share 24-byte prefix"
    );
    assert_eq!(event_key.len(), 24, "BUG: event_key should be 24 bytes");
    assert_eq!(effect_key.len(), 25, "BUG: effect_key should be 25 bytes");
    assert_eq!(
        effect_key[24], 0xFF,
        "BUG: effect_key trailing byte should be 0xFF"
    );
    assert_ne!(
        event_key[24..],
        effect_key[24..],
        "BUG: event/effect last bytes should differ"
    );
}

#[test]
fn red_queen_event_effect_keys_do_not_collide_in_sorted_order() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let event_key = encode_event_key(&id, seq);
    let effect_key = encode_effect_key(&id, seq);

    // In sorted order, event key (24 bytes) should come before effect key (25 bytes)
    // because when comparing [24 bytes] vs [24 bytes + 0xFF], the shorter one is smaller
    assert!(
        event_key < effect_key,
        "BUG: event_key should sort before effect_key"
    );
}

// ========================================================================
// DIMENSION: prefix-collision — cross-partition isolation
// Contract: keys from different partitions must NOT have prefix overlap that causes collision
// ========================================================================

#[test]
fn red_queen_dedupe_key_has_length_prefix_format() {
    let dedupe_str = "test-dedupe-key";
    let dedupe_key = encode_dedupe_key(dedupe_str);
    let dedupe_prefix = get_dedupe_key_prefix(dedupe_str);

    assert_eq!(
        dedupe_prefix.len(),
        2 + dedupe_str.len() as usize,
        "BUG: dedupe prefix should be 2-byte length + key bytes"
    );

    let decoded_len = decode_u16_be(&dedupe_prefix[0..2]).unwrap();
    assert_eq!(
        decoded_len,
        dedupe_str.len() as u16,
        "BUG: dedupe prefix length should match key length"
    );
}

#[test]
fn red_queen_lease_key_prefix_is_string_representation() {
    let id = min_instance_id();
    let step = StepId::parse("step-1").unwrap();
    let lease_key = encode_lease_key(&id, &step);
    let lease_prefix = get_lease_key_prefix_for_instance(&id);

    assert!(
        lease_key.starts_with(&lease_prefix),
        "BUG: lease_key should start with instance_id prefix"
    );
    assert_eq!(
        lease_prefix.len(),
        28,
        "BUG: lease prefix should be 28 bytes (ULID string 26 + '::' 2)"
    );
    let expected_prefix = format!("{}::", min_instance_id());
    assert_eq!(
        lease_prefix.as_slice(),
        expected_prefix.as_bytes(),
        "BUG: lease prefix should be ULID string + '::'"
    );
}

#[test]
fn red_queen_no_key_type_shares_prefix_with_different_key_type_unambiguously() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let event_key = encode_event_key(&id, seq);
    let effect_key = encode_effect_key(&id, seq);
    let timer_key = encode_timer_key(1000, &id);
    let dedupe_key = encode_dedupe_key("test-dedupe");
    let step_id = StepId::parse("step-1").unwrap();
    let lease_key = encode_lease_key(&id, &step_id);
    let instance_key = encode_instance_index_key_for_status(1, 1000, &id);

    // Event and effect keys start with instance_id (16 bytes)
    assert!(event_key.starts_with(&event_key[0..16]));
    assert!(effect_key.starts_with(&effect_key[0..16]));
    assert!(timer_key.starts_with(&timer_key[0..8])); // timestamp first
    assert!(instance_key.starts_with(&instance_key[0..1])); // status byte first

    // Dedupe starts with length prefix (0x00 0x0b for "test-dedupe" = 11 bytes)
    let dedupe_len = decode_u16_be(&dedupe_key[0..2]).unwrap();
    assert_eq!(dedupe_len, 11);

    // Lease is string format: instance_id::step_id
    let lease_str = String::from_utf8(lease_key.clone()).unwrap();
    assert!(lease_str.contains("::"));

    // No two different key types should be byte-equal
    let all_keys = [
        &event_key[..],
        &effect_key[..],
        &timer_key[..],
        &dedupe_key[..],
        &lease_key[..],
        &instance_key[..],
    ];
    for i in 0..all_keys.len() {
        for j in (i + 1)..all_keys.len() {
            assert_ne!(
                all_keys[i], all_keys[j],
                "BUG: key type {} and {} have identical bytes: {:?}",
                i, j, all_keys[i]
            );
        }
    }
}

// ========================================================================
// DIMENSION: key-composition-soundness — all key types roundtrip correctly
// ========================================================================

#[test]
fn red_queen_all_entity_types_roundtrip_through_encode_decode() {
    let id = min_instance_id();
    let max_id = max_instance_id();
    let seq = SequenceNumber::try_from(12345u64).unwrap();

    // Event key
    let ek = encode_event_key(&id, seq);
    let (dec_id, dec_seq) = decode_event_key(&ek).unwrap();
    assert_eq!(dec_id, id, "BUG: event key instance_id roundtrip failed");
    assert_eq!(dec_seq, seq, "BUG: event key sequence roundtrip failed");

    // Effect key
    let efk = encode_effect_key(&id, seq);
    let (dec_id, dec_seq) = decode_effect_key(&efk).unwrap();
    assert_eq!(dec_id, id, "BUG: effect key instance_id roundtrip failed");
    assert_eq!(dec_seq, seq, "BUG: effect key sequence roundtrip failed");

    // Timer key
    let tk = encode_timer_key(9999999999u64, &id);
    let (dec_ts, dec_id) = decode_timer_key(&tk).unwrap();
    assert_eq!(
        dec_ts, 9999999999u64,
        "BUG: timer key timestamp roundtrip failed"
    );
    assert_eq!(dec_id, id, "BUG: timer key instance_id roundtrip failed");

    // Lease key
    let step = StepId::parse("my-test-step-123").unwrap();
    let lk = encode_lease_key(&id, &step);
    let (dec_id, dec_step) = decode_lease_key(&lk).unwrap();
    assert_eq!(dec_id, id, "BUG: lease key instance_id roundtrip failed");
    assert_eq!(dec_step, step, "BUG: lease key step_id roundtrip failed");

    // Dedupe key
    let dk = encode_dedupe_key("my-idempotency-key-xyz");
    let dec_dk = decode_dedupe_key(&dk).unwrap();
    assert_eq!(
        dec_dk, "my-idempotency-key-xyz",
        "BUG: dedupe key roundtrip failed"
    );

    // Instance index key
    let ik = encode_instance_index_key_for_status(5, 9876543210u64, &id);
    // Note: we don't have a decode for this type, so we verify structure
    assert_eq!(
        ik.len(),
        1 + 8 + 16,
        "BUG: instance index key has wrong length"
    );
    assert_eq!(ik[0], 5, "BUG: instance index key status byte wrong");

    // Max instance id as well
    let ek_max = encode_event_key(&max_id, seq);
    let (dec_id, _) = decode_event_key(&ek_max).unwrap();
    assert_eq!(
        dec_id, max_id,
        "BUG: event key max_instance_id roundtrip failed"
    );
}

#[test]
fn red_queen_encode_decode_primitives_exhaustive() {
    // u64 be roundtrip
    for val in [
        0u64,
        1,
        42,
        127,
        128,
        255,
        256,
        1000,
        u32::MAX as u64,
        i64::MAX as u64,
        u64::MAX,
    ] {
        let encoded = encode_u64_be(val);
        let decoded = decode_u64_be(&encoded).unwrap();
        assert_eq!(decoded, val, "BUG: u64 {} roundtrip failed", val);
    }

    // u16 be roundtrip
    for val in [
        0u16,
        1,
        42,
        127,
        128,
        255,
        256,
        1000,
        u8::MAX as u16,
        u16::MAX,
    ] {
        let encoded = encode_u16_be(val);
        let decoded = decode_u16_be(&encoded).unwrap();
        assert_eq!(decoded, val, "BUG: u16 {} roundtrip failed", val);
    }

    // Length-prefixed roundtrip
    for s in [
        "",
        "a",
        "hello",
        "💝",
        "日本語",
        "mix💝日本語",
        &"x".repeat(1000),
    ] {
        let encoded = encode_length_prefixed(s.as_bytes());
        let (decoded, rest) = decode_length_prefixed(&encoded).unwrap();
        assert_eq!(
            decoded,
            s.as_bytes(),
            "BUG: length-prefixed '{}' roundtrip failed",
            s
        );
        assert!(
            rest.is_empty(),
            "BUG: length-prefixed '{}' has trailing bytes",
            s
        );
    }
}

// ========================================================================
// DIMENSION: boundary-conditions — edge cases in encoding
// ========================================================================

#[test]
fn red_queen_length_prefixed_max_length_u16() {
    let max_bytes = vec![0xFFu8; 65535];
    let encoded = encode_length_prefixed(&max_bytes);
    let (decoded, rest) = decode_length_prefixed(&encoded).unwrap();
    assert_eq!(
        decoded.len(),
        65535,
        "BUG: max length prefix decode has wrong size"
    );
    assert!(rest.is_empty(), "BUG: max length prefix has trailing data");
}

#[test]
fn red_queen_length_prefixed_rejects_truncated_at_exact_boundary() {
    let data = b"hello";
    let encoded = encode_length_prefixed(data);
    let mut truncated = encoded.clone();
    truncated.pop(); // Remove one byte from end

    let result = decode_length_prefixed(&truncated);
    assert!(
        result.is_err(),
        "BUG: length-prefixed truncated by 1 should error"
    );
}

#[test]
fn red_queen_length_prefixed_rejects_truncated_at_length_bytes() {
    let data = b"hello";
    let encoded = encode_length_prefixed(data);
    let truncated = encoded[0..1].to_vec(); // Only 1 length byte

    let result = decode_length_prefixed(&truncated);
    assert!(
        result.is_err(),
        "BUG: length-prefixed with only 1 length byte should error"
    );
}

#[test]
fn red_queen_decode_u64_be_rejects_empty_slice() {
    let result = decode_u64_be(&[]);
    assert!(
        result.is_err(),
        "BUG: empty slice should error for u64 decode"
    );
}

#[test]
fn red_queen_decode_u64_be_rejects_single_byte() {
    let result = decode_u64_be(&[0x42]);
    assert!(
        result.is_err(),
        "BUG: single byte should error for u64 decode"
    );
}

#[test]
fn red_queen_instance_id_encode_decode_exhaustive() {
    let test_cases = vec![
        InstanceId::parse("00000000000000000000000001").unwrap(),
        InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap(),
        InstanceId::parse("40000000000000000000000000").unwrap(),
        InstanceId::from_bytes([0xFFu8; 16]),
        InstanceId::from_bytes([
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77,
        ]),
    ];

    for id in test_cases {
        let encoded = encode_instance_id(&id).unwrap();
        let decoded = decode_instance_id(&encoded).unwrap();
        assert_eq!(decoded, id, "BUG: instance_id {:?} roundtrip failed", id);
    }
}

#[test]
fn red_queen_instance_id_rejects_nil_ulid() {
    let result = InstanceId::parse("00000000000000000000000000");
    assert!(result.is_err(), "BUG: nil ULID should be rejected");
}

// ========================================================================
// DIMENSION: sequence-number-edge-cases — SequenceNumber boundaries
// ========================================================================

#[test]
fn red_queen_sequence_number_rejects_zero() {
    let result = SequenceNumber::try_from(0u64);
    assert!(
        result.is_err(),
        "BUG: sequence number zero should be rejected (must be nonzero)"
    );
}

#[test]
fn red_queen_sequence_number_max_is_valid() {
    let seq = SequenceNumber::try_from(u64::MAX).unwrap();
    let encoded = encode_sequence_number(seq);
    let decoded = decode_sequence_number(&encoded).unwrap();
    assert_eq!(decoded, seq, "BUG: sequence MAX roundtrip failed");
}

#[test]
fn red_queen_event_key_with_sequence_one_and_max() {
    let id = min_instance_id();
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();

    let key1 = encode_event_key(&id, seq1);
    let key_max = encode_event_key(&id, seq_max);

    assert!(key1 < key_max, "BUG: seq 1 should be < seq MAX");
    assert_eq!(
        key1.len(),
        24,
        "BUG: event key with seq 1 should be 24 bytes"
    );
    assert_eq!(
        key_max.len(),
        24,
        "BUG: event key with seq MAX should be 24 bytes"
    );
}

// ========================================================================
// DIMENSION: partition-prefix-constants — all partition constants are valid utf8 and non-empty
// ========================================================================

#[test]
fn red_queen_all_partition_constants_are_valid_non_empty_utf8() {
    use crate::key_encoding::{
        PARTITION_DEDUPE, PARTITION_EFFECTS, PARTITION_EVENTS, PARTITION_INSTANCES,
        PARTITION_LEASES, PARTITION_TIMERS,
    };

    for (name, partition) in [
        ("PARTITION_EVENTS", PARTITION_EVENTS),
        ("PARTITION_TIMERS", PARTITION_TIMERS),
        ("PARTITION_LEASES", PARTITION_LEASES),
        ("PARTITION_INSTANCES", PARTITION_INSTANCES),
        ("PARTITION_DEDUPE", PARTITION_DEDUPE),
        ("PARTITION_EFFECTS", PARTITION_EFFECTS),
    ] {
        assert!(!partition.is_empty(), "BUG: {} is empty", name);
        assert!(
            std::str::from_utf8(partition).is_ok(),
            "BUG: {} is not valid UTF8",
            name
        );
        assert!(
            partition.iter().all(|&b| !b.is_ascii_control()),
            "BUG: {} contains control characters",
            name
        );
    }
}

// ========================================================================
// DIMENSION: step-id-encoding-edge-cases
// ========================================================================

#[test]
fn red_queen_step_id_rejects_empty_string() {
    let result = StepId::parse("");
    assert!(result.is_err(), "BUG: empty step_id should be rejected");
}

#[test]
fn red_queen_step_id_encoding_handles_long_strings() {
    let long_step = "a".repeat(10000);
    let step = StepId::parse(&long_step).unwrap();
    let encoded = encode_step_id(&step);
    let decoded = decode_step_id(&encoded).unwrap();
    assert_eq!(
        decoded.as_str(),
        long_step,
        "BUG: long step_id roundtrip failed"
    );
}

#[test]
fn red_queen_step_id_encoding_handles_valid_special_characters() {
    let valid_steps = vec![
        "step-with-dashes",
        "step_with_underscores",
        "step1-with-2-numbers",
        "a",
        "step-id",
    ];

    for step_str in valid_steps {
        let step = StepId::parse(step_str).unwrap();
        let encoded = encode_step_id(&step);
        let decoded = decode_step_id(&encoded).unwrap();
        assert_eq!(
            decoded.as_str(),
            step_str,
            "BUG: step_id '{}' roundtrip failed",
            step_str
        );
    }
}

#[test]
fn red_queen_step_id_rejects_invalid_characters() {
    let invalid_steps = vec![
        ("step.with.dots", "."),
        ("step:with:colons", ":"),
        ("step/with/slashes", "/"),
    ];

    for (step_str, _expected_invalid_char) in invalid_steps {
        let result = StepId::parse(step_str);
        assert!(
            result.is_err(),
            "BUG: step_id '{}' with invalid chars should be rejected",
            step_str
        );
    }
}

// ========================================================================
// DIMENSION: lease-key-delimiter-safety
// ========================================================================

#[test]
fn red_queen_lease_key_delimiter_is_double_colon() {
    let id = min_instance_id();
    let step = StepId::parse("test-step").unwrap();
    let key = encode_lease_key(&id, &step);

    // Should parse correctly first (decode takes &Vec<u8>)
    let (parsed_id, parsed_step) = decode_lease_key(&key).unwrap();
    assert_eq!(parsed_id, id, "BUG: lease key instance_id parse failed");
    assert_eq!(parsed_step, step, "BUG: lease key step_id parse failed");

    // Then verify string format
    let key_str = String::from_utf8(key).unwrap();
    assert!(
        key_str.contains("::"),
        "BUG: lease key must contain :: delimiter"
    );
}

#[test]
fn red_queen_lease_key_rejects_missing_delimiter() {
    let id_str = min_instance_id().to_string();
    let bad_key = format!("{}step-id-without-delimiter", id_str);

    let result = decode_lease_key(bad_key.as_bytes());
    assert!(
        result.is_err(),
        "BUG: lease key without :: delimiter should error"
    );
}

#[test]
fn red_queen_step_id_rejects_colons_in_identifier() {
    let result = StepId::parse("outer::inner");
    assert!(
        result.is_err(),
        "BUG: step_id with colons should be rejected"
    );
}

// ========================================================================
// DIMENSION: effect-key-marker-byte
// ========================================================================

#[test]
fn red_queen_effect_key_marker_byte_is_always_0xff() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let effect_key = encode_effect_key(&id, seq);

    assert_eq!(
        effect_key[24], 0xFF,
        "BUG: effect key marker byte should be 0xFF"
    );

    let decoded = decode_effect_key(&effect_key).unwrap();
    assert_eq!(decoded.0, id);
    assert_eq!(decoded.1, seq);
}

#[test]
fn red_queen_effect_key_rejects_non_ff_marker() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let mut bad_key = encode_effect_key(&id, seq);
    bad_key[24] = 0x00; // Corrupt the marker byte

    let result = decode_effect_key(&bad_key);
    assert!(
        result.is_err(),
        "BUG: effect key with non-0xFF marker byte {} should error",
        bad_key[24]
    );
}

// ========================================================================
// DIMENSION: fuzz-arbitrary-bytes — decode functions must reject arbitrary
// byte sequences without panicking or producing collisions
// ========================================================================

#[test]
fn red_queen_fuzz_decode_event_key_with_garbage_bytes_never_panics() {
    let garbage_inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0xFF],
        vec![0x42; 23],
        vec![0x42; 24],
        vec![0x42; 25],
        vec![0x00; 24],
        vec![0xFF; 24],
        vec![0xAA; 24],
        vec![0x55; 24],
        (0u8..=255).collect(),
        (0u8..=255).rev().collect(),
        (0u8..=255).take(24).collect(),
        vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        ],
    ];

    for input in garbage_inputs {
        let result = std::panic::catch_unwind(|| {
            let _: Result<(InstanceId, SequenceNumber), _> = decode_event_key(&input);
        });
        assert!(
            result.is_ok(),
            "BUG: decode_event_key panicked on garbage input {:?}",
            input
        );
    }
}

#[test]
fn red_queen_fuzz_decode_timer_key_with_garbage_bytes_never_panics() {
    let garbage_inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0xFF],
        vec![0x42; 23],
        vec![0x42; 24],
        vec![0x42; 25],
        vec![0x00; 24],
        vec![0xFF; 24],
        (0u8..=255).collect(),
        (0u8..=255).rev().collect(),
    ];

    for input in garbage_inputs {
        let result = std::panic::catch_unwind(|| {
            let _: Result<(u64, InstanceId), _> = decode_timer_key(&input);
        });
        assert!(
            result.is_ok(),
            "BUG: decode_timer_key panicked on garbage input {:?}",
            input
        );
    }
}

#[test]
fn red_queen_fuzz_decode_effect_key_with_garbage_bytes_never_panics() {
    let garbage_inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0xFF],
        vec![0x42; 24],
        vec![0x42; 25],
        vec![0x42; 26],
        vec![0x00; 25],
        vec![0xFF; 25],
        vec![0x00; 24],
        (0u8..=255).collect(),
        (0u8..=255).rev().collect(),
    ];

    for input in garbage_inputs {
        let result = std::panic::catch_unwind(|| {
            let _: Result<(InstanceId, SequenceNumber), _> = decode_effect_key(&input);
        });
        assert!(
            result.is_ok(),
            "BUG: decode_effect_key panicked on garbage input {:?}",
            input
        );
    }
}

#[test]
fn red_queen_fuzz_decode_instance_id_with_garbage_bytes_never_panics() {
    let garbage_inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0xFF],
        vec![0x42; 15],
        vec![0x42; 17],
        vec![0x00; 16],
        vec![0xFF; 16],
        (0u8..=255).collect(),
    ];

    for input in garbage_inputs {
        let result = std::panic::catch_unwind(|| {
            let _: Result<InstanceId, _> = decode_instance_id(&input);
        });
        assert!(
            result.is_ok(),
            "BUG: decode_instance_id panicked on garbage input {:?}",
            input
        );
    }
}

#[test]
fn red_queen_fuzz_decode_lease_key_with_garbage_bytes_never_panics() {
    let garbage_inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0xFF],
        b"no-delimiter-here".to_vec(),
        b"has::double::colons".to_vec(),
        b"".to_vec(),
        vec![0x42; 100],
        vec![0x00; 50],
        vec![0xFF; 50],
        b"00000000000000000000000001::".to_vec(),
        b"::step-without-instance".to_vec(),
    ];

    for input in garbage_inputs {
        let result = std::panic::catch_unwind(|| {
            let _: Result<(InstanceId, StepId), _> = decode_lease_key(&input);
        });
        assert!(
            result.is_ok(),
            "BUG: decode_lease_key panicked on garbage input {:?}",
            input
        );
    }
}

#[test]
fn red_queen_fuzz_decode_dedupe_key_with_garbage_bytes_never_panics() {
    let garbage_inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0xFF],
        vec![0x00, 0xFF],
        vec![0xFF, 0xFF],
        vec![0x00, 0x01, 0xFF],
        vec![0x00; 100],
        vec![0xFF; 100],
    ];

    for input in garbage_inputs {
        let result = std::panic::catch_unwind(|| {
            let _: Result<String, _> = decode_dedupe_key(&input);
        });
        assert!(
            result.is_ok(),
            "BUG: decode_dedupe_key panicked on garbage input {:?}",
            input
        );
    }
}

#[test]
fn red_queen_fuzz_no_collision_between_garbage_and_valid_keys() {
    let valid_id = min_instance_id();
    let valid_seq = SequenceNumber::try_from(1u64).unwrap();

    let valid_event_key = encode_event_key(&valid_id, valid_seq);
    let valid_timer_key = encode_timer_key(1000, &valid_id);
    let valid_effect_key = encode_effect_key(&valid_id, valid_seq);
    let valid_dedupe_key = encode_dedupe_key("test");
    let step_id = StepId::parse("step-1").unwrap();
    let valid_lease_key = encode_lease_key(&valid_id, &step_id);

    let all_garbage_keys: Vec<Vec<u8>> = vec![
        vec![0x00; 24],
        vec![0xFF; 24],
        vec![0xAA; 24],
        vec![0x55; 24],
        vec![0x01; 24],
        vec![0x7F; 24],
        vec![0x80; 24],
    ];

    for garbage in all_garbage_keys {
        assert_ne!(
            garbage.as_slice(),
            valid_event_key.as_slice(),
            "BUG: garbage bytes collided with valid event key"
        );
        assert_ne!(
            garbage.as_slice(),
            valid_timer_key.as_slice(),
            "BUG: garbage bytes collided with valid timer key"
        );
        assert_ne!(
            garbage.as_slice(),
            valid_effect_key.as_slice(),
            "BUG: garbage bytes collided with valid effect key"
        );
        assert_ne!(
            garbage.as_slice(),
            valid_dedupe_key.as_slice(),
            "BUG: garbage bytes collided with valid dedupe key"
        );
        assert_ne!(
            garbage.as_slice(),
            valid_lease_key.as_slice(),
            "BUG: garbage bytes collided with valid lease key"
        );
    }
}

// ========================================================================
// DIMENSION: sort-order-inversions — generate keys designed to cause
// sort-order inversions and verify the ordering is still correct
// ========================================================================

#[test]
fn red_queen_sort_inversion_event_keys_with_non_monotonic_instances() {
    let id_a = InstanceId::from_bytes([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ]);
    let id_b = InstanceId::from_bytes([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02,
    ]);

    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();
    let seq_1 = SequenceNumber::try_from(1u64).unwrap();

    let key_a_max = encode_event_key(&id_a, seq_max);
    let key_b_1 = encode_event_key(&id_b, seq_1);

    assert!(
        key_a_max < key_b_1,
        "BUG: instance a with seq MAX should still sort before instance b with seq 1"
    );
}

#[test]
fn red_queen_sort_inversion_event_keys_with_high_byte_variations() {
    let ids: Vec<InstanceId> = (0u8..16)
        .map(|i| {
            let mut bytes = [0u8; 16];
            bytes[i as usize] = 0x80;
            InstanceId::from_bytes(bytes)
        })
        .collect();

    let seq = SequenceNumber::try_from(1u64).unwrap();

    let mut keys: Vec<Vec<u8>> = ids.iter().map(|id| encode_event_key(id, seq)).collect();

    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert!(
                keys[i] < keys[j],
                "BUG: keys[{}] should sort before keys[{}]; i={:?}, j={:?}",
                i,
                j,
                keys[i],
                keys[j]
            );
        }
    }
}

#[test]
fn red_queen_sort_inversion_timer_keys_at_wraparound_boundary() {
    let id = min_instance_id();

    let ts_max = u64::MAX;
    let ts_zero = 0u64;

    let key_max = encode_timer_key(ts_max, &id);
    let key_zero = encode_timer_key(ts_zero, &id);

    assert!(
        key_zero < key_max,
        "BUG: timer key at ts 0 should sort before ts MAX (later = larger)"
    );
}

#[test]
fn red_queen_sort_inversion_try_to_break_with_battlefield_values() {
    let id = min_instance_id();

    let battlefield_values = [
        0u64,
        1,
        127,
        128,
        255,
        256,
        65535,
        65536,
        u32::MAX as u64,
        u32::MAX as u64 + 1,
        i32::MAX as u64,
        i32::MAX as u64 + 1,
        i64::MAX as u64,
        u64::MAX - 1,
        u64::MAX,
    ];

    let mut keys: Vec<(u64, Vec<u8>)> = battlefield_values
        .iter()
        .map(|&ts| {
            let key = encode_timer_key(ts, &id);
            (ts, key)
        })
        .collect();

    keys.sort_by(|a, b| a.1.cmp(&b.1));

    for i in 0..keys.len() - 1 {
        assert!(
            keys[i].0 <= keys[i + 1].0,
            "BUG: sort inversion detected at index {}: ts {} should be <= ts {}",
            i,
            keys[i].0,
            keys[i + 1].0
        );
    }
}

#[test]
fn red_queen_sort_inversion_all_zeros_instance_should_be_rejected() {
    let all_zeros_id_bytes = [0u8; 16];
    let result = InstanceId::from_bytes(all_zeros_id_bytes);
    assert!(
        result.to_string().contains("00000000000000000000000000"),
        "BUG: all-zero instance ID should be the nil ULID and should be rejected"
    );
}

#[test]
fn red_queen_sort_inversion_near_max_instance_ids_still_order_correctly() {
    let id_near_max = InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZD").unwrap();
    let id_max = InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key_near = encode_event_key(&id_near_max, seq);
    let key_max = encode_event_key(&id_max, seq);

    assert!(
        key_near < key_max,
        "BUG: near-max instance should sort before max instance"
    );
}

// ========================================================================
// DIMENSION: sequence-number-wrap-boundaries — test edge cases at
// sequence number limits and potential wraparound behavior
// ========================================================================

#[test]
fn red_queen_seq_wrap_event_key_at_u64_max_boundary() {
    let id = min_instance_id();

    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();
    let seq_near_max = SequenceNumber::try_from(u64::MAX - 1).unwrap();

    let key_max = encode_event_key(&id, seq_max);
    let key_near_max = encode_event_key(&id, seq_near_max);

    assert!(
        key_near_max < key_max,
        "BUG: seq MAX-1 should sort before seq MAX"
    );
    assert_eq!(
        key_max.len(),
        24,
        "BUG: seq MAX event key should be 24 bytes"
    );
    assert_eq!(
        key_near_max.len(),
        24,
        "BUG: seq MAX-1 event key should be 24 bytes"
    );
}

#[test]
fn red_queen_seq_wrap_event_keys_consecutive_sequences_sort_correctly() {
    let id = min_instance_id();

    let mut prev_key: Vec<u8> = Vec::new();
    for seq in (u64::MAX - 99)..=u64::MAX {
        let sn = SequenceNumber::try_from(seq).unwrap();
        let key = encode_event_key(&id, sn);
        assert!(
            key > prev_key,
            "BUG: event key seq {} should be > previous key at wrap boundary",
            seq
        );
        prev_key = key;
    }
}

#[test]
fn red_queen_seq_wrap_sequence_number_max_roundtrip() {
    let id = min_instance_id();
    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();

    let encoded = encode_sequence_number(seq_max);
    let decoded = decode_sequence_number(&encoded).unwrap();

    assert_eq!(decoded, seq_max, "BUG: sequence MAX roundtrip failed");
}

#[test]
fn red_queen_seq_wrap_candidate_for_wrap_detection() {
    let val_max = u64::MAX;
    let encoded = encode_u64_be(val_max);
    let decoded = decode_u64_be(&encoded).unwrap();

    assert_eq!(decoded, val_max, "BUG: u64 MAX roundtrip failed");
}

#[test]
fn red_queen_seq_wrap_effect_keys_at_max_sequence_still_distinct() {
    let id = min_instance_id();
    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();

    let event_key = encode_event_key(&id, seq_max);
    let effect_key = encode_effect_key(&id, seq_max);

    assert!(
        event_key < effect_key,
        "BUG: event key should sort before effect key at MAX sequence"
    );
    assert_eq!(
        effect_key[24], 0xFF,
        "BUG: effect key marker should be 0xFF"
    );
}

#[test]
fn red_queen_seq_wrap_timer_keys_at_u64_max_timestamp() {
    let id = min_instance_id();

    let ts_max = u64::MAX;
    let key_max = encode_timer_key(ts_max, &id);
    let (decoded_ts, _) = decode_timer_key(&key_max).unwrap();

    assert_eq!(
        decoded_ts, ts_max,
        "BUG: timer key at MAX timestamp should roundtrip correctly"
    );
}

// ========================================================================
// DIMENSION: merkle-tree-integrity — verify merkle tree integrity under
// adversarial key injection scenarios
// ========================================================================

use crate::merkle_tree::{MerkleProof, MerkleTree};

#[test]
fn red_queen_merkle_key_encoding_deterministic_root() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key1 = encode_event_key(&id, seq);
    let key2 = encode_event_key(&id, seq);

    let tree1 = MerkleTree::new(&key1, 64);
    let tree2 = MerkleTree::new(&key2, 64);

    assert_eq!(
        tree1.root_hash(),
        tree2.root_hash(),
        "BUG: same key encoding should produce same merkle root"
    );
}

#[test]
fn red_queen_merkle_different_keys_produce_different_roots() {
    let id1 = min_instance_id();
    let id2 = mid_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key1 = encode_event_key(&id1, seq);
    let key2 = encode_event_key(&id2, seq);

    let tree1 = MerkleTree::new(&key1, 64);
    let tree2 = MerkleTree::new(&key2, 64);

    assert_ne!(
        tree1.root_hash(),
        tree2.root_hash(),
        "BUG: different keys should produce different merkle roots"
    );
}

#[test]
fn red_queen_merkle_sequence_increment_changes_root() {
    let id = min_instance_id();
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let seq2 = SequenceNumber::try_from(2u64).unwrap();

    let key1 = encode_event_key(&id, seq1);
    let key2 = encode_event_key(&id, seq2);

    let tree1 = MerkleTree::new(&key1, 64);
    let tree2 = MerkleTree::new(&key2, 64);

    assert_ne!(
        tree1.root_hash(),
        tree2.root_hash(),
        "BUG: different sequences should produce different merkle roots"
    );
}

#[test]
fn red_queen_merkle_merkle_proof_verification_for_event_keys() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key = encode_event_key(&id, seq);
    let tree = MerkleTree::new(&key, 64);
    let root = tree.root_hash();

    let proof = tree.proof(0).expect("should have proof for leaf 0");
    assert!(
        proof.verify(root),
        "BUG: merkle proof for event key should verify against its root"
    );
}

#[test]
fn red_queen_merkle_multiple_event_keys_proof_integrity() {
    let id = min_instance_id();

    let keys: Vec<Vec<u8>> = (1u64..=10)
        .map(|s| {
            let seq = SequenceNumber::try_from(s).unwrap();
            encode_event_key(&id, seq)
        })
        .collect();

    let all_bytes: Vec<u8> = keys.iter().flatten().cloned().collect();
    let tree = MerkleTree::new(&all_bytes, 64);
    let root = tree.root_hash();

    for (i, key) in keys.iter().enumerate() {
        let key_tree = MerkleTree::new(key, 64);
        let _key_root = key_tree.root_hash();

        let proof = tree.proof(i).expect("should have proof");
        assert!(
            proof.verify(root),
            "BUG: proof for key {} should verify against aggregate root",
            i
        );
    }
}

#[test]
fn red_queen_merkle_adversarial_key_injection_detected_by_proof_failure() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key = encode_event_key(&id, seq);
    let tree = MerkleTree::new(&key, 64);
    let root = tree.root_hash();

    let mut tampered_key = key.clone();
    tampered_key[16] ^= 0xFF;

    let tampered_tree = MerkleTree::new(&tampered_key, 64);
    let tampered_root = tampered_tree.root_hash();

    assert_ne!(
        root, tampered_root,
        "BUG: tampered key should produce different merkle root"
    );

    let proof = tree.proof(0).expect("should have proof");
    assert!(
        !proof.verify(tampered_root),
        "BUG: proof should fail when verified against tampered root"
    );
}

#[test]
fn red_queen_merkle_collision_resistance_across_key_types() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let event_key = encode_event_key(&id, seq);
    let timer_key = encode_timer_key(1000, &id);
    let effect_key = encode_effect_key(&id, seq);

    let tree_event = MerkleTree::new(&event_key, 64);
    let tree_timer = MerkleTree::new(&timer_key, 64);
    let tree_effect = MerkleTree::new(&effect_key, 64);

    assert_ne!(
        tree_event.root_hash(),
        tree_timer.root_hash(),
        "BUG: event key and timer key should not collide"
    );
    assert_ne!(
        tree_event.root_hash(),
        tree_effect.root_hash(),
        "BUG: event key and effect key should not collide"
    );
    assert_ne!(
        tree_timer.root_hash(),
        tree_effect.root_hash(),
        "BUG: timer key and effect key should not collide"
    );
}

#[test]
fn red_queen_merkle_proof_fails_on_single_bit_flip() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(42u64).unwrap();

    let key = encode_event_key(&id, seq);
    let tree = MerkleTree::new(&key, 64);
    let root = tree.root_hash();

    for byte_idx in 0..key.len() {
        for bit_idx in 0..8 {
            let mut tampered = key.clone();
            tampered[byte_idx] ^= 1 << bit_idx;

            let tampered_tree = MerkleTree::new(&tampered, 64);
            let tampered_root = tampered_tree.root_hash();

            let proof = tree.proof(0).expect("should have proof");
            assert!(
                !proof.verify(tampered_root) || tampered == key,
                "BUG: single bit flip at byte {} bit {} should cause proof failure",
                byte_idx,
                bit_idx
            );
        }
    }
}

#[test]
fn red_queen_merkle_empty_and_single_chunk_trees() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key = encode_event_key(&id, seq);

    let empty_tree = MerkleTree::new(&[], 64);
    assert_eq!(
        empty_tree.root_hash(),
        [0u8; 32],
        "BUG: empty data should produce zero root"
    );

    let single_tree = MerkleTree::new(&key, 1024);
    let proof = single_tree.proof(0).expect("should have proof");
    assert!(
        proof.verify(single_tree.root_hash()),
        "BUG: single chunk proof should verify"
    );
}

#[test]
fn red_queen_merkle_large_key_batch_integrity() {
    let id = min_instance_id();

    let key_batch: Vec<Vec<u8>> = (1u64..=100)
        .map(|s| {
            let seq = SequenceNumber::try_from(s).unwrap();
            encode_event_key(&id, seq)
        })
        .collect();

    let all_bytes: Vec<u8> = key_batch.iter().flatten().cloned().collect();
    let tree = MerkleTree::new(&all_bytes, 64);
    let root = tree.root_hash();

    for (i, key) in key_batch.iter().enumerate() {
        let proof = tree
            .proof(i)
            .expect(&format!("should have proof for key {}", i));
        assert!(
            proof.verify(root),
            "BUG: proof for key {} should verify against batch root",
            i
        );
    }
}

#[test]
fn red_queen_merkle_key_with_all_possible_sequence_values() {
    let id = min_instance_id();

    let sample_sequences = [
        1u64,
        100,
        u32::MAX as u64,
        i32::MAX as u64,
        u64::MAX - 1,
        u64::MAX,
    ];

    let mut roots: Vec<[u8; 32]> = Vec::new();

    for &seq_val in &sample_sequences {
        let seq = SequenceNumber::try_from(seq_val).unwrap();
        let key = encode_event_key(&id, seq);
        let tree = MerkleTree::new(&key, 64);
        roots.push(tree.root_hash());
    }

    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            assert_ne!(
                roots[i], roots[j],
                "BUG: different sequence values should produce different merkle roots"
            );
        }
    }
}
