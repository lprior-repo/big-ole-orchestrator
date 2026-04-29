use super::*;
use std::time::Duration;

#[test]
fn duration_ms_accepts_zero_when_input_is_zero() {
    let dm = DurationMs::parse("0").expect("valid");
    assert_eq!(dm.as_u64(), 0);
}

#[test]
fn duration_ms_accepts_nonzero_decimal_when_input_parses() {
    let dm = DurationMs::parse("1500").expect("valid");
    assert_eq!(dm.as_u64(), 1500);
}

#[test]
fn duration_ms_accepts_u64_max_when_at_upper_boundary() {
    let dm = DurationMs::parse("18446744073709551615").expect("valid");
    assert_eq!(dm.as_u64(), u64::MAX);
}

#[test]
fn duration_ms_rejects_non_integer_with_not_an_integer_when_input_is_float_string() {
    assert_eq!(
        DurationMs::parse("1.5s"),
        Err(ParseError::NotAnInteger {
            type_name: "DurationMs",
            input: "1.5s".to_string(),
        })
    );
}

#[test]
fn duration_ms_to_duration_returns_zero_duration_when_value_is_zero() {
    let dm = DurationMs(0);
    assert_eq!(dm.to_duration(), Duration::from_millis(0));
}

#[test]
fn duration_ms_to_duration_returns_correct_duration_when_value_is_nonzero() {
    let dm = DurationMs(2000);
    assert_eq!(dm.to_duration(), Duration::from_millis(2000));
}

#[test]
fn duration_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
    assert_eq!(
        DurationMs::parse("-1"),
        Err(ParseError::NotAnInteger {
            type_name: "DurationMs",
            input: "-1".to_string(),
        })
    );
}

#[test]
fn duration_ms_display_equals_decimal() {
    let dm = DurationMs(1500);
    assert_eq!(format!("{dm}"), "1500");
}

#[test]
fn duration_ms_display_round_trips_through_parse_when_valid() {
    let dm = DurationMs(1500);
    let s = format!("{dm}");
    assert_eq!(DurationMs::parse(&s), Ok(dm));
}
