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

    for seq in 0..1000u64 {
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

    for ts in (0..1000u64).rev() {
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
fn red_queen_dedupe_key_prefix_cannot_collision_with_event_key_prefix() {
    let id = min_instance_id();
    let dedupe_str = "events:00000000000000000000000001::1";
    let dedupe_key = encode_dedupe_key(dedupe_str);
    let dedupe_prefix = get_dedupe_key_prefix(dedupe_str);

    let event_key = encode_event_key(&id, SequenceNumber::try_from(1u64).unwrap());

    // Dedupe keys start with a length prefix (u16 be), not with instance_id bytes
    // This means they cannot collide with event keys which start with raw instance_id bytes
    assert_ne!(
        dedupe_prefix[0], event_key[0],
        "BUG: dedupe prefix and event key should not share first byte (length prefix vs raw id)"
    );
}

#[test]
fn red_queen_lease_key_prefix_is_instance_id_bytes() {
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
        16,
        "BUG: lease prefix should be exactly 16 bytes (instance_id)"
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

    // Dedupe starts with length prefix (0x00 0x0c for "test-dedupe" = 12 bytes)
    let dedupe_len = decode_u16_be(&dedupe_key[0..2]).unwrap();
    assert_eq!(dedupe_len, 12);

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
        InstanceId::parse("00000000000000000000000000").unwrap(),
        InstanceId::from_bytes([0u8; 16]),
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

// ========================================================================
// DIMENSION: sequence-number-edge-cases — SequenceNumber boundaries
// ========================================================================

#[test]
fn red_queen_sequence_number_zero_is_valid() {
    let seq = SequenceNumber::try_from(0u64).unwrap();
    let encoded = encode_sequence_number(seq);
    let decoded = decode_sequence_number(&encoded).unwrap();
    assert_eq!(decoded, seq, "BUG: sequence 0 roundtrip failed");
}

#[test]
fn red_queen_sequence_number_max_is_valid() {
    let seq = SequenceNumber::try_from(u64::MAX).unwrap();
    let encoded = encode_sequence_number(seq);
    let decoded = decode_sequence_number(&encoded).unwrap();
    assert_eq!(decoded, seq, "BUG: sequence MAX roundtrip failed");
}

#[test]
fn red_queen_event_key_with_sequence_zero_and_max() {
    let id = min_instance_id();
    let seq0 = SequenceNumber::try_from(0u64).unwrap();
    let seq_max = SequenceNumber::try_from(u64::MAX).unwrap();

    let key0 = encode_event_key(&id, seq0);
    let key_max = encode_event_key(&id, seq_max);

    assert!(key0 < key_max, "BUG: seq 0 should be < seq MAX");
    assert_eq!(
        key0.len(),
        24,
        "BUG: event key with seq 0 should be 24 bytes"
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
fn red_queen_step_id_encoding_handles_empty_string() {
    let step = StepId::parse("").unwrap();
    let encoded = encode_step_id(&step);
    let decoded = decode_step_id(&encoded).unwrap();
    assert_eq!(decoded.as_str(), "", "BUG: empty step_id roundtrip failed");
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
fn red_queen_step_id_encoding_handles_special_characters() {
    let special_steps = vec![
        "step-with-dashes",
        "step.with.dots",
        "step_with_underscores",
        "step:with:colons",
        "step/with/slashes",
        "💝-emoji-step",
        "日本語-step",
    ];

    for step_str in special_steps {
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
fn red_queen_lease_key_rejects_multiple_delimiters_in_step_id() {
    let id = min_instance_id();
    // This tests that step IDs with :: are preserved (not treated as extra delimiter)
    let step = StepId::parse("outer::inner::deep").unwrap();
    let key = encode_lease_key(&id, &step);
    let key_str = String::from_utf8(key.clone()).unwrap();

    // The entire step_id including internal :: should be preserved
    assert!(
        key_str.ends_with("outer::inner::deep"),
        "BUG: step_id with internal :: not preserved in lease key: {}",
        key_str
    );

    let (decoded_id, decoded_step) = decode_lease_key(&key).unwrap();
    assert_eq!(
        decoded_id, id,
        "BUG: lease key instance_id roundtrip failed"
    );
    assert_eq!(
        decoded_step.as_str(),
        "outer::inner::deep",
        "BUG: step_id with :: roundtrip failed"
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
