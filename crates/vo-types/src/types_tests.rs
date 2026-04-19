mod tests {
    use crate::types::*;
    use crate::ParseError;
    use std::num::NonZeroU64;

    #[test]
    fn task_failure_kind_user_has_correct_str() {
        use crate::TaskFailureKind;
        assert_eq!(TaskFailureKind::User.as_str(), "User");
    }

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
        check_identifier_boundaries("abc", "T").unwrap();
        check_identifier_boundaries("a-b_c", "T").unwrap();
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

    // === CONTRACT VIOLATION FIX TESTS (vel-205) ===
    // These tests document the CORRECT behavior after the bug fix.
    // They FAIL against the buggy code and PASS after the fix.

    #[test]
    fn check_boundaries_accepts_leading_underscore_after_fix() {
        // CONTRACT FIX: Since is_identifier_char('_') returns true,
        // leading underscore should be ACCEPTED (not rejected).
        // This test FAILS on buggy code, PASSES after fix.
        assert_eq!(check_identifier_boundaries("_abc", "T"), Ok(()));
    }

    #[test]
    fn check_boundaries_single_underscore_rejects_suffix_after_fix() {
        // After fixing the leading underscore bug, single underscore "_"
        // should be caught by the trailing underscore check (not leading).
        // This is the CORRECT error: suffix error takes priority.
        assert_eq!(
            check_identifier_boundaries("_", "T"),
            Err(ParseError::BoundaryViolation {
                type_name: "T",
                reason: "must not end with underscore".to_string(),
            })
        );
    }

    // --- proptest ---

    #[cfg(feature = "proptest")]
    mod proptests {
        use super::*;
        use proptest::prop_assert;

        #[test]
        fn extract_invalid_chars_proptest() {
            proptest::proptest!(|(s in "[a-zA-Z0-9_-]{0,100}")| {
                let invalid = extract_invalid_chars(&s, is_identifier_char);
                // Every char in the invalid set must NOT be an identifier char
                prop_assert!(invalid.chars().all(|c| !is_identifier_char(c)));
            });
        }
    }
}
