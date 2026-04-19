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
fn encode_event_key_produces_24_byte_key() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_event_key(&id, seq);
    assert_eq!(key.len(), 24);
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
    assert!(decode_event_key(&[0u8; 23]).is_err());
    assert!(decode_event_key(&[0u8; 25]).is_err());
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
fn encode_timer_key_produces_24_byte_key() {
    let id = min_instance_id();
    let key = encode_timer_key(1000, &id);
    assert_eq!(key.len(), 24);
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
fn lease_key_format_uses_delimiter() {
    let id = min_instance_id();
    let step = StepId::parse("my-step").unwrap();
    let key = encode_lease_key(&id, &step);
    let key_str = String::from_utf8(key.clone()).unwrap();
    assert!(key_str.contains("::"));
    assert!(key_str.starts_with(&id.to_string()));
}

#[test]
fn get_lease_key_prefix_for_instance_matches_lease_key_encoding() {
    let id = min_instance_id();
    let prefix = get_lease_key_prefix_for_instance(&id);
    // Prefix should be "{instance_id}::" (26 chars + 2 separator = 28 bytes)
    assert_eq!(prefix.len(), 28);
    assert!(prefix.ends_with(b"::"));

    // Verify it actually matches lease key encoding
    let step = vo_types::StepId::parse("test-step").unwrap();
    let lease_key = encode_lease_key(&id, &step);
    assert!(lease_key.starts_with(&prefix));
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
fn encode_effect_key_produces_25_byte_key() {
    let id = min_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();
    let key = encode_effect_key(&id, seq);
    assert_eq!(key.len(), 25);
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
    assert!(decode_effect_key(&[0u8; 24]).is_err());
    assert!(decode_effect_key(&[0u8; 26]).is_err());
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
fn get_effect_key_prefix(instance_id: &InstanceId) -> Vec<u8> {
    let iid_bytes = instance_id.to_bytes().unwrap_or([0u8; 16]);
    let mut prefix = Vec::with_capacity(16);
    prefix.extend_from_slice(&iid_bytes);
    prefix
}
