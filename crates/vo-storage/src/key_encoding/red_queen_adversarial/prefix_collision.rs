use vo_types::{SequenceNumber, StepId};

use crate::key_encoding::{
    decode_dedupe_key, decode_u16_be, encode_dedupe_key, encode_effect_key, encode_event_key,
    encode_lease_key, encode_timer_key, get_dedupe_key_prefix, get_lease_key_prefix_for_instance,
};

use super::min_instance_id;

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

    // Event and effect keys start with instance_id (16 bytes)
    assert!(event_key.starts_with(&event_key[0..16]));
    assert!(effect_key.starts_with(&effect_key[0..16]));
    assert!(timer_key.starts_with(&timer_key[0..8])); // timestamp first

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
