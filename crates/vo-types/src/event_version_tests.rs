use super::*;

#[test]
fn event_version_accepts_valid_nonzero_decimal_when_input_parses() {
    let ev = EventVersion::parse("1").expect("valid");
    assert_eq!(ev.as_u64(), 1);
}

#[test]
fn event_version_accepts_u64_max_when_at_upper_boundary() {
    let ev = EventVersion::parse("18446744073709551615").expect("valid");
    assert_eq!(ev.as_u64(), u64::MAX);
}

#[test]
fn event_version_rejects_non_integer_with_not_an_integer_when_input_is_alpha() {
    assert_eq!(
        EventVersion::parse("not-a-version"),
        Err(ParseError::NotAnInteger {
            type_name: "EventVersion",
            input: "not-a-version".to_string(),
        })
    );
}

#[test]
fn event_version_rejects_zero_with_zero_value_when_input_is_zero() {
    assert_eq!(
        EventVersion::parse("0"),
        Err(ParseError::ZeroValue {
            type_name: "EventVersion"
        })
    );
}

#[test]
fn event_version_accepts_minimum_when_value_is_1() {
    let ev = EventVersion::parse("1").expect("valid");
    assert_eq!(ev.as_u64(), 1);
}

#[test]
fn event_version_display_equals_decimal() {
    let ev = EventVersion::new_unchecked(1);
    assert_eq!(format!("{ev}"), "1");
}

#[test]
fn event_version_display_round_trips_through_parse_when_valid() {
    let ev = EventVersion::new_unchecked(1);
    let s = format!("{ev}");
    assert_eq!(EventVersion::parse(&s), Ok(ev));
}

#[test]
fn event_version_new_unchecked_constructs_when_value_is_nonzero() {
    let ev = EventVersion::new_unchecked(1);
    assert_eq!(ev.as_u64(), 1);
}

#[test]
#[should_panic(expected = "EventVersion must be nonzero")]
fn event_version_new_unchecked_panics_when_value_is_zero() {
    let _val = EventVersion::new_unchecked(0);
}