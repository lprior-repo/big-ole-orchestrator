use super::*;

#[test]
fn timer_id_accepts_non_empty_string_when_within_length_limit() {
    let ti = TimerId::parse("timer-abc-123").expect("valid");
    assert_eq!(ti.as_str(), "timer-abc-123");
}

#[test]
fn timer_id_accepts_any_non_empty_chars_when_opaque_string() {
    let ti = TimerId::parse("timer@#$%^&*()").expect("valid");
    assert_eq!(ti.as_str(), "timer@#$%^&*()");
}

#[test]
fn timer_id_rejects_empty_with_empty_error_when_input_is_empty() {
    assert_eq!(
        TimerId::parse(""),
        Err(ParseError::Empty {
            type_name: "TimerId"
        })
    );
}

#[test]
fn timer_id_rejects_exceeds_max_length_when_input_is_257_chars() {
    let input = "a".repeat(257);
    assert_eq!(
        TimerId::parse(&input),
        Err(ParseError::ExceedsMaxLength {
            type_name: "TimerId",
            max: 256,
            actual: 257,
        })
    );
}

#[test]
fn timer_id_accepts_exactly_256_chars_when_at_boundary() {
    let input = "a".repeat(256);
    let ti = TimerId::parse(&input).expect("valid");
    assert_eq!(ti.as_str().len(), 256);
}

#[test]
fn timer_id_accepts_single_char_when_input_is_one_character() {
    let ti = TimerId::parse("a").expect("valid");
    assert_eq!(ti.as_str(), "a");
}

#[test]
fn timer_id_accepts_unicode_when_input_has_non_ascii_chars() {
    let ti = TimerId::parse("\u{00e9}\u{00f1}").expect("valid");
    assert_eq!(ti.as_str(), "\u{00e9}\u{00f1}");
}

#[test]
fn timer_id_accepts_trailing_whitespace_when_opaque_type_preserves_input() {
    let ti = TimerId::parse("timer ").expect("valid");
    assert_eq!(ti.as_str(), "timer ");
}

#[test]
fn timer_id_display_equals_inner_string() {
    let ti = TimerId::parse("timer-123").expect("valid");
    assert_eq!(format!("{ti}"), "timer-123");
}

#[test]
fn timer_id_display_round_trips_through_parse_when_valid() {
    let ti = TimerId::parse("timer-123").expect("valid");
    let s = format!("{ti}");
    assert_eq!(TimerId::parse(&s), Ok(ti));
}