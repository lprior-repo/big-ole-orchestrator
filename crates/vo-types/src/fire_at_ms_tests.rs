use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn fire_at_ms_accepts_zero_when_input_is_zero() {
    let fa = FireAtMs::parse("0").expect("valid");
    assert_eq!(fa.as_u64(), 0);
}

#[test]
fn fire_at_ms_accepts_nonzero_decimal_when_input_parses() {
    let fa = FireAtMs::parse("1710000000000").expect("valid");
    assert_eq!(fa.as_u64(), 1710000000000);
}

#[test]
fn fire_at_ms_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
    assert_eq!(
        FireAtMs::parse("soon"),
        Err(ParseError::NotAnInteger {
            type_name: "FireAtMs",
            input: "soon".to_string(),
        })
    );
}

#[test]
fn fire_at_ms_to_system_time_returns_correct_time_when_called() {
    let fa = FireAtMs(5000);
    assert_eq!(
        fa.to_system_time(),
        SystemTime::UNIX_EPOCH + Duration::from_millis(5000)
    );
}

#[test]
fn fire_at_ms_has_elapsed_returns_true_when_fire_at_is_before_now() {
    let fa = FireAtMs(1000);
    let now = TimestampMs(2000);
    assert!(fa.has_elapsed(now));
}

#[test]
fn fire_at_ms_has_elapsed_returns_false_when_fire_at_is_after_now() {
    let fa = FireAtMs(3000);
    let now = TimestampMs(2000);
    assert!(!fa.has_elapsed(now));
}

#[test]
fn fire_at_ms_has_elapsed_returns_false_when_fire_at_equals_now() {
    let fa = FireAtMs(2000);
    let now = TimestampMs(2000);
    assert!(
        !fa.has_elapsed(now),
        "has_elapsed must be false when times are equal"
    );
}

#[test]
fn fire_at_ms_accepts_u64_max_when_at_upper_boundary() {
    let fa = FireAtMs::parse("18446744073709551615").expect("valid");
    assert_eq!(fa.as_u64(), u64::MAX);
}

#[test]
fn fire_at_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
    assert_eq!(
        FireAtMs::parse("-1"),
        Err(ParseError::NotAnInteger {
            type_name: "FireAtMs",
            input: "-1".to_string(),
        })
    );
}

#[test]
fn fire_at_ms_display_equals_decimal() {
    let fa = FireAtMs(1710000000000);
    assert_eq!(format!("{fa}"), "1710000000000");
}

#[test]
fn fire_at_ms_display_round_trips_through_parse_when_valid() {
    let fa = FireAtMs(1710000000000);
    let s = format!("{fa}");
    assert_eq!(FireAtMs::parse(&s), Ok(fa));
}
