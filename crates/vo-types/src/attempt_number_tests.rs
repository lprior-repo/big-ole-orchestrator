use super::*;

#[test]
fn attempt_number_accepts_valid_nonzero_decimal_when_input_parses() {
    let an = AttemptNumber::parse("1").expect("valid");
    assert_eq!(an.as_u64(), 1);
}

#[test]
fn attempt_number_accepts_u64_max_when_at_upper_boundary() {
    let an = AttemptNumber::parse("18446744073709551615").expect("valid");
    assert_eq!(an.as_u64(), u64::MAX);
}

#[test]
fn attempt_number_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
    assert_eq!(
        AttemptNumber::parse("retry"),
        Err(ParseError::NotAnInteger {
            type_name: "AttemptNumber",
            input: "retry".to_string(),
        })
    );
}

#[test]
fn attempt_number_rejects_zero_with_zero_value_when_input_is_zero() {
    assert_eq!(
        AttemptNumber::parse("0"),
        Err(ParseError::ZeroValue {
            type_name: "AttemptNumber"
        })
    );
}

#[test]
fn attempt_number_accepts_minimum_when_value_is_1() {
    let an = AttemptNumber::parse("1").expect("valid");
    assert_eq!(an.as_u64(), 1);
}

#[test]
fn attempt_number_display_equals_decimal() {
    let an = AttemptNumber::new_unchecked(3);
    assert_eq!(format!("{an}"), "3");
}

#[test]
fn attempt_number_display_round_trips_through_parse_when_valid() {
    let an = AttemptNumber::new_unchecked(3);
    let s = format!("{an}");
    assert_eq!(AttemptNumber::parse(&s), Ok(an));
}

#[test]
fn attempt_number_new_unchecked_constructs_when_value_is_nonzero() {
    let an = AttemptNumber::new_unchecked(1);
    assert_eq!(an.as_u64(), 1);
}

#[test]
#[should_panic(expected = "AttemptNumber must be nonzero")]
fn attempt_number_new_unchecked_panics_when_value_is_zero() {
    let _val = AttemptNumber::new_unchecked(0);
}