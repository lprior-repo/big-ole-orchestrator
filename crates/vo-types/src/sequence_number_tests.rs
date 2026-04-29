use super::*;
use std::num::NonZeroU64;

#[test]
fn sequence_number_accepts_valid_nonzero_decimal_when_input_parses() {
    let sn = SequenceNumber::parse("42").expect("valid");
    assert_eq!(sn.as_u64(), 42);
}

#[test]
fn sequence_number_accepts_u64_max_when_at_upper_boundary() {
    let sn = SequenceNumber::parse("18446744073709551615").expect("valid");
    assert_eq!(sn.as_u64(), u64::MAX);
}

#[test]
fn sequence_number_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
    assert_eq!(
        SequenceNumber::parse("abc"),
        Err(ParseError::NotAnInteger {
            type_name: "SequenceNumber",
            input: "abc".to_string(),
        })
    );
}

#[test]
fn sequence_number_rejects_zero_with_zero_value_when_input_is_zero() {
    assert_eq!(
        SequenceNumber::parse("0"),
        Err(ParseError::ZeroValue {
            type_name: "SequenceNumber"
        })
    );
}

#[test]
fn sequence_number_accepts_minimum_when_value_is_1() {
    let sn = SequenceNumber::parse("1").expect("valid");
    assert_eq!(sn.as_u64(), 1);
}

#[test]
fn sequence_number_display_equals_decimal() {
    let sn = SequenceNumber::new_unchecked(42);
    assert_eq!(format!("{sn}"), "42");
}

#[test]
fn sequence_number_display_round_trips_through_parse_when_valid() {
    let sn = SequenceNumber::new_unchecked(42);
    let s = format!("{sn}");
    assert_eq!(SequenceNumber::parse(&s), Ok(sn));
}

#[test]
fn sequence_number_new_unchecked_constructs_when_value_is_nonzero() {
    let sn = SequenceNumber::new_unchecked(42);
    assert_eq!(sn.as_u64(), 42);
}

#[test]
#[should_panic(expected = "SequenceNumber must be nonzero")]
fn sequence_number_new_unchecked_panics_when_value_is_zero() {
    let _val = SequenceNumber::new_unchecked(0);
}

#[test]
fn from_sequence_number_returns_correct_nonzero_u64_when_converted() {
    let sn = SequenceNumber::new_unchecked(42);
    let nz: NonZeroU64 = sn.into();
    assert_eq!(nz.get(), 42);
}
