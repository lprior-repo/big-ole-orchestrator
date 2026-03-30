//! Event replay query engine — pure key/encode/decode functions + stateful iterator.
//!
//! Architecture: Data (`StorageError`, `IteratorState`) → Calc (`encode_key`, `decode_key`,
//! `prefix_generator`, `error_mapper`) → Actions (`EventReplayIterator`, `replay_events`).

use vo_types::{EventEnvelope, EventError, InstanceId};

// ---------------------------------------------------------------------------
// Data layer — error enum
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub enum StorageError {
    SequenceGap,
    CorruptEventPayload,
    UnsupportedVersion,
    Storage,
    InvalidArgument,
}

// ---------------------------------------------------------------------------
// Calc layer — pure functions
// ---------------------------------------------------------------------------

/// Encode a sequence number as big-endian bytes.
///
/// # Errors
///
/// Returns `StorageError::InvalidArgument` if `sequence` is zero.
#[must_use = "encode_key performs a pure encoding computation"]
pub const fn encode_key(sequence: u64) -> Result<[u8; 8], StorageError> {
    if sequence == 0 {
        return Err(StorageError::InvalidArgument);
    }
    Ok(sequence.to_be_bytes())
}

/// Decode a big-endian 8-byte slice into a sequence number.
///
/// # Errors
///
/// Returns `StorageError::Storage` if the slice is not exactly 8 bytes or decodes to zero.
pub fn decode_key(bytes: &[u8]) -> Result<u64, StorageError> {
    let arr: [u8; 8] = bytes.try_into().map_err(|_| StorageError::Storage)?;
    let seq = u64::from_be_bytes(arr);
    if seq == 0 {
        return Err(StorageError::Storage);
    }
    Ok(seq)
}

/// Produce the prefix bytes for range-scanning a given instance.
///
/// # Errors
///
/// Returns `StorageError::Storage` if the instance ID exceeds 255 bytes.
/// Returns `StorageError::InvalidArgument` if the instance ID contains null bytes.
pub fn prefix_generator(instance_id: &str) -> Result<Vec<u8>, StorageError> {
    if instance_id.len() > 255 {
        return Err(StorageError::Storage);
    }
    if instance_id.as_bytes().contains(&b'\0') {
        return Err(StorageError::InvalidArgument);
    }
    Ok(instance_id.as_bytes().to_vec())
}

/// Map a decode-layer error into the appropriate `StorageError` variant.
#[must_use]
pub const fn error_mapper(error: &EventError) -> StorageError {
    match error {
        EventError::UnsupportedEnvelopeVersion(_) => StorageError::UnsupportedVersion,
        _ => StorageError::CorruptEventPayload,
    }
}

// ---------------------------------------------------------------------------
// Data layer — iterator state machine
// ---------------------------------------------------------------------------

pub struct IteratorState {
    expected: Option<u64>,
    started: bool,
}

impl Default for IteratorState {
    fn default() -> Self {
        Self::new()
    }
}

impl IteratorState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            expected: None,
            started: false,
        }
    }

    pub fn advance(
        &mut self,
        found: u64,
        record: EventEnvelope,
    ) -> Option<Result<EventEnvelope, StorageError>> {
        if found == 0 {
            return Some(Err(StorageError::InvalidArgument));
        }
        if !self.started {
            self.started = true;
            self.expected = found.checked_add(1);
            return Some(Ok(record));
        }
        match self.expected {
            Some(expected) if found != expected => Some(Err(StorageError::SequenceGap)),
            Some(expected) => {
                self.expected = expected.checked_add(1);
                Some(Ok(record))
            }
            None => Some(Err(StorageError::SequenceGap)),
        }
    }
}

// ---------------------------------------------------------------------------
// Actions layer — iterator + constructor
// ---------------------------------------------------------------------------

pub struct EventReplayIterator {
    state: IteratorState,
    inner: Option<Box<dyn DoubleEndedIterator<Item = fjall::Result<fjall::KvPair>>>>,
    init_error: Option<StorageError>,
}

impl Iterator for EventReplayIterator {
    type Item = Result<EventEnvelope, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.init_error.take() {
            return Some(Err(err));
        }
        let Some(inner) = &mut self.inner else {
            return None;
        };
        match inner.next() {
            Some(Ok((k_bytes, v_bytes))) => self.process_kv(&k_bytes, &v_bytes),
            Some(Err(_)) => {
                self.inner = None;
                Some(Err(StorageError::Storage))
            }
            None => None,
        }
    }
}

impl EventReplayIterator {
    fn process_kv(
        &mut self,
        k_bytes: &fjall::Slice,
        v_bytes: &fjall::Slice,
    ) -> Option<Result<EventEnvelope, StorageError>> {
        let seq_len: usize = 8;
        if k_bytes.len() < seq_len {
            self.inner = None;
            return Some(Err(StorageError::Storage));
        }
        let seq_bytes = &k_bytes[k_bytes.len() - seq_len..];
        let found_seq = match decode_key(seq_bytes) {
            Ok(s) => s,
            Err(e) => {
                self.inner = None;
                return Some(Err(e));
            }
        };
        let envelope = match EventEnvelope::from_bytes(v_bytes) {
            Ok(e) => e,
            Err(EventError::UnsupportedEnvelopeVersion(_)) => {
                self.inner = None;
                return Some(Err(StorageError::UnsupportedVersion));
            }
            Err(_) => {
                self.inner = None;
                return Some(Err(StorageError::CorruptEventPayload));
            }
        };
        match self.state.advance(found_seq, envelope) {
            Some(Err(e)) => {
                self.inner = None;
                Some(Err(e))
            }
            Some(Ok(env)) => Some(Ok(env)),
            None => None,
        }
    }
}

#[must_use]
pub fn replay_events(keyspace: &fjall::Keyspace, instance_id: &InstanceId) -> EventReplayIterator {
    let prefix = match prefix_generator(instance_id.as_str()) {
        Ok(p) => p,
        Err(e) => {
            return EventReplayIterator {
                state: IteratorState::new(),
                inner: None,
                init_error: Some(e),
            };
        }
    };
    let Ok(partition) = keyspace.open_partition("events", fjall::PartitionCreateOptions::default())
    else {
        return EventReplayIterator {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };
    let Ok(min_seq) = encode_key(1) else {
        return EventReplayIterator {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };
    let Ok(max_seq) = encode_key(u64::MAX) else {
        return EventReplayIterator {
            state: IteratorState::new(),
            inner: None,
            init_error: Some(StorageError::Storage),
        };
    };
    let mut start = prefix.clone();
    start.extend_from_slice(&min_seq);
    let mut end = prefix;
    end.extend_from_slice(&max_seq);
    let iter = partition.range(start..=end);
    EventReplayIterator {
        state: IteratorState::new(),
        inner: Some(Box::new(iter)),
        init_error: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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
        assert_eq!(decode_key(&[0u8; 8]), Err(StorageError::Storage));
    }

    #[test]
    fn decode_key_roundtrips_with_encode_key() {
        for seq in [1u64, 100, u64::MAX, 42, 999_999] {
            let encoded = encode_key(seq).expect("valid seq should encode");
            assert_eq!(decode_key(&encoded), Ok(seq));
        }
    }

    // ---- prefix_generator tests ----

    #[test]
    fn prefix_generator_returns_bytes_of_instance_id() {
        let result = prefix_generator("abc");
        assert_eq!(result, Ok(vec![b'a', b'b', b'c']));
    }

    #[test]
    fn prefix_generator_returns_empty_vec_for_empty_string() {
        let result = prefix_generator("");
        assert_eq!(result, Ok(vec![]));
    }

    #[test]
    fn prefix_generator_returns_error_for_null_byte_in_instance_id() {
        let input = "ab\0c";
        assert_eq!(prefix_generator(input), Err(StorageError::InvalidArgument));
    }

    #[test]
    fn prefix_generator_returns_error_for_overly_long_instance_id() {
        let long_id = "a".repeat(256);
        assert_eq!(prefix_generator(&long_id), Err(StorageError::Storage));
    }

    #[test]
    fn prefix_generator_accepts_max_length_instance_id() {
        let max_id = "a".repeat(255);
        assert!(prefix_generator(&max_id).is_ok());
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
        assert!(result.is_some());
        assert!(result.unwrap().is_ok());
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
        let _ = state.advance(1, env1);
        let env2 = make_envelope(3);
        let result = state.advance(3, env2);
        assert_eq!(result, Some(Err(StorageError::SequenceGap)));
    }

    #[test]
    fn iterator_state_accepts_consecutive_sequences() {
        let mut state = IteratorState::new();
        let env1 = make_envelope(1);
        let r1 = state.advance(1, env1);
        assert!(r1.unwrap().is_ok());
        let env2 = make_envelope(2);
        let r2 = state.advance(2, env2);
        assert!(r2.unwrap().is_ok());
    }

    #[test]
    fn iterator_state_handles_u64_overflow_checked_add() {
        let mut state = IteratorState::new();
        let env = make_envelope(u64::MAX);
        let r1 = state.advance(u64::MAX, env);
        assert!(r1.unwrap().is_ok());
        // expected is now None (overflow)
        let env2 = make_envelope(1);
        let r2 = state.advance(1, env2);
        assert_eq!(r2, Some(Err(StorageError::SequenceGap)));
    }

    fn make_envelope(seq: u64) -> EventEnvelope {
        EventEnvelope {
            version: 1,
            instance_id: "test-instance".to_string(),
            sequence: seq,
            timestamp_ms: 1000,
            payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-1"}),
            metadata: serde_json::json!({}),
        }
    }

    // ---- proptests ----

    proptest! {
        #[test]
        fn proptest_encode_decode_roundtrip(seq in 1u64..u64::MAX) {
            let encoded = encode_key(seq).expect("valid");
            prop_assert_eq!(decode_key(&encoded), Ok(seq));
        }

        #[test]
        fn proptest_prefix_generator_never_panics(s in "\\PC{0,255}") {
            let _ = prefix_generator(&s);
        }

        #[test]
        fn proptest_encode_key_never_returns_none_for_nonzero(seq in 1u64..) {
            prop_assert!(encode_key(seq).is_ok());
        }
    }
}
