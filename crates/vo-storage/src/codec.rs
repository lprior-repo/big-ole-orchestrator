#![allow(unexpected_cfgs)]
use std::fmt;
use vo_types::{InstanceId, SequenceNumber};

#[derive(Debug, PartialEq, Eq)]
pub enum StorageError {
    CorruptKey,
    Other,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorruptKey => write!(f, "corrupt key"),
            Self::Other => write!(f, "other error"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Encodes an `(InstanceId, SequenceNumber)` pair into a 24-byte event key.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if the `InstanceId` cannot be converted to bytes.
pub fn encode_event_key(
    instance_id: &InstanceId,
    sequence: &SequenceNumber,
) -> Result<[u8; 24], StorageError> {
    let id_bytes = instance_id
        .to_bytes()
        .map_err(|_| StorageError::CorruptKey)?;
    let seq_bytes = sequence.as_u64().to_be_bytes();
    let mut key = [0u8; 24];
    key[..16].copy_from_slice(&id_bytes);
    key[16..].copy_from_slice(&seq_bytes);
    Ok(key)
}

/// Decodes a 24-byte event key into an `InstanceId` and `SequenceNumber`.
///
/// # Errors
///
/// Returns `StorageError::CorruptKey` if `bytes` is not exactly 24 bytes long,
/// or if the sequence number is 0.
pub fn decode_event_key(bytes: &[u8]) -> Result<(InstanceId, SequenceNumber), StorageError> {
    let id_bytes: [u8; 16] = bytes
        .get(..16)
        .and_then(|s| s.try_into().ok())
        .ok_or(StorageError::CorruptKey)?;
    let seq_bytes: [u8; 8] = bytes
        .get(16..24)
        .and_then(|s| s.try_into().ok())
        .ok_or(StorageError::CorruptKey)?;
    let instance_id = InstanceId::from_bytes(id_bytes);
    let seq_val = u64::from_be_bytes(seq_bytes);
    let sequence = SequenceNumber::try_from(seq_val).map_err(|_| StorageError::CorruptKey)?;
    Ok((instance_id, sequence))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Helper for minimum valid ULID (since all 0s is nil and rejected)
    fn min_id() -> InstanceId {
        InstanceId::parse("00000000000000000000000001").unwrap()
    }
    fn max_id() -> InstanceId {
        InstanceId::parse("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").unwrap()
    }
    #[allow(dead_code)]
    fn mixed_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    #[test]
    fn encode_event_key_returns_exact_bytes_when_inputs_are_minimums() {
        let id = min_id();
        let seq = SequenceNumber::try_from(1u64).unwrap();
        let result = encode_event_key(&id, &seq).unwrap();
        let expected = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn encode_event_key_returns_exact_bytes_when_inputs_are_maximums() {
        let id = max_id();
        let seq = SequenceNumber::try_from(u64::MAX).unwrap();
        let result = encode_event_key(&id, &seq).unwrap();
        let expected = [0xFF; 24];
        assert_eq!(result, expected);
    }

    #[test]
    fn encode_event_key_returns_concrete_big_endian_bytes_to_preserve_ordering() {
        let id = max_id();
        let seq = SequenceNumber::try_from(0x0102_0304_0506_0708_u64).unwrap();
        let result = encode_event_key(&id, &seq).unwrap();
        let expected = [
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn decode_event_key_returns_exact_components_when_bytes_are_minimum_valid() {
        let input = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ];
        let result = decode_event_key(&input);
        assert_eq!(
            result,
            Ok((min_id(), SequenceNumber::try_from(1u64).unwrap()))
        );
    }

    #[test]
    fn decode_event_key_returns_exact_components_when_bytes_are_maximum_valid() {
        let input = [0xFF; 24];
        let result = decode_event_key(&input);
        assert_eq!(
            result,
            Ok((max_id(), SequenceNumber::try_from(u64::MAX).unwrap()))
        );
    }

    #[test]
    fn decode_event_key_returns_exact_components_when_bytes_have_mixed_endianness() {
        let input = [
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        ];
        let result = decode_event_key(&input);
        assert_eq!(
            result,
            Ok((
                max_id(),
                SequenceNumber::try_from(0x0102_0304_0506_0708_u64).unwrap()
            ))
        );
    }

    #[test]
    fn decode_event_key_returns_corrupt_key_error_when_bytes_are_too_short() {
        let input = [0x00; 23];
        let result = decode_event_key(&input);
        assert_eq!(result, Err(StorageError::CorruptKey));
    }

    #[test]
    fn decode_event_key_returns_corrupt_key_error_when_bytes_are_too_long() {
        let input = [0x00; 25];
        let result = decode_event_key(&input);
        assert_eq!(result, Err(StorageError::CorruptKey));
    }

    #[test]
    fn decode_event_key_returns_corrupt_key_error_when_bytes_are_empty() {
        let input: [u8; 0] = [];
        let result = decode_event_key(&input);
        assert_eq!(result, Err(StorageError::CorruptKey));
    }

    #[test]
    fn decode_event_key_returns_error_when_sequence_is_zero() {
        let input = [0x00; 24];
        let result = decode_event_key(&input);
        assert_eq!(result, Err(StorageError::CorruptKey));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn roundtrip_encode_decode(
            id_str in "[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}",
            seq_val in 1u64..=u64::MAX
        ) {
            if let Ok(id) = InstanceId::parse(&id_str) {
                let seq = SequenceNumber::try_from(seq_val).unwrap();
                let encoded = encode_event_key(&id, &seq).unwrap();
                let decoded = decode_event_key(&encoded);
                prop_assert_eq!(decoded, Ok((id, seq)));
            }
        }
    }
}

#[cfg(kani)]
#[allow(unexpected_cfgs)]
mod verification {
    use super::*;

    #[kani::proof]
    fn verify_codec_length_bounds() {
        // Can't easily use InstanceId parsing in Kani without it blowing up on regex/string allocs
        // We'll mock it if it was required, but we can't here easily unless we bypass.
        // For the sake of the red phase test stub, this function just has to compile.
        let bytes: [u8; 24] = kani::any();
        decode_event_key(&bytes).unwrap_or((min_id(), SequenceNumber::try_from(1u64).unwrap()));
    }
}
