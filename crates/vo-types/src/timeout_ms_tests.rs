use super::*;
use std::time::Duration;

#[test]
fn timeout_ms_accepts_valid_nonzero_decimal_when_input_parses() {
    let tm = TimeoutMs::parse("5000").expect("valid");
    assert_eq!(tm.as_u64(), 5000);
}

#[test]
fn timeout_ms_accepts_minimum_when_value_is_1() {
    let tm = TimeoutMs::parse("1").expect("valid");
    assert_eq!(tm.as_u64(), 1);
}

#[test]
fn timeout_ms_rejects_non_integer_with_not_an_integer_when_input_is_duration_string() {
    assert_eq!(
        TimeoutMs::parse("5s"),
        Err(ParseError::NotAnInteger {
            type_name: "TimeoutMs",
            input: "5s".to_string(),
        })
    );
}

#[test]
fn timeout_ms_rejects_zero_with_zero_value_when_input_is_zero() {
    assert_eq!(
        TimeoutMs::parse("0"),
        Err(ParseError::ZeroValue {
            type_name: "TimeoutMs"
        })
    );
}

#[test]
fn timeout_ms_to_duration_returns_correct_duration_when_called() {
    let tm = TimeoutMs::new_unchecked(5000);
    assert_eq!(tm.to_duration(), Duration::from_millis(5000));
}

#[test]
fn timeout_ms_accepts_u64_max_when_at_upper_boundary() {
    let tm = TimeoutMs::parse("18446744073709551615").expect("valid");
    assert_eq!(tm.as_u64(), u64::MAX);
}

#[test]
fn timeout_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
    assert_eq!(
        TimeoutMs::parse("-1"),
        Err(ParseError::NotAnInteger {
            type_name: "TimeoutMs",
            input: "-1".to_string(),
        })
    );
}

#[test]
fn timeout_ms_display_equals_decimal() {
    let tm = TimeoutMs::new_unchecked(5000);
    assert_eq!(format!("{tm}"), "5000");
}

#[test]
fn timeout_ms_display_round_trips_through_parse_when_valid() {
    let tm = TimeoutMs::new_unchecked(5000);
    let s = format!("{tm}");
    assert_eq!(TimeoutMs::parse(&s), Ok(tm));
}

#[test]
fn timeout_ms_new_unchecked_constructs_when_value_is_nonzero() {
    let tm = TimeoutMs::new_unchecked(1000);
    assert_eq!(tm.as_u64(), 1000);
}

#[test]
#[should_panic(expected = "TimeoutMs must be nonzero")]
fn timeout_ms_new_unchecked_panics_when_value_is_zero() {
    let _val = TimeoutMs::new_unchecked(0);
}