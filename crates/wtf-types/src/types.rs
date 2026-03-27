pub(crate) fn extract_invalid_chars(input: &str, is_valid: impl Fn(char) -> bool) -> String {
    input.chars().filter(|&c| !is_valid(c)).collect()
}

pub(crate) fn parse_u64_str(
    input: &str,
    type_name: &'static str,
) -> Result<u64, crate::ParseError> {
    input
        .parse::<u64>()
        .map_err(|_| crate::ParseError::NotAnInteger {
            type_name,
            input: input.to_string(),
        })
}

pub(crate) fn require_nonzero(
    value: u64,
    type_name: &'static str,
) -> Result<std::num::NonZeroU64, crate::ParseError> {
    std::num::NonZeroU64::new(value).ok_or(crate::ParseError::ZeroValue { type_name })
}

pub(crate) fn parse_nonzero_u64(
    input: &str,
    type_name: &'static str,
) -> Result<std::num::NonZeroU64, crate::ParseError> {
    let value = parse_u64_str(input, type_name)?;
    require_nonzero(value, type_name)
}

pub(crate) fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

pub(crate) fn is_lowercase_hex(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='f')
}

pub(crate) fn check_identifier_boundaries(
    input: &str,
    type_name: &'static str,
) -> Result<(), crate::ParseError> {
    let first = input.chars().next();
    let last = input.chars().next_back();

    match (first, last) {
        (Some('-'), _) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not start with hyphen".to_string(),
        }),
        (Some('_'), _) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not start with underscore".to_string(),
        }),
        (_, Some('-')) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not end with hyphen".to_string(),
        }),
        (_, Some('_')) => Err(crate::ParseError::BoundaryViolation {
            type_name,
            reason: "must not end with underscore".to_string(),
        }),
        _ => Ok(()),
    }
}

pub use crate::integer_types::{
    AttemptNumber, DurationMs, EventVersion, FireAtMs, MaxAttempts, SequenceNumber, TimeoutMs,
    TimestampMs,
};
pub use crate::string_types::{
    BinaryHash, IdempotencyKey, InstanceId, NodeName, TimerId, WorkflowName,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParseError;
    use std::num::NonZeroU64;

    // --- extract_invalid_chars ---

    #[test]
    fn extract_invalid_chars_keeps_only_invalid_chars() {
        let result = extract_invalid_chars("abc123_-", |c| c.is_ascii_alphanumeric());
        assert_eq!(result, "_-");
    }

    #[test]
    fn extract_invalid_chars_returns_empty_when_all_valid() {
        let result = extract_invalid_chars("abc123", |c| c.is_ascii_alphanumeric());
        assert!(result.is_empty());
    }

    #[test]
    fn extract_invalid_chars_returns_all_when_none_valid() {
        let result = extract_invalid_chars("!@#", |c| c.is_ascii_alphanumeric());
        assert_eq!(result, "!@#");
    }

    #[test]
    fn extract_invalid_chars_empty_input() {
        let result = extract_invalid_chars("", |c| c.is_ascii_alphanumeric());
        assert!(result.is_empty());
    }

    // --- parse_u64_str ---

    #[test]
    fn parse_u64_str_valid_integer() {
        assert_eq!(parse_u64_str("42", "T"), Ok(42));
    }

    #[test]
    fn parse_u64_str_zero() {
        assert_eq!(parse_u64_str("0", "T"), Ok(0));
    }

    #[test]
    fn parse_u64_str_max() {
        assert_eq!(parse_u64_str("18446744073709551615", "T"), Ok(u64::MAX));
    }

    #[test]
    fn parse_u64_str_leading_zeros() {
        assert_eq!(parse_u64_str("007", "T"), Ok(7));
    }

    #[test]
    fn parse_u64_str_rejects_alpha() {
        assert_eq!(
            parse_u64_str("abc", "T"),
            Err(ParseError::NotAnInteger {
                type_name: "T",
                input: "abc".to_string(),
            })
        );
    }

    #[test]
    fn parse_u64_str_rejects_empty() {
        assert_eq!(
            parse_u64_str("", "T"),
            Err(ParseError::NotAnInteger {
                type_name: "T",
                input: "".to_string(),
            })
        );
    }

    #[test]
    fn parse_u64_str_rejects_negative() {
        assert_eq!(
            parse_u64_str("-1", "T"),
            Err(ParseError::NotAnInteger {
                type_name: "T",
                input: "-1".to_string(),
            })
        );
    }

    #[test]
    fn parse_u64_str_rejects_float() {
        assert_eq!(
            parse_u64_str("3.14", "T"),
            Err(ParseError::NotAnInteger {
                type_name: "T",
                input: "3.14".to_string(),
            })
        );
    }

    #[test]
    fn parse_u64_str_rejects_overflow() {
        assert_eq!(
            parse_u64_str("18446744073709551616", "T"),
            Err(ParseError::NotAnInteger {
                type_name: "T",
                input: "18446744073709551616".to_string(),
            })
        );
    }

    // --- require_nonzero ---

    #[test]
    fn require_nonzero_valid() {
        assert_eq!(
            require_nonzero(1, "T"),
            Ok(NonZeroU64::new(1).expect("nonzero"))
        );
    }

    #[test]
    fn require_nonzero_max() {
        assert_eq!(
            require_nonzero(u64::MAX, "T"),
            Ok(NonZeroU64::new(u64::MAX).expect("nonzero"))
        );
    }

    #[test]
    fn require_nonzero_zero() {
        assert_eq!(
            require_nonzero(0, "T"),
            Err(ParseError::ZeroValue { type_name: "T" })
        );
    }

    // --- parse_nonzero_u64 ---

    #[test]
    fn parse_nonzero_u64_valid() {
        assert_eq!(
            parse_nonzero_u64("42", "T"),
            Ok(NonZeroU64::new(42).expect("nonzero"))
        );
    }

    #[test]
    fn parse_nonzero_u64_zero() {
        assert_eq!(
            parse_nonzero_u64("0", "T"),
            Err(ParseError::ZeroValue { type_name: "T" })
        );
    }

    #[test]
    fn parse_nonzero_u64_non_integer() {
        assert_eq!(
            parse_nonzero_u64("abc", "T"),
            Err(ParseError::NotAnInteger {
                type_name: "T",
                input: "abc".to_string(),
            })
        );
    }

    // --- is_identifier_char ---

    #[test]
    fn is_identifier_char_accepts_alphanumeric() {
        assert!(is_identifier_char('a'));
        assert!(is_identifier_char('Z'));
        assert!(is_identifier_char('0'));
        assert!(is_identifier_char('9'));
    }

    #[test]
    fn is_identifier_char_accepts_hyphen_and_underscore() {
        assert!(is_identifier_char('-'));
        assert!(is_identifier_char('_'));
    }

    #[test]
    fn is_identifier_char_rejects_other_chars() {
        assert!(!is_identifier_char(' '));
        assert!(!is_identifier_char('@'));
        assert!(!is_identifier_char('.'));
        assert!(!is_identifier_char('\x00'));
        assert!(!is_identifier_char('\n'));
    }

    // --- is_lowercase_hex ---

    #[test]
    fn is_lowercase_hex_accepts_digits() {
        assert!(is_lowercase_hex('0'));
        assert!(is_lowercase_hex('9'));
    }

    #[test]
    fn is_lowercase_hex_accepts_lowercase_letters() {
        assert!(is_lowercase_hex('a'));
        assert!(is_lowercase_hex('f'));
    }

    #[test]
    fn is_lowercase_hex_rejects_uppercase() {
        assert!(!is_lowercase_hex('A'));
        assert!(!is_lowercase_hex('F'));
    }

    #[test]
    fn is_lowercase_hex_rejects_non_hex() {
        assert!(!is_lowercase_hex('g'));
        assert!(!is_lowercase_hex(' '));
        assert!(!is_lowercase_hex('-'));
    }

    // --- check_identifier_boundaries ---

    #[test]
    fn check_boundaries_valid() {
        assert!(check_identifier_boundaries("abc", "T").is_ok());
        assert!(check_identifier_boundaries("a-b_c", "T").is_ok());
    }

    #[test]
    fn check_boundaries_rejects_leading_hyphen() {
        assert_eq!(
            check_identifier_boundaries("-abc", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not start with hyphen".to_string(),
            })
        );
    }

    #[test]
    fn check_boundaries_rejects_leading_underscore() {
        assert_eq!(
            check_identifier_boundaries("_abc", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not start with underscore".to_string(),
            })
        );
    }

    #[test]
    fn check_boundaries_rejects_trailing_hyphen() {
        assert_eq!(
            check_identifier_boundaries("abc-", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not end with hyphen".to_string(),
            })
        );
    }

    #[test]
    fn check_boundaries_rejects_trailing_underscore() {
        assert_eq!(
            check_identifier_boundaries("abc_", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not end with underscore".to_string(),
            })
        );
    }

    #[test]
    fn check_boundaries_single_hyphen_rejects_leading() {
        assert_eq!(
            check_identifier_boundaries("-", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not start with hyphen".to_string(),
            })
        );
    }

    #[test]
    fn check_boundaries_single_underscore_rejects_leading() {
        assert_eq!(
            check_identifier_boundaries("_", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not start with underscore".to_string(),
            })
        );
    }

    // --- proptest ---

    mod proptests {
        use super::*;

        #[test]
        fn extract_invalid_chars_proptest() {
            proptest::proptest!(|(s in "[a-zA-Z0-9_-]{0,100}")| {
                let invalid = extract_invalid_chars(&s, is_identifier_char);
                // Every char in the invalid set must NOT be an identifier char
                for c in invalid.chars() {
                    assert!(!is_identifier_char(c));
                }
            });
        }
    }
}
