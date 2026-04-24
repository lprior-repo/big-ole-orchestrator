//! Unit tests for the event replay query engine.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use super::*;
use proptest::prelude::*;
use vo_types::events::EventMetadata;

// ---- encode_key tests ----

#[test]
fn encode_key_returns_big_endian_bytes_for_sequence_one() {
    assert_eq!(encode_key(1), Ok([0u8, 0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn encode_key_returns_big_endian_bytes_for_u64_max() {
    assert_eq!(encode_key(u64::MAX), Ok([0xFF; 8]));
}

#[test]
fn encode_key_returns_error_for_zero_sequence() {
    assert_eq!(encode_key(0), Err(StorageError::InvalidArgument));
}

#[test]
fn encode_key_returns_big_endian_bytes_for_large_value() {
    let result = encode_key(256);
    assert_eq!(result, Ok([0u8, 0, 0, 0, 0, 0, 1, 0]));
}

#[test]
fn encode_key_is_const_fn() {
    const _VAL: Result<[u8; 8], StorageError> = encode_key(42);
}

// ---- decode_key tests ----

#[test]
fn decode_key_returns_sequence_for_valid_big_endian_bytes() {
    let bytes = 1u64.to_be_bytes();
    assert_eq!(decode_key(&bytes), Ok(1));
}

#[test]
fn decode_key_returns_error_for_empty_slice() {
    assert_eq!(decode_key(&[]), Err(StorageError::Storage));
}

#[test]
fn decode_key_returns_error_for_short_slice() {
    assert_eq!(decode_key(&[0u8, 0, 0, 0]), Err(StorageError::Storage));
}

#[test]
fn decode_key_returns_error_for_zero_sequence() {
    assert_eq!(decode_key(&[0u8; 8]), Err(StorageError::InvalidArgument));
}

#[test]
fn decode_key_roundtrips_with_encode_key() {
    let seqs = [1u64, 100, u64::MAX, 42, 999_999];
    let encoded0 = encode_key(seqs[0]).expect("valid seq should encode");
    assert_eq!(decode_key(&encoded0), Ok(seqs[0]));
    let encoded1 = encode_key(seqs[1]).expect("valid seq should encode");
    assert_eq!(decode_key(&encoded1), Ok(seqs[1]));
    let encoded2 = encode_key(seqs[2]).expect("valid seq should encode");
    assert_eq!(decode_key(&encoded2), Ok(seqs[2]));
    let encoded3 = encode_key(seqs[3]).expect("valid seq should encode");
    assert_eq!(decode_key(&encoded3), Ok(seqs[3]));
    let encoded4 = encode_key(seqs[4]).expect("valid seq should encode");
    assert_eq!(decode_key(&encoded4), Ok(seqs[4]));
}

// ---- prefix_generator tests ----
// Note: prefix_generator now accepts &InstanceId (DDD: parse, don't validate).
// Invalid string inputs (empty, null bytes, >255 chars) are rejected at the
// InstanceId::parse boundary — the type system makes those tests unnecessary.

#[test]
fn prefix_generator_returns_bytes_for_valid_instance_id() {
    let id = InstanceId::from_bytes([0x01; 16]);
    let result = prefix_generator(&id);
    let Ok(bytes) = result else {
        panic!("result should be Ok");
    };
    assert_eq!(bytes, id.as_str().as_bytes().to_vec());
}

#[test]
fn prefix_generator_returns_26_bytes_for_any_valid_instance_id() {
    // ULIDs are always exactly 26 Crockford Base32 characters
    let id = InstanceId::from_bytes([0x42; 16]);
    let bytes = prefix_generator(&id).unwrap();
    assert_eq!(bytes.len(), 26);
}

// ---- error_mapper tests ----

#[test]
fn error_mapper_maps_unsupported_envelope_version() {
    let err = EventError::UnsupportedEnvelopeVersion(99);
    assert_eq!(error_mapper(&err), StorageError::UnsupportedVersion);
}

#[test]
fn error_mapper_maps_invalid_input_to_corrupt_payload() {
    let err = EventError::InvalidInput;
    assert_eq!(error_mapper(&err), StorageError::CorruptEventPayload);
}

#[test]
fn error_mapper_maps_invalid_envelope_format_to_corrupt_payload() {
    let err = EventError::InvalidEnvelopeFormat;
    assert_eq!(error_mapper(&err), StorageError::CorruptEventPayload);
}

// ---- IteratorState tests ----

#[test]
fn iterator_state_first_advance_accepts_any_nonzero() {
    let mut state = IteratorState::new();
    let env = make_envelope(1);
    let result = state.advance(5, env);
    assert_eq!(result, Some(Ok(make_envelope(1))));
}

#[test]
fn iterator_state_rejects_zero_sequence() {
    let mut state = IteratorState::new();
    let env = make_envelope(0);
    let result = state.advance(0, env);
    assert_eq!(result, Some(Err(StorageError::InvalidArgument)));
}

#[test]
fn iterator_state_detects_sequence_gap() {
    let mut state = IteratorState::new();
    let env1 = make_envelope(1);
    let first = state.advance(1, env1);
    assert_eq!(first, Some(Ok(make_envelope(1))));
    let env2 = make_envelope(3);
    let result = state.advance(3, env2);
    assert_eq!(result, Some(Err(StorageError::SequenceGap)));
}

#[test]
fn iterator_state_accepts_consecutive_sequences() {
    let mut state = IteratorState::new();
    let env1 = make_envelope(1);
    let r1 = state.advance(1, env1);
    assert_eq!(r1, Some(Ok(make_envelope(1))));
    let env2 = make_envelope(2);
    let r2 = state.advance(2, env2);
    assert_eq!(r2, Some(Ok(make_envelope(2))));
}

#[test]
fn iterator_state_handles_u64_overflow_checked_add() {
    let mut state = IteratorState::new();
    let env = make_envelope(u64::MAX);
    let r1 = state.advance(u64::MAX, env);
    assert_eq!(r1, Some(Ok(make_envelope(u64::MAX))));
    // expected is now None (overflow)
    let env2 = make_envelope(1);
    let r2 = state.advance(1, env2);
    assert_eq!(r2, Some(Err(StorageError::SequenceGap)));
}

fn make_envelope(seq: u64) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: "test-instance".to_string(),
        sequence: seq,
        timestamp_ms: 1000,
        payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1"}),
        metadata: EventMetadata::default(),
    }
}

// ---- lineage_prefix_generator tests ----

#[test]
fn lineage_prefix_generator_returns_prefix_with_null_delimiters() {
    let lineage_id = "wf-lineage-123";
    let result = lineage_prefix_generator(lineage_id).expect("should succeed");
    assert_eq!(result.len(), 1 + lineage_id.len() + 1);
    assert_eq!(result[0], LINEAGE_ID_NULL_BYTE);
    assert_eq!(result[result.len() - 1], LINEAGE_ID_NULL_BYTE);
}

#[test]
fn lineage_prefix_generator_rejects_empty_lineage_id() {
    let result = lineage_prefix_generator("");
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn lineage_prefix_generator_rejects_lineage_id_with_null_byte() {
    let lineage_id = "wf\0lineage";
    let result = lineage_prefix_generator(lineage_id);
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn lineage_prefix_generator_rejects_lineage_id_exceeding_max_len() {
    let lineage_id = "x".repeat(LINEAGE_ID_MAX_LEN + 1);
    let result = lineage_prefix_generator(&lineage_id);
    assert_eq!(result, Err(StorageError::InvalidArgument));
}

#[test]
fn lineage_prefix_generator_accepts_lineage_id_at_max_len() {
    let lineage_id = "x".repeat(LINEAGE_ID_MAX_LEN);
    let result = lineage_prefix_generator(&lineage_id);
    assert!(result.is_ok());
}

// ---- epoch_prefix_generator tests ----

#[test]
fn epoch_prefix_generator_returns_prefix_with_lineage_and_epoch() {
    let lineage_id = "wf-123";
    let epoch = Epoch::new(5);
    let result = epoch_prefix_generator(lineage_id, epoch).expect("should succeed");
    let lineage_prefix = lineage_prefix_generator(lineage_id).expect("should succeed");
    let epoch_bytes = epoch.get().to_be_bytes();
    assert_eq!(result.len(), lineage_prefix.len() + 8);
    assert!(result.starts_with(&lineage_prefix));
    assert_eq!(&result[lineage_prefix.len()..], &epoch_bytes);
}

#[test]
fn epoch_prefix_generator_zero_epoch_produces_valid_prefix() {
    let lineage_id = "wf-zero";
    let epoch = Epoch::ZERO;
    let result = epoch_prefix_generator(lineage_id, epoch).expect("should succeed");
    assert!(!result.is_empty());
}

// ---- LineageQuery tests ----

#[test]
fn lineage_query_instance_id_to_prefix_uses_instance_id_prefix() {
    let id = InstanceId::from_bytes([0x01; 16]);
    let query = LineageQuery::InstanceId(&id);
    let prefix = query.to_prefix().expect("should succeed");
    assert_eq!(prefix, id.as_str().as_bytes().to_vec());
}

#[test]
fn lineage_query_lineage_wide_to_prefix_uses_lineage_prefix() {
    let query = LineageQuery::LineageWide {
        lineage_id: "wf-abc",
    };
    let prefix = query.to_prefix().expect("should succeed");
    let expected = lineage_prefix_generator("wf-abc").expect("should succeed");
    assert_eq!(prefix, expected);
}

#[test]
fn lineage_query_epoch_specific_to_prefix_uses_epoch_prefix() {
    let lineage_id = "wf-xyz";
    let epoch = Epoch::new(3);
    let query = LineageQuery::EpochSpecific { lineage_id, epoch };
    let prefix = query.to_prefix().expect("should succeed");
    let expected = epoch_prefix_generator(lineage_id, epoch).expect("should succeed");
    assert_eq!(prefix, expected);
}

// ---- proptests ----

proptest! {
    #[test]
    fn proptest_encode_decode_roundtrip(seq in 1u64..u64::MAX) {
        let encoded = encode_key(seq).expect("valid");
        prop_assert_eq!(decode_key(&encoded), Ok(seq));
    }

    #[test]
    fn proptest_encode_key_never_returns_none_for_nonzero(seq in 1u64..) {
        prop_assert_eq!(encode_key(seq), Ok(seq.to_be_bytes()));
    }

    #[test]
    fn proptest_lineage_prefix_roundtrip(lineage_id in "[a-z]{1,50}") {
        let prefix = lineage_prefix_generator(&lineage_id).expect("valid");
        prop_assert!(prefix.len() <= LINEAGE_ID_MAX_LEN + 2);
    }

    #[test]
    fn proptest_epoch_prefix_includes_epoch_bytes(epoch_val in 0u64..1000u64) {
        let lineage_id = "wf-test";
        let epoch = Epoch::new(epoch_val);
        let result = epoch_prefix_generator(lineage_id, epoch).expect("valid");
        let epoch_bytes = epoch_val.to_be_bytes();
        prop_assert!(result.ends_with(&epoch_bytes));
    }
}
