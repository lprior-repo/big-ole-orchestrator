use super::*;

#[test]
fn idempotency_key_accepts_non_empty_string_when_within_length_limit() {
    let ik = IdempotencyKey::parse("key-20240101-abc").expect("valid");
    assert_eq!(ik.as_str(), "key-20240101-abc");
}

#[test]
fn idempotency_key_rejects_non_identifier_chars_when_invalid_input() {
    let result = IdempotencyKey::parse("key@\t\n!()");
    assert!(
        result.is_err(),
        "IdempotencyKey must reject non-identifier chars"
    );
}

#[test]
fn idempotency_key_rejects_empty_with_empty_error_when_input_is_empty() {
    assert_eq!(
        IdempotencyKey::parse(""),
        Err(ParseError::Empty {
            type_name: "IdempotencyKey"
        })
    );
}

#[test]
fn idempotency_key_rejects_exceeds_max_length_when_input_is_1025_chars() {
    let input = "b".repeat(1025);
    assert_eq!(
        IdempotencyKey::parse(&input),
        Err(ParseError::ExceedsMaxLength {
            type_name: "IdempotencyKey",
            max: 1024,
            actual: 1025,
        })
    );
}

#[test]
fn idempotency_key_accepts_exactly_1024_chars_when_at_boundary() {
    let input = "b".repeat(1024);
    let ik = IdempotencyKey::parse(&input).expect("valid");
    assert_eq!(ik.as_str().len(), 1024);
}

#[test]
fn idempotency_key_accepts_single_char_when_input_is_one_character() {
    let ik = IdempotencyKey::parse("a").expect("valid");
    assert_eq!(ik.as_str(), "a");
}

#[test]
fn idempotency_key_rejects_unicode_when_input_has_non_ascii_chars() {
    let result = IdempotencyKey::parse("key-\u{00e9}");
    assert!(
        result.is_err(),
        "IdempotencyKey must reject non-ASCII chars"
    );
}

#[test]
fn idempotency_key_rejects_trailing_whitespace_because_not_identifier_char() {
    let result = IdempotencyKey::parse("key ");
    assert!(
        result.is_err(),
        "IdempotencyKey must reject whitespace (not identifier char)"
    );
}

#[test]
fn idempotency_key_display_equals_inner_string() {
    let ik = IdempotencyKey::parse("key-abc").expect("valid");
    assert_eq!(format!("{ik}"), "key-abc");
}

#[test]
fn idempotency_key_display_round_trips_through_parse_when_valid() {
    let ik = IdempotencyKey::parse("key-abc").expect("valid");
    let s = format!("{ik}");
    assert_eq!(IdempotencyKey::parse(&s), Ok(ik));
}