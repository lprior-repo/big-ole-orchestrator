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
    let result = encode_length_prefixed(b"hello").unwrap();
    assert_eq!(result.len(), 7);
    assert_eq!(&result[0..2], &[0, 5]);
    assert_eq!(&result[2..], b"hello");
}

#[test]
fn encode_length_prefixed_handles_empty() {
    let result = encode_length_prefixed(b"").unwrap();
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
        let encoded = encode_length_prefixed(input).unwrap();
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
    let encoded = encode_step_id(&step).unwrap();
    assert!(encoded.len() > step.as_str().len());
    assert_eq!(&encoded[0..2], &(step.as_str().len() as u16).to_be_bytes());
}

#[test]
fn encode_step_id_roundtrips_correctly() {
    for step_str in ["step-1", "a", "my-very-long-step-id-12345"] {
        let step = StepId::parse(step_str).unwrap();
        let encoded = encode_step_id(&step).unwrap();
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
fn encode_event_key_produces_26_byte_key_with_length_prefix() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&id, seq);
    assert_eq!(key.len(), 26);
    assert_eq!(&key[0..2], &[0, 16], "instance ID length prefix should be 16");
    assert_eq!(&key[2..18], &id.to_bytes().unwrap(), "instance ID bytes should start at offset 2");
    assert_eq!(&key[18..26], &seq.as_u64().to_be_bytes(), "sequence number should be at offset 18");
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
fn decode_event_key_returns_error_for_wrong_length() {
    assert!(decode_event_key(&[]).is_err());
    assert!(decode_event_key(&[0u8; 25]).is_err());
    assert!(decode_event_key(&[0u8; 27]).is_err());
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
    assert_eq!(key.len(), 26);
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
fn lease_key_uses_binary_format_with_length_prefix() {
    let id = min_instance_id();
    let step = StepId::parse("my-step").unwrap();
    let key = encode_lease_key(&id, &step);
    let step_bytes = step.as_str().as_bytes();
    assert_eq!(key.len(), 16 + 2 + step_bytes.len());
    assert_eq!(&key[..16], &id.to_bytes().unwrap());
    assert_eq!(&key[16..18], &(step_bytes.len() as u16).to_be_bytes());
    assert_eq!(&key[18..], step_bytes);
}

#[test]
fn get_lease_key_prefix_for_instance_matches_key_format() {
    let id = min_instance_id();
    let step = StepId::parse("step-a").unwrap();
    let prefix = get_lease_key_prefix_for_instance(&id);
    let key = encode_lease_key(&id, &step);
    assert_eq!(prefix.len(), 16);
    assert_eq!(prefix, id.to_bytes().unwrap());
    assert!(
        key.starts_with(&prefix),
        "lease key should start with instance prefix"
    );
}

#[test]
fn encode_dedupe_key_is_length_prefixed() {
    let key = encode_dedupe_key("my-idempotency-key").unwrap();
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
        let encoded = encode_dedupe_key(key_str).unwrap();
        let decoded = decode_dedupe_key(&encoded).unwrap();
        assert_eq!(decoded, key_str);
    }
}

#[test]
fn get_dedupe_key_prefix_equals_full_key_when_short() {
    let key_str = "short-key";
    let encoded = encode_dedupe_key(key_str).unwrap();
    let prefix = get_dedupe_key_prefix(key_str).unwrap();
    assert_eq!(prefix, encoded);
}

#[test]
fn encode_effect_key_produces_27_byte_key_with_length_prefix() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_effect_key(&id, seq);
    assert_eq!(key.len(), 27);
    assert_eq!(&key[0..2], &[0, 16], "instance ID length prefix should be 16");
    assert_eq!(&key[2..18], &id.to_bytes().unwrap());
    assert_eq!(&key[18..26], &seq.as_u64().to_be_bytes());
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
    assert_eq!(&effect_key[0..24], &event_key[0..24]);
    assert_eq!(effect_key[24], 0xFF);
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
    let key = encode_event_key(&id, seq); // 24 bytes, no 0xFF marker
    assert!(
        decode_effect_key(&key).is_err(),
        "effect key decode should reject keys without 0xFF marker"
    );
}

#[allow(dead_code)]
#[test]
fn decode_lease_key_rejects_missing_delimiter() {
    let bad_key = b"00000000000000000000000001step-no-delimiter";
    assert!(
        decode_lease_key(bad_key).is_err(),
        "lease key decode should reject keys without :: delimiter"
    );
}

#[test]
fn decode_lease_key_rejects_invalid_instance_id() {
    let bad_key = b"INVALID::step-1";
    assert!(
        decode_lease_key(bad_key).is_err(),
        "lease key decode should reject invalid instance IDs"
    );
}

#[test]
fn decode_lease_key_rejects_invalid_step_id() {
    let id = min_instance_id();
    let bad_key = format!("{id}::step with spaces").into_bytes();
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
    let encoded = encode_dedupe_key("").unwrap();
    let decoded = decode_dedupe_key(&encoded).unwrap();
    assert_eq!(decoded, "");
}

#[test]
fn dedupe_key_different_inputs_produce_different_outputs() {
    let key1 = encode_dedupe_key("key-a").unwrap();
    let key2 = encode_dedupe_key("key-b").unwrap();
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

#[allow(dead_code)]
fn get_effect_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&iid_bytes);
    prefix
}

/// ADR-020 / ADR-005: Timer keys use length-prefixed instance IDs for unambiguous
/// decoding. The format is [timestamp_u64_be(8)][instance_id_len_u16_be(2)]
/// [instance_id_bytes]. This guarantees that the instance ID can be parsed without
/// ambiguity even if instance IDs were variable length, while lexicographic ordering
/// by due time is preserved because the timestamp is the leading component.
#[test]
fn given_timer_key_when_encoded_then_components_are_unambiguous_and_ordered() {
    // Given: instance IDs of varying apparent ambiguity
    let id_a = min_instance_id();
    let id_b = max_instance_id();
    let id_c = InstanceId::parse("00000000000000000000000005").unwrap();

    // When: we encode timer keys at different timestamps
    let key_a_1000 = encode_timer_key(1000, &id_a);
    let key_a_2000 = encode_timer_key(2000, &id_a);
    let key_b_1000 = encode_timer_key(1000, &id_b);
    let key_c_1000 = encode_timer_key(1000, &id_c);

    // Then: the timestamp is the leading 8 bytes for lexicographic ordering
    assert!(key_a_1000 < key_a_2000, "earlier timestamps sort first");
    assert!(key_a_1000 < key_b_1000, "same timestamp: lexicographic on instance ID");
    assert!(key_c_1000 < key_b_1000, "middle < max for instance IDs at same timestamp");

    // And: decoding recovers the original components exactly (unambiguous)
    let (decoded_ts, decoded_id) = decode_timer_key(&key_a_1000).unwrap();
    assert_eq!(decoded_ts, 1000);
    assert_eq!(decoded_id, id_a);

    let (decoded_ts, decoded_id) = decode_timer_key(&key_b_1000).unwrap();
    assert_eq!(decoded_ts, 1000);
    assert_eq!(decoded_id, id_b);

    // And: the length prefix is correctly encoded and decoded
    let expected_len_bytes = 16u16.to_be_bytes();
    assert_eq!(&key_a_1000[8..10], &expected_len_bytes, "length prefix must be 16 for 16-byte ULID");
    assert_eq!(&key_a_1000[..8], &1000u64.to_be_bytes(), "timestamp must be first 8 bytes");
    assert_eq!(&key_a_1000[10..26], &id_a.to_bytes().unwrap(), "instance ID bytes start after length prefix");
}
