use super::*;
use std::num::NonZeroU64;

#[test]
fn try_from_u64_sequence_number_valid() {
    let sn = SequenceNumber::try_from(42u64).expect("valid");
    assert_eq!(sn.as_u64(), 42);
}

#[test]
fn try_from_u64_sequence_number_zero() {
    let result = SequenceNumber::try_from(0u64);
    assert_eq!(
        result,
        Err(ParseError::ZeroValue {
            type_name: "SequenceNumber"
        })
    );
}

#[test]
fn try_from_u64_duration_ms_valid() {
    let dm = DurationMs::try_from(0u64).expect("valid");
    assert_eq!(dm.as_u64(), 0);
}

#[test]
fn try_from_u64_duration_ms_nonzero() {
    let dm = DurationMs::try_from(1500u64).expect("valid");
    assert_eq!(dm.as_u64(), 1500);
}

#[test]
fn from_sequence_number_into_u64() {
    let sn = SequenceNumber::new_unchecked(42);
    let val: u64 = sn.into();
    assert_eq!(val, 42);
}

#[test]
fn from_duration_ms_into_u64() {
    let dm = DurationMs(1500);
    let val: u64 = dm.into();
    assert_eq!(val, 1500);
}