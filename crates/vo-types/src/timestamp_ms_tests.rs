use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn timestamp_ms_accepts_zero_when_input_is_zero() {
    let ts = TimestampMs::parse("0").expect("valid");
    assert_eq!(ts.as_u64(), 0);
}

#[test]
fn timestamp_ms_accepts_nonzero_decimal_when_input_parses() {
    let ts = TimestampMs::parse("1710000000000").expect("valid");
    assert_eq!(ts.as_u64(), 1710000000000);
}

#[test]
fn timestamp_ms_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
    assert_eq!(
        TimestampMs::parse("now"),
        Err(ParseError::NotAnInteger {
            type_name: "TimestampMs",
            input: "now".to_string(),
        })
    );
}

#[test]
fn timestamp_ms_to_system_time_returns_unix_epoch_when_value_is_zero() {
    let ts = TimestampMs(0);
    assert_eq!(ts.to_system_time(), SystemTime::UNIX_EPOCH);
}

#[test]
fn timestamp_ms_to_system_time_returns_correct_time_when_value_is_positive() {
    let ts = TimestampMs(1000);
    assert_eq!(
        ts.to_system_time(),
        SystemTime::UNIX_EPOCH + Duration::from_millis(1000)
    );
}

#[test]
fn timestamp_ms_now_returns_parseable_value_when_system_clock_available() {
    let ts = TimestampMs::now();
    let parsed = TimestampMs::parse(&ts.to_string()).expect("parseable");
    assert_eq!(parsed.as_u64(), ts.as_u64());
}

#[test]
fn timestamp_ms_now_is_approximately_current_time_when_called() {
    let ts = TimestampMs::now();
    let system_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock")
        .as_millis() as u64;
    let diff = if ts.as_u64() > system_ms {
        ts.as_u64() - system_ms
    } else {
        system_ms - ts.as_u64()
    };
    assert!(
        diff < 5000,
        "TimestampMs::now() should be within 5000ms of system time, was {diff}ms off"
    );
}

#[test]
fn timestamp_ms_accepts_u64_max_when_at_upper_boundary() {
    let ts = TimestampMs::parse("18446744073709551615").expect("valid");
    assert_eq!(ts.as_u64(), u64::MAX);
}

#[test]
fn timestamp_ms_rejects_negative_with_not_an_integer_when_input_starts_with_minus() {
    assert_eq!(
        TimestampMs::parse("-1"),
        Err(ParseError::NotAnInteger {
            type_name: "TimestampMs",
            input: "-1".to_string(),
        })
    );
}

#[test]
fn timestamp_ms_display_equals_decimal() {
    let ts = TimestampMs(1710000000000);
    assert_eq!(format!("{ts}"), "1710000000000");
}

#[test]
fn timestamp_ms_display_round_trips_through_parse_when_valid() {
    let ts = TimestampMs(1710000000000);
    let s = format!("{ts}");
    assert_eq!(TimestampMs::parse(&s), Ok(ts));
}