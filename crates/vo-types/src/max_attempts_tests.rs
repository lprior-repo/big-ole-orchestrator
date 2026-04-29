use super::*;

#[test]
fn max_attempts_accepts_valid_nonzero_decimal_when_input_parses() {
    let ma = MaxAttempts::parse("3").expect("valid");
    assert_eq!(ma.as_u64(), 3);
}

#[test]
fn max_attempts_accepts_minimum_when_value_is_1() {
    let ma = MaxAttempts::parse("1").expect("valid");
    assert_eq!(ma.as_u64(), 1);
}

#[test]
fn max_attempts_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
    assert_eq!(
        MaxAttempts::parse("unlimited"),
        Err(ParseError::NotAnInteger {
            type_name: "MaxAttempts",
            input: "unlimited".to_string(),
        })
    );
}

#[test]
fn max_attempts_rejects_zero_with_zero_value_when_input_is_zero() {
    assert_eq!(
        MaxAttempts::parse("0"),
        Err(ParseError::ZeroValue {
            type_name: "MaxAttempts"
        })
    );
}

#[test]
fn max_attempts_is_exhausted_returns_false_when_attempt_less_than_max() {
    let ma = MaxAttempts::new_unchecked(3);
    let attempt = AttemptNumber::new_unchecked(1);
    assert!(!ma.is_exhausted(attempt));
}

#[test]
fn max_attempts_is_exhausted_returns_false_when_attempt_is_max_minus_one() {
    let ma = MaxAttempts::new_unchecked(3);
    let attempt = AttemptNumber::new_unchecked(2);
    assert!(!ma.is_exhausted(attempt));
}

#[test]
fn max_attempts_is_exhausted_returns_true_when_attempt_equals_max() {
    let ma = MaxAttempts::new_unchecked(3);
    let attempt = AttemptNumber::new_unchecked(3);
    assert!(ma.is_exhausted(attempt));
}

#[test]
fn max_attempts_is_exhausted_returns_true_when_attempt_exceeds_max() {
    let ma = MaxAttempts::new_unchecked(3);
    let attempt = AttemptNumber::new_unchecked(5);
    assert!(ma.is_exhausted(attempt));
}

#[test]
fn max_attempts_is_exhausted_returns_true_when_max_is_1_and_attempt_is_1() {
    let ma = MaxAttempts::new_unchecked(1);
    let attempt = AttemptNumber::new_unchecked(1);
    assert!(ma.is_exhausted(attempt));
}

#[test]
fn max_attempts_accepts_u64_max_when_at_upper_boundary() {
    let ma = MaxAttempts::parse("18446744073709551615").expect("valid");
    assert_eq!(ma.as_u64(), u64::MAX);
}

#[test]
fn max_attempts_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
    assert_eq!(
        MaxAttempts::parse("-1"),
        Err(ParseError::NotAnInteger {
            type_name: "MaxAttempts",
            input: "-1".to_string(),
        })
    );
}

#[test]
fn max_attempts_display_equals_decimal() {
    let ma = MaxAttempts::new_unchecked(3);
    assert_eq!(format!("{ma}"), "3");
}

#[test]
fn max_attempts_display_round_trips_through_parse_when_valid() {
    let ma = MaxAttempts::new_unchecked(3);
    let s = format!("{ma}");
    assert_eq!(MaxAttempts::parse(&s), Ok(ma));
}

#[test]
fn max_attempts_new_unchecked_constructs_when_value_is_nonzero() {
    let ma = MaxAttempts::new_unchecked(3);
    assert_eq!(ma.as_u64(), 3);
}

#[test]
#[should_panic(expected = "MaxAttempts must be nonzero")]
fn max_attempts_new_unchecked_panics_when_value_is_zero() {
    let _val = MaxAttempts::new_unchecked(0);
}
