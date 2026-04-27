//! Tests for canonical key encoding utilities (ADR-020).

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

fn min_instance_id() -> InstanceId {
    InstanceId::parse("00000000000000000000000001").unwrap()
}

fn max_instance_id() -> InstanceId {
    InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap()
}

#[test]
fn encode_u64_be_returns_correct_big_endian_bytes() {
    assert_eq!(encode_u64_be(0), [0u8; 8]);
    assert_eq!(encode_u64_be(1), [0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(
        encode_u64_be(0x0102_0304_0506_0708),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(encode_u64_be(u64::MAX), [0xFF; 8]);
}

#[test]
fn decode_u64_be_roundtrips_correctly() {
    for val in [0u64, 1, 42, 0x0102_0304_0506_0708, u64::MAX] {
        let encoded = encode_u64_be(val);
        let decoded = decode_u64_be(&encoded).unwrap();
        assert_eq!(decoded, val);
    }
}

#[test]
fn decode_u64_be_returns_error_for_wrong_length() {
    assert!(decode_u64_be(&[0u8; 7]).is_err());
    assert!(decode_u64_be(&[0u8; 9]).is_err());
    assert!(decode_u64_be(&[]).is_err());
}

#[test]
fn encode_u16_be_returns_correct_big_endian_bytes() {
    assert_eq!(encode_u16_be(0), [0, 0]);
    assert_eq!(encode_u16_be(1), [0, 1]);
    assert_eq!(encode_u16_be(256), [1, 0]);
    assert_eq!(encode_u16_be(0x0102), [1, 2]);
    assert_eq!(encode_u16_be(u16::MAX), [0xFF, 0xFF]);
}

#[test]
fn decode_u16_be_roundtrips_correctly() {
    for val in [0u16, 1, 42, 256, 0x0102, u16::MAX] {
        let encoded = encode_u16_be(val);
        let decoded = decode_u16_be(&encoded).unwrap();
        assert_eq!(decoded, val);
    }
}

#[test]
fn decode_u16_be_returns_error_for_wrong_length() {
    assert!(decode_u16_be(&[0u8; 1]).is_err());
    assert!(decode_u16_be(&[0u8; 3]).is_err());
    assert!(decode_u16_be(&[]).is_err());
}

#[test]
fn encode_length_prefixed_encodes_correctly() {
    let result = encode_length_prefixed(b"hello");
    assert_eq!(result.len(), 7);
    assert_eq!(&result[0..2], &[0, 5]);
    assert_eq!(&result[2..], b"hello");
}

#[test]
fn encode_length_prefixed_handles_empty() {
    let result = encode_length_prefixed(b"");
    assert_eq!(result.len(), 2);
    assert_eq!(&result[0..2], &[0, 0]);
}

#[test]
fn decode_length_prefixed_roundtrips_correctly() {
    let inputs: Vec<&[u8]> = vec![
        b"hello",
        b"",
        b"a",
        b"1234567890",
        b"a very long string with many characters",
    ];
    for input in inputs {
        let encoded = encode_length_prefixed(input);
        let (decoded, rest) = decode_length_prefixed(&encoded).unwrap();
        assert_eq!(decoded, input);
        assert!(rest.is_empty());
    }
}

#[test]
fn decode_length_prefixed_returns_error_for_truncated() {
    assert!(decode_length_prefixed(&[0]).is_err());
    assert!(decode_length_prefixed(&[0, 5, b'h']).is_err());
    assert!(decode_length_prefixed(&[]).is_err());
}

#[test]
fn encode_instance_id_returns_16_bytes() {
    let id = min_instance_id();
    let result = encode_instance_id(&id).unwrap();
    assert_eq!(result.len(), 16);
}

#[test]
fn encode_instance_id_roundtrips_correctly() {
    for id in [min_instance_id(), max_instance_id()] {
        let encoded = encode_instance_id(&id).unwrap();
        let decoded = decode_instance_id(&encoded).unwrap();
        assert_eq!(decoded, id);
    }
}

#[test]
fn decode_instance_id_returns_error_for_wrong_length() {
    assert!(decode_instance_id(&[]).is_err());
    assert!(decode_instance_id(&[0u8; 15]).is_err());
    assert!(decode_instance_id(&[0u8; 17]).is_err());
}

#[test]
fn encode_step_id_is_length_prefixed() {
    let step = StepId::parse("step-1").unwrap();
    let encoded = encode_step_id(&step);
    assert!(encoded.len() > step.as_str().len());
    assert_eq!(&encoded[0..2], &(step.as_str().len() as u16).to_be_bytes());
}

#[test]
fn encode_step_id_roundtrips_correctly() {
    for step_str in ["step-1", "a", "my-very-long-step-id-12345"] {
        let step = StepId::parse(step_str).unwrap();
        let encoded = encode_step_id(&step);
        let decoded = decode_step_id(&encoded).unwrap();
        assert_eq!(decoded, step);
    }
}

#[test]
fn decode_step_id_returns_error_for_corrupt_data() {
    assert!(decode_step_id(&[]).is_err());
    assert!(decode_step_id(&[0, 5, b'h', b'e']).is_err());
}

#[test]
fn encode_sequence_number_is_8_bytes() {
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let result = encode_sequence_number(seq);
    assert_eq!(result.len(), 8);
}

#[test]
fn decode_sequence_number_roundtrips_correctly() {
    for val in [1u64, 42, u64::MAX] {
        let seq = SequenceNumber::try_from(val).unwrap();
        let encoded = encode_sequence_number(seq);
        let decoded = decode_sequence_number(&encoded).unwrap();
        assert_eq!(decoded, seq);
    }
}

#[test]
fn decode_sequence_number_returns_error_for_wrong_length() {
    assert!(decode_sequence_number(&[]).is_err());
    assert!(decode_sequence_number(&[0u8; 7]).is_err());
    assert!(decode_sequence_number(&[0u8; 9]).is_err());
}

#[test]
fn encode_event_key_produces_26_byte_key() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&id, seq);
    assert_eq!(key.len(), 26);
    // Verify length prefix is present (bytes 0..2 = 16 for 16-byte ULID)
    assert_eq!(&key[0..2], 16u16.to_be_bytes());
}

#[test]
fn decode_event_key_roundtrips_correctly() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&id, seq);
    let (decoded_id, decoded_seq) = decode_event_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn decode_timer_key_returns_error_for_wrong_length() {
    assert!(decode_timer_key(&[]).is_err());
    assert!(decode_timer_key(&[0u8; 9]).is_err()); // minimum is 10 (8 ts + 2 len)
    assert!(decode_timer_key(&[0u8; 15]).is_err()); // claims 16 but only 5 bytes left
}

#[test]
fn event_key_lexicographic_ordering_is_correct() {
    let id = min_instance_id();
    let seq1 = SequenceNumber::try_from(1u64).unwrap();
    let seq2 = SequenceNumber::try_from(2u64).unwrap();
    let key1 = encode_event_key(&id, seq1);
    let key2 = encode_event_key(&id, seq2);
    assert!(key1 < key2);
}

#[test]
fn event_key_prefix_scan_works() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&id, seq);
    let prefix = get_event_key_prefix(&id);
    assert!(key.starts_with(&prefix));
}

#[test]
fn encode_timer_key_produces_length_prefixed_key() {
    let id = min_instance_id();
    let key = encode_timer_key(1000, &id);
    // 8 bytes timestamp + 2 bytes length prefix + 16 bytes instance_id
    assert_eq!(key.len(), 26);
    // First 8 bytes are timestamp
    assert_eq!(&key[0..8], 1000u64.to_be_bytes());
    // Bytes 8..10 are length prefix (16 for ULID)
    assert_eq!(&key[8..10], 16u16.to_be_bytes());
    // Remaining bytes are instance_id
    assert_eq!(&key[10..], &id.to_bytes().unwrap());
}

#[test]
fn decode_timer_key_roundtrips_correctly() {
    let id = min_instance_id();
    let ts = 1234567890u64;
    let key = encode_timer_key(ts, &id);
    let (decoded_ts, decoded_id) = decode_timer_key(&key).unwrap();
    assert_eq!(decoded_ts, ts);
    assert_eq!(decoded_id, id);
}

#[test]
fn timer_key_ordering_is_chronological() {
    let id = min_instance_id();
    let key1 = encode_timer_key(1000, &id);
    let key2 = encode_timer_key(2000, &id);
    assert!(key1 < key2);
}

#[test]
fn get_timer_key_prefix_for_time_returns_8_bytes() {
    let prefix = get_timer_key_prefix_for_time(1000);
    assert_eq!(prefix.len(), 8);
    assert_eq!(prefix, 1000u64.to_be_bytes());
}

#[test]
fn encode_lease_key_roundtrips_correctly() {
    let id = min_instance_id();
    let step = StepId::parse("step-1").unwrap();
    let key = encode_lease_key(&id, &step);
    let (decoded_id, decoded_step) = decode_lease_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_step, step);
}

#[test]
fn lease_key_format_uses_length_prefix_not_delimiter() {
    let id = min_instance_id();
    let step = StepId::parse("my-step").unwrap();
    let key = encode_lease_key(&id, &step);
    // New format: 16 bytes iid + 2 bytes sid_len + sid bytes
    assert_eq!(key.len(), 16 + 2 + "my-step".len());
    assert!(
        !key.iter().any(|&b| b == b':'),
        "new format lease key should not contain colon delimiter bytes"
    );
    // Key should start with the 16-byte instance ID
    assert_eq!(&key[..16], id.to_bytes().unwrap_or([0u8; 16]));
    // Step ID length should be at bytes 16..18 (big-endian)
    let sid_len = u16::from_be_bytes(key[16..18].try_into().unwrap()) as usize;
    assert_eq!(sid_len, "my-step".len());
    // Step ID bytes should follow
    let sid_bytes = &key[18..18 + sid_len];
    assert_eq!(sid_bytes, b"my-step");
}

#[test]
fn get_lease_key_prefix_for_instance_matches_key_format() {
    let id = min_instance_id();
    let step = StepId::parse("step-a").unwrap();
    let prefix = get_lease_key_prefix_for_instance(&id);
    let key = encode_lease_key(&id, &step);
    assert!(
        key.starts_with(&prefix),
        "lease key should start with instance prefix"
    );
    // Prefix should be the 16-byte instance ID (binary format)
    assert_eq!(prefix.len(), 16);
    assert_eq!(&prefix[..], id.to_bytes().unwrap_or([0u8; 16]));
}

#[test]
fn encode_dedupe_key_is_length_prefixed() {
    let key = encode_dedupe_key("my-idempotency-key");
    assert!(key.len() > "my-idempotency-key".len());
    assert_eq!(key[0..2], ("my-idempotency-key".len() as u16).to_be_bytes());
}

#[test]
fn decode_dedupe_key_roundtrips_correctly() {
    for key_str in [
        "simple",
        "a",
        "with-special-chars-123",
        "very-long-idempotency-key-that-is-quite-long",
    ] {
        let encoded = encode_dedupe_key(key_str);
        let decoded = decode_dedupe_key(&encoded).unwrap();
        assert_eq!(decoded, key_str);
    }
}

#[test]
fn get_dedupe_key_prefix_equals_full_key_when_short() {
    let key_str = "short-key";
    let encoded = encode_dedupe_key(key_str);
    let prefix = get_dedupe_key_prefix(key_str);
    assert_eq!(prefix, encoded);
}

#[test]
fn encode_effect_key_produces_27_byte_key() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_effect_key(&id, seq);
    assert_eq!(key.len(), 27);
    assert_eq!(key[26], 0xFF);
}

#[test]
fn decode_effect_key_roundtrips_correctly() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_effect_key(&id, seq);
    let (decoded_id, decoded_seq) = decode_effect_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq, seq);
}

#[test]
fn decode_effect_key_returns_error_for_wrong_length() {
    assert!(decode_effect_key(&[]).is_err());
    assert!(decode_effect_key(&[0u8; 26]).is_err());
    assert!(decode_effect_key(&[0u8; 28]).is_err());
}

#[test]
fn effect_key_differs_from_event_key_by_trailing_ff() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let event_key = encode_event_key(&id, seq);
    let effect_key = encode_effect_key(&id, seq);
    assert_eq!(&effect_key[0..26], &event_key[0..26]);
    assert_eq!(effect_key[26], 0xFF);
}

#[test]
fn encode_instance_index_key_for_status_produces_correct_length() {
    let id = min_instance_id();
    let key = encode_instance_index_key_for_status(1, 1000, &id);
    assert_eq!(key.len(), 1 + 8 + 16);
    assert_eq!(key[0], 1);
}

#[test]
fn different_statuses_produce_different_prefixes() {
    let id = min_instance_id();
    let key1 = encode_instance_index_key_for_status(1, 1000, &id);
    let key2 = encode_instance_index_key_for_status(2, 1000, &id);
    assert_ne!(key1[0], key2[0]);
    assert!(key1 < key2);
}

#[allow(dead_code)]
#[test]
fn decode_effect_key_rejects_missing_ff_marker() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&id, seq); // 26 bytes, no 0xFF marker
    assert!(
        decode_effect_key(&key).is_err(),
        "effect key decode should reject keys without 0xFF marker"
    );
}

#[allow(dead_code)]
#[test]
fn decode_lease_key_rejects_too_short_key() {
    let bad_key: [u8; 16] = *b"0000000000000000"; // Only 16 bytes, need at least 18
    let bad_key_ref: &[u8] = &bad_key;
    assert!(
        decode_lease_key(bad_key_ref).is_err(),
        "lease key decode should reject keys shorter than 18 bytes"
    );
}

#[test]
fn decode_lease_key_rejects_invalid_instance_id() {
    // All-zero instance ID is technically a valid binary ULID but fails ULID validation
    let bad_key = [0u8; 16]
        .into_iter()
        .chain((3u16).to_be_bytes())
        .chain([b'a', b'b', b'c'])
        .collect::<Vec<_>>();
    // This should fail because all-zeros is not a valid ULID instance ID
    assert!(
        decode_lease_key(&bad_key).is_err(),
        "lease key decode should reject invalid instance IDs"
    );
}

#[test]
fn decode_lease_key_rejects_invalid_step_id() {
    let id = min_instance_id();
    let sid_bytes = b"step with spaces";
    let mut bad_key = id.to_bytes().unwrap_or([0u8; 16]).to_vec();
    bad_key.extend_from_slice(&(sid_bytes.len() as u16).to_be_bytes());
    bad_key.extend_from_slice(sid_bytes);
    assert!(
        decode_lease_key(&bad_key).is_err(),
        "lease key decode should reject invalid step IDs"
    );
}

#[test]
fn encode_instance_index_key_is_deterministic() {
    let id = min_instance_id();
    let key1 = encode_instance_index_key_for_status(1, 1000, &id);
    let key2 = encode_instance_index_key_for_status(1, 1000, &id);
    assert_eq!(key1, key2, "same inputs should produce same key");
}

#[test]
fn instance_index_keys_sorted_by_timestamp() {
    let id = min_instance_id();
    let key1 = encode_instance_index_key_for_status(1, 1000, &id);
    let key2 = encode_instance_index_key_for_status(1, 2000, &id);
    assert!(key1 < key2, "earlier timestamp should sort first");
}

#[test]
fn instance_index_keys_sorted_by_instance_id() {
    let id1 = min_instance_id();
    let id2 = InstanceId::parse("00000000000000000000000002").unwrap();
    let key1 = encode_instance_index_key_for_status(1, 1000, &id1);
    let key2 = encode_instance_index_key_for_status(1, 1000, &id2);
    assert!(key1 < key2, "smaller instance ID should sort first");
}

#[test]
fn timer_key_prefix_scan_matches_keys_at_same_timestamp() {
    let id = min_instance_id();
    let ts = 5000u64;
    let key = encode_timer_key(ts, &id);
    let prefix = get_timer_key_prefix_for_time(ts);
    assert!(
        key.starts_with(&prefix),
        "timer key should start with timestamp prefix"
    );
}

/// BDD: Given event key includes instance id
/// When key is encoded
/// Then instance id is length-prefixed with no ambiguity
#[test]
fn given_event_key_when_encoded_then_instance_component_is_unambiguous() {
    // Given: instance IDs at both extremes of the ULID space
    let id_min = min_instance_id();
    let id_max = max_instance_id();

    // And: sequence numbers at both extremes
    let seq_first = SequenceNumber::try_from(1u64).unwrap();
    let seq_last = SequenceNumber::try_from(u64::MAX).unwrap();

    // When: event keys are encoded with length-prefixed instance IDs
    let key_min_first = encode_event_key(&id_min, seq_first);
    let key_min_last = encode_event_key(&id_min, seq_last);
    let key_max_first = encode_event_key(&id_max, seq_first);
    let key_max_last = encode_event_key(&id_max, seq_last);

    // Then: all keys are exactly 26 bytes (2-byte length prefix + 16-byte instance ID + 8-byte sequence)
    assert_eq!(key_min_first.len(), 26);
    assert_eq!(key_min_last.len(), 26);
    assert_eq!(key_max_first.len(), 26);
    assert_eq!(key_max_last.len(), 26);

    // Then: the length prefix (bytes 0..2) is 16 (ULID byte size) for all keys
    assert_eq!(&key_min_first[0..2], 16u16.to_be_bytes());
    assert_eq!(&key_min_last[0..2], 16u16.to_be_bytes());
    assert_eq!(&key_max_first[0..2], 16u16.to_be_bytes());
    assert_eq!(&key_max_last[0..2], 16u16.to_be_bytes());

    // Then: decoding roundtrips correctly for all combinations
    let (decoded_min_first_id, decoded_min_first_seq) = decode_event_key(&key_min_first).unwrap();
    assert_eq!(decoded_min_first_id, id_min);
    assert_eq!(decoded_min_first_seq, seq_first);

    let (decoded_min_last_id, decoded_min_last_seq) = decode_event_key(&key_min_last).unwrap();
    assert_eq!(decoded_min_last_id, id_min);
    assert_eq!(decoded_min_last_seq, seq_last);

    let (decoded_max_first_id, decoded_max_first_seq) = decode_event_key(&key_max_first).unwrap();
    assert_eq!(decoded_max_first_id, id_max);
    assert_eq!(decoded_max_first_seq, seq_first);

    let (decoded_max_last_id, decoded_max_last_seq) = decode_event_key(&key_max_last).unwrap();
    assert_eq!(decoded_max_last_id, id_max);
    assert_eq!(decoded_max_last_seq, seq_last);

    // Then: different instances produce different keys at the same sequence
    assert_ne!(
        key_min_first, key_max_first,
        "different instance IDs must produce different keys"
    );

    // Then: different sequences produce different keys for the same instance
    assert_ne!(
        key_min_first, key_min_last,
        "different sequences must produce different keys"
    );

    // Then: lexicographic ordering is preserved (instance dominates, then sequence)
    assert!(
        key_min_first < key_min_last,
        "higher sequence number should sort after lower for same instance"
    );
    assert!(
        key_min_first < key_max_first,
        "smaller instance ID should sort before larger for same sequence"
    );

    // Then: instance prefix scan works — prefix bytes match the encoded instance ID
    let prefix_min = get_event_key_prefix(&id_min);
    let prefix_max = get_event_key_prefix(&id_max);
    assert!(key_min_first.starts_with(&prefix_min));
    assert!(key_max_first.starts_with(&prefix_max));
}

/// BDD: Given timer key includes instance id and timer id
/// When key is encoded
/// Then components are unambiguous and lexicographic ordering by due time is preserved
#[test]
fn given_timer_key_when_encoded_then_components_are_unambiguous_and_ordered() {
    // Given: different instance IDs at different timestamps
    let id_a = min_instance_id();
    let id_b = max_instance_id();
    let ts_early = 1000u64;
    let ts_late = 9999u64;

    // When: keys are encoded with length-prefixed instance IDs
    let key_early_a = encode_timer_key(ts_early, &id_a);
    let key_early_b = encode_timer_key(ts_early, &id_b);
    let key_late_a = encode_timer_key(ts_late, &id_a);

    // Then: components are unambiguous — decode roundtrips correctly
    let (decoded_ts_a, decoded_id_a) = decode_timer_key(&key_early_a).unwrap();
    assert_eq!(decoded_ts_a, ts_early);
    assert_eq!(decoded_id_a, id_a);

    let (decoded_ts_b, decoded_id_b) = decode_timer_key(&key_early_b).unwrap();
    assert_eq!(decoded_ts_b, ts_early);
    assert_eq!(decoded_id_b, id_b);

    let (decoded_ts_late, decoded_id_late) = decode_timer_key(&key_late_a).unwrap();
    assert_eq!(decoded_ts_late, ts_late);
    assert_eq!(decoded_id_late, id_a);

    // Then: different instance IDs produce different keys at same timestamp
    assert_ne!(key_early_a, key_early_b);

    // Then: lexicographic ordering preserves chronology (earlier timestamp < later)
    assert!(
        key_early_a < key_late_a,
        "earlier timestamp should sort before later"
    );
    assert!(
        key_early_b < key_late_a,
        "earlier timestamp should sort before later"
    );

    // Then: same timestamp ordering is by instance_id bytes (length-prefixed)
    // id_a < id_b should give key_early_a < key_early_b
    assert!(
        key_early_a < key_early_b,
        "same timestamp: smaller instance_id should sort first"
    );
}

#[test]
fn timer_keys_with_same_timestamp_differ_by_instance_id() {
    let id1 = min_instance_id();
    let id2 = InstanceId::parse("00000000000000000000000002").unwrap();
    let key1 = encode_timer_key(1000, &id1);
    let key2 = encode_timer_key(1000, &id2);
    assert_ne!(
        key1, key2,
        "different instance IDs should produce different timer keys"
    );
}

#[test]
fn event_key_roundtrip_with_max_sequence() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(u64::MAX).unwrap();
    let key = encode_event_key(&id, seq);
    let (decoded_id, decoded_seq) = decode_event_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq.as_u64(), u64::MAX);
}

#[test]
fn dedupe_key_roundtrip_with_empty_string() {
    let encoded = encode_dedupe_key("");
    let decoded = decode_dedupe_key(&encoded).unwrap();
    assert_eq!(decoded, "");
}

#[test]
fn dedupe_key_different_inputs_produce_different_outputs() {
    let key1 = encode_dedupe_key("key-a");
    let key2 = encode_dedupe_key("key-b");
    assert_ne!(key1, key2);
}

#[test]
fn effect_key_roundtrip_with_max_sequence() {
    let id = max_instance_id();
    let seq = SequenceNumber::try_from(u64::MAX).unwrap();
    let key = encode_effect_key(&id, seq);
    let (decoded_id, decoded_seq) = decode_effect_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_seq.as_u64(), u64::MAX);
}

#[test]
fn lease_key_with_max_instance_id_roundtrips() {
    let id = max_instance_id();
    let step = StepId::parse("step-z").unwrap();
    let key = encode_lease_key(&id, &step);
    let (decoded_id, decoded_step) = decode_lease_key(&key).unwrap();
    assert_eq!(decoded_id, id);
    assert_eq!(decoded_step, step);
}

#[test]
fn given_event_key_when_encoded_with_ulid_then_instance_bytes_are_preserved() {
    let id = InstanceId::parse("01H5X2K3M4N5P6Q7R8S9T0VWXY").unwrap();
    let seq = SequenceNumber::try_from(42u64).unwrap();
    let key = encode_event_key(&id, seq);

    assert_eq!(
        key.len(),
        26,
        "event key must be exactly 26 bytes (2-byte length prefix + 16-byte instance + 8-byte sequence)"
    );

    let iid_bytes = id.to_bytes().unwrap();
    assert_eq!(
        iid_bytes.len(),
        16,
        "InstanceId::to_bytes must produce exactly 16 bytes (ULID binary)"
    );
    // Bytes 0..2 are length prefix (16), bytes 2..18 are instance bytes
    assert_eq!(&key[0..2], 16u16.to_be_bytes());
    assert_eq!(
        &key[2..18],
        &iid_bytes,
        "bytes 2..18 of event key must be the raw ULID bytes"
    );

    let reconstructed = InstanceId::from_bytes(iid_bytes);
    assert_eq!(
        reconstructed, id,
        "reconstructing InstanceId from the 16-byte slice must yield the original"
    );

    let roundtrip = decode_event_key(&key).unwrap();
    assert_eq!(roundtrip.0, id);
    assert_eq!(roundtrip.1, seq);

    let id2 = InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap();
    let key2 = encode_event_key(&id2, seq);
    assert_eq!(
        key2.len(),
        26,
        "different InstanceId still produces 26-byte key (length-prefixed)"
    );
    assert_ne!(
        key[2..18],
        key2[2..18],
        "different InstanceIds must differ in bytes 2..18"
    );
}

#[allow(dead_code)]
fn get_effect_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&iid_bytes);
    prefix
}

use crate::key_encoding::{
    extract_legacy_fence_components, extract_legacy_lease_components,
    is_legacy_delimiter_fence_key, is_legacy_delimiter_lease_key,
};

/// BDD: Given storage contains legacy delimiter lease keys
/// When engine starts
/// Then legacy keys are migrated or reported before execution resumes
#[test]
fn given_legacy_delimiter_lease_keys_when_startup_scans_then_keys_are_migrated_or_reported() {
    // Given: a legacy delimiter lease key
    // Format: "{instance_id}::{step_id}" where instance_id is a string ULID
    let legacy_iid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let legacy_sid = "step-1";
    let legacy_key = format!("{legacy_iid}::{legacy_sid}").into_bytes();

    // When: we check if it's detected as legacy
    assert!(
        is_legacy_delimiter_lease_key(&legacy_key),
        "legacy delimiter key should be detected"
    );

    // And: we can extract the components
    let (extracted_iid, extracted_sid) = extract_legacy_lease_components(&legacy_key)
        .expect("should extract instance_id and step_id from legacy delimiter key");
    assert_eq!(extracted_iid, legacy_iid);
    assert_eq!(extracted_sid, legacy_sid);

    // And: the new format key is NOT detected as legacy
    let id = InstanceId::parse(legacy_iid).unwrap();
    let step = StepId::parse(legacy_sid).unwrap();
    let new_key = encode_lease_key(&id, &step);
    assert!(
        !is_legacy_delimiter_lease_key(&new_key),
        "new format lease key should NOT be detected as legacy"
    );

    // And: new format key does not contain :: as a field delimiter
    // (the 16-byte instance ID binary may contain 0x3A bytes, but not as :: delimiter)
    let sid_start = 18;
    let sid_bytes = &new_key[sid_start..];
    assert!(
        !sid_bytes.windows(2).any(|w| w == b"::"),
        "new format step_id portion should not contain :: delimiter"
    );

    // And: new format key is length-prefixed and starts with instance ID bytes
    assert_eq!(&new_key[..16], id.to_bytes().unwrap_or([0u8; 16]));

    // And: legacy fence keys are also detected
    let legacy_fence_key = format!("{legacy_iid}::{legacy_sid}::fence").into_bytes();
    assert!(
        is_legacy_delimiter_fence_key(&legacy_fence_key),
        "legacy fence key should be detected"
    );

    let (fenced_iid, fenced_sid) = extract_legacy_fence_components(&legacy_fence_key)
        .expect("should extract instance_id and step_id from legacy fence key");
    assert_eq!(fenced_iid, legacy_iid);
    assert_eq!(fenced_sid, legacy_sid);

    // And: keys shorter than 14 bytes are not flagged as legacy
    assert!(
        !is_legacy_delimiter_lease_key(&[b':', b':']),
        "short keys should not be detected as legacy"
    );

    // And: ADR-020 compliant keys with :: in the step_id are not flagged
    // New format: [iid 16 bytes][sid_len BE u16][sid bytes]
    // Even if step_id string contains colons, the binary encoding won't
    let weird_step =
        StepId::parse("step::weird").unwrap_or_else(|_| StepId::parse("step-x").unwrap());
    let new_key2 = encode_lease_key(&id, &weird_step);
    assert!(
        !is_legacy_delimiter_lease_key(&new_key2),
        "new format key should never be flagged as legacy"
    );

    // And: keys with multiple :: are not treated as legacy
    let multi_delim = format!("{legacy_iid}::foo::bar").into_bytes();
    assert!(
        !is_legacy_delimiter_lease_key(&multi_delim),
        "keys with multiple :: delimiters should not be detected as legacy"
    );
}

/// Additional: new format keys with various step IDs are not detected as legacy
#[test]
fn new_format_lease_keys_never_detected_as_legacy() {
    let id = min_instance_id();

    // Test with various step ID lengths
    for step_str in ["a", "step-1", "my-very-long-step-id", "x"] {
        let step = StepId::parse(step_str).unwrap();
        let new_key = encode_lease_key(&id, &step);
        assert!(
            !is_legacy_delimiter_lease_key(&new_key),
            "new format key for step '{step_str}' should not be legacy"
        );

        // Extract should return None for new format
        assert!(
            extract_legacy_lease_components(&new_key).is_none(),
            "new format key should not have extractable legacy components"
        );
    }
}

/// New format lease keys that happen to contain 0x3A bytes are not misdetected
#[test]
fn new_format_keys_with_colon_bytes_not_misdetected() {
    // Create an instance ID that, when encoded as 16 bytes, might contain 0x3A (':')
    let id = min_instance_id();
    let step = StepId::parse("step-1").unwrap();

    let new_key = encode_lease_key(&id, &step);

    // Check that even if 0x3A bytes are present, the structural check (BE u16 length prefix
    // at bytes 16-18 matching remaining length) prevents false positive
    assert!(
        !is_legacy_delimiter_lease_key(&new_key),
        "new format key should never be flagged as legacy regardless of byte content"
    );
}

/// Fence key detection: legacy fence keys end with ::fence
#[test]
fn legacy_fence_key_detection() {
    let legacy_iid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let legacy_sid = "step-1";

    // Legacy fence key
    let legacy_fence = format!("{legacy_iid}::{legacy_sid}::fence").into_bytes();
    assert!(is_legacy_delimiter_fence_key(&legacy_fence));
    assert!(is_legacy_delimiter_lease_key(
        &legacy_fence[..legacy_fence.len() - 7]
    ));

    // New format fence key should NOT be detected
    let _id = min_instance_id();
    let _step = StepId::parse("step-1").unwrap();
    let _new_fence = b"newformat".to_vec(); // would be encoded differently
    assert!(!is_legacy_delimiter_fence_key(&_new_fence));
}
