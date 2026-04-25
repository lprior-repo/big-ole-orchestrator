use crate::ParseError;
use crate::*;

#[test]
fn binary_hash_accepts_valid_lowercase_hex_when_input_is_wellformed() {
    let bh =
        BinaryHash::parse("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            .expect("valid");
    assert_eq!(
        bh.as_str(),
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    );
}

#[test]
fn binary_hash_accepts_8_char_hex_when_at_minimum_boundary() {
    let bh = BinaryHash::parse("abcdef01").expect("valid");
    assert_eq!(bh.as_str(), "abcdef01");
}

#[test]
fn binary_hash_rejects_empty_with_empty_error_when_input_is_empty() {
    assert_eq!(
        BinaryHash::parse(""),
        Err(ParseError::Empty {
            type_name: "BinaryHash"
        })
    );
}

#[test]
fn binary_hash_rejects_uppercase_hex_with_invalid_chars_when_input_has_uppercase() {
    assert_eq!(
        BinaryHash::parse("ABCDEF0123456789"),
        Err(ParseError::InvalidCharacters {
            type_name: "BinaryHash",
            invalid_chars: "ABCDEF".to_string(),
        })
    );
}

#[test]
fn binary_hash_rejects_non_hex_with_invalid_chars_when_input_has_non_hex() {
    assert_eq!(
        BinaryHash::parse("ghijklmn"),
        Err(ParseError::InvalidCharacters {
            type_name: "BinaryHash",
            invalid_chars: "ghijklmn".to_string(),
        })
    );
}

#[test]
fn binary_hash_rejects_odd_length_with_invalid_format_when_length_is_odd() {
    let result = BinaryHash::parse("abc");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BinaryHash",
            ref reason
        }) if reason.contains("odd")
    ));
}

#[test]
fn binary_hash_rejects_too_short_with_invalid_format_when_length_is_less_than_8() {
    let result = BinaryHash::parse("ab");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BinaryHash",
            ref reason
        }) if reason.contains("8") || reason.contains("minimum")
    ));
}

#[test]
fn binary_hash_rejects_6_chars_with_invalid_format_when_below_minimum() {
    let result = BinaryHash::parse("abcdef");
    assert!(matches!(
        result,
        Err(ParseError::InvalidFormat {
            type_name: "BinaryHash",
            ref reason
        }) if reason.contains("8") || reason.contains("minimum")
    ));
}

#[test]
fn binary_hash_accepts_100_char_hex_when_within_valid_range() {
    let input = "a".repeat(100);
    let bh = BinaryHash::parse(&input).expect("valid");
    assert_eq!(bh.as_str().len(), 100);
}

#[test]
fn binary_hash_rejects_mixed_case_with_invalid_chars_when_input_has_uppercase() {
    let result = BinaryHash::parse("AbCdEf01");
    assert!(matches!(
        result,
        Err(ParseError::InvalidCharacters {
            type_name: "BinaryHash",
            ref invalid_chars
        }) if invalid_chars.chars().any(|c| c.is_ascii_uppercase())
    ));
}

#[test]
fn binary_hash_rejects_leading_whitespace_with_invalid_chars_when_input_has_space_prefix() {
    assert_eq!(
        BinaryHash::parse(" abcdef01"),
        Err(ParseError::InvalidCharacters {
            type_name: "BinaryHash",
            invalid_chars: " ".to_string(),
        })
    );
}

#[test]
fn binary_hash_rejects_trailing_whitespace_with_invalid_chars_when_input_has_space_suffix() {
    assert_eq!(
        BinaryHash::parse("abcdef01 "),
        Err(ParseError::InvalidCharacters {
            type_name: "BinaryHash",
            invalid_chars: " ".to_string(),
        })
    );
}

#[test]
fn binary_hash_accepts_all_zeros_when_at_minimum_boundary() {
    let bh = BinaryHash::parse("00000000").expect("valid");
    assert_eq!(bh.as_str(), "00000000");
}

#[test]
fn binary_hash_display_equals_inner_string() {
    let bh = BinaryHash::parse("abcdef0123456789").expect("valid");
    assert_eq!(format!("{bh}"), "abcdef0123456789");
}

#[test]
fn binary_hash_display_round_trips_through_parse_when_valid() {
    let bh =
        BinaryHash::parse("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
            .expect("valid");
    let s = format!("{bh}");
    assert_eq!(BinaryHash::parse(&s), Ok(bh));
}
