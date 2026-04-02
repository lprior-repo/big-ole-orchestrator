mod tests {
    use crate::ParseError;
    use crate::*;

    // ========== InstanceId ==========

    #[test]
    fn instance_id_accepts_valid_ulid_when_input_is_wellformed() {
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn instance_id_rejects_empty_with_empty_error_when_input_is_empty() {
        assert_eq!(
            InstanceId::parse(""),
            Err(ParseError::Empty {
                type_name: "InstanceId"
            })
        );
    }

    #[test]
    fn instance_id_rejects_wrong_length_with_invalid_format_when_input_is_not_26_chars() {
        let result = InstanceId::parse("01H5JYV4XH");
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ref reason
            }) if reason.contains("26")
        ));
    }

    #[test]
    fn instance_id_rejects_invalid_chars_with_invalid_format_when_input_has_non_crockford_chars() {
        let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFM@");
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ..
            })
        ));
    }

    #[test]
    fn instance_id_rejects_malformed_ulid_with_invalid_format_when_ulid_validation_fails() {
        let result = InstanceId::parse("00000000000000000000000000");
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ref reason
            }) if reason.to_lowercase().contains("validation")
                || reason.to_lowercase().contains("ulid")
                || reason.to_lowercase().contains("nil")
        ));
    }

    #[test]
    fn instance_id_rejects_long_input_with_invalid_format_when_input_exceeds_26_chars() {
        let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMAAAA");
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ref reason
            }) if reason.contains("26")
        ));
    }

    #[test]
    fn instance_id_rejects_leading_whitespace_with_invalid_format_when_input_has_space_prefix() {
        let result = InstanceId::parse(" 01H5JYV4XHGSR2F8KZ9BWNRFMA");
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ..
            })
        ));
    }

    #[test]
    fn instance_id_rejects_trailing_whitespace_with_invalid_format_when_input_has_space_suffix() {
        let result = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA ");
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ..
            })
        ));
    }

    #[test]
    fn instance_id_display_equals_inner_string() {
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        assert_eq!(format!("{id}"), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn instance_id_display_round_trips_through_parse_when_valid() {
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let s = format!("{id}");
        assert_eq!(InstanceId::parse(&s), Ok(id));
    }

    #[test]
    fn instance_id_try_from_string_valid() {
        let id = InstanceId::try_from("01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string()).expect("valid");
        assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn instance_id_try_from_string_invalid() {
        let result = InstanceId::try_from("bad".to_string());
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                type_name: "InstanceId",
                ..
            })
        ));
    }

    #[test]
    fn instance_id_from_into_string() {
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let s: String = id.into();
        assert_eq!(s, "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    // ========== WorkflowName ==========

    #[test]
    fn workflow_name_accepts_valid_identifier_when_chars_match_pattern() {
        let wn = WorkflowName::parse("deploy-production_v2").expect("valid");
        assert_eq!(wn.as_str(), "deploy-production_v2");
    }

    #[test]
    fn workflow_name_rejects_empty_with_empty_error_when_input_is_empty() {
        assert_eq!(
            WorkflowName::parse(""),
            Err(ParseError::Empty {
                type_name: "WorkflowName"
            })
        );
    }

    #[test]
    fn workflow_name_rejects_invalid_chars_when_input_contains_space() {
        assert_eq!(
            WorkflowName::parse("deploy job"),
            Err(ParseError::InvalidCharacters {
                type_name: "WorkflowName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn workflow_name_rejects_exceeds_max_length_when_input_is_129_chars() {
        let input = "a".repeat(129);
        assert_eq!(
            WorkflowName::parse(&input),
            Err(ParseError::ExceedsMaxLength {
                type_name: "WorkflowName",
                max: 128,
                actual: 129,
            })
        );
    }

    #[test]
    fn workflow_name_accepts_exactly_128_chars_when_at_boundary() {
        let input = "a".repeat(128);
        let wn = WorkflowName::parse(&input).expect("valid");
        assert_eq!(wn.as_str().len(), 128);
    }

    #[test]
    fn workflow_name_rejects_leading_hyphen_with_boundary_violation_when_starts_with_hyphen() {
        let result = WorkflowName::parse("-deploy");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "WorkflowName",
                ref reason
            }) if reason.contains("hyphen")
        ));
    }

    #[test]
    fn workflow_name_rejects_trailing_hyphen_with_boundary_violation_when_ends_with_hyphen() {
        let result = WorkflowName::parse("deploy-");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "WorkflowName",
                ref reason
            }) if reason.contains("hyphen")
        ));
    }

    #[test]
    fn workflow_name_rejects_trailing_underscore_with_boundary_violation_when_ends_with_underscore()
    {
        let result = WorkflowName::parse("deploy_");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "WorkflowName",
                ref reason
            }) if reason.contains("underscore")
        ));
    }

    #[test]
    fn workflow_name_rejects_hyphen_only_with_boundary_violation_when_input_is_single_hyphen() {
        let result = WorkflowName::parse("-");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "WorkflowName",
                ref reason
            }) if reason.contains("hyphen")
        ));
    }

    #[test]
    fn workflow_name_rejects_underscore_only_with_boundary_violation_when_input_is_single_underscore(
    ) {
        let result = WorkflowName::parse("_");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "WorkflowName",
                ref reason
            }) if reason.contains("underscore")
        ));
    }

    // === CONTRACT VIOLATION FIX TESTS (vel-205) ===
    // These tests document the CORRECT behavior after the bug fix.
    // They FAIL against the buggy code and PASS after the fix.

    #[test]
    fn workflow_name_returns_ok_when_input_starts_with_underscore() {
        // CONTRACT FIX: "_valid" should parse successfully.
        // This test FAILS on buggy code (returns Err), PASSES after fix (returns Ok).
        let wn = WorkflowName::parse("_valid").expect("underscore prefix should be valid");
        assert_eq!(wn.as_str(), "_valid");
    }

    #[test]
    fn workflow_name_returns_ok_when_input_is_underscore_prefixed_identifier() {
        // Additional underscore-prefix cases
        assert_eq!(WorkflowName::parse("_abc").expect("valid").as_str(), "_abc");
        assert_eq!(WorkflowName::parse("_123").expect("valid").as_str(), "_123");
        assert_eq!(
            WorkflowName::parse("_abc_def").expect("valid").as_str(),
            "_abc_def"
        );
        assert_eq!(
            WorkflowName::parse("_abc-def").expect("valid").as_str(),
            "_abc-def"
        );
    }

    #[test]
    fn workflow_name_single_underscore_returns_suffix_error_after_fix() {
        // After fix, single underscore "_" should fail with SUFFIX error (not prefix).
        // This is because the leading underscore rule is removed, so it falls through
        // to the trailing underscore check.
        let result = WorkflowName::parse("_");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "WorkflowName",
                ref reason
            }) if reason.contains("end with underscore")
        ));
    }

    #[test]
    fn workflow_name_rejects_leading_whitespace_with_invalid_chars_when_input_starts_with_space() {
        assert_eq!(
            WorkflowName::parse(" deploy"),
            Err(ParseError::InvalidCharacters {
                type_name: "WorkflowName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn workflow_name_accepts_single_char_when_input_is_one_valid_character() {
        let wn = WorkflowName::parse("a").expect("valid");
        assert_eq!(wn.as_str(), "a");
    }

    #[test]
    fn workflow_name_accepts_valid_with_hyphen_when_input_contains_hyphen() {
        let wn = WorkflowName::parse("deploy-production").expect("valid");
        assert_eq!(wn.as_str(), "deploy-production");
    }

    #[test]
    fn workflow_name_accepts_valid_with_underscore_when_input_contains_underscore() {
        let wn = WorkflowName::parse("deploy_production").expect("valid");
        assert_eq!(wn.as_str(), "deploy_production");
    }

    #[test]
    fn workflow_name_accepts_valid_with_digits_when_input_contains_digits() {
        let wn = WorkflowName::parse("v2-node").expect("valid");
        assert_eq!(wn.as_str(), "v2-node");
    }

    #[test]
    fn workflow_name_rejects_trailing_whitespace_with_invalid_chars_when_input_ends_with_space() {
        assert_eq!(
            WorkflowName::parse("deploy "),
            Err(ParseError::InvalidCharacters {
                type_name: "WorkflowName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn workflow_name_rejects_null_byte_with_invalid_chars_when_input_contains_null() {
        let result = WorkflowName::parse("deploy\x00");
        assert!(matches!(
            result,
            Err(ParseError::InvalidCharacters {
                type_name: "WorkflowName",
                ref invalid_chars
            }) if invalid_chars.contains('\x00')
        ));
    }

    #[test]
    fn workflow_name_rejects_unicode_combining_char_with_invalid_chars_when_input_has_composing_mark(
    ) {
        let result = WorkflowName::parse("deploy-cafe\u{301}");
        assert!(matches!(
            result,
            Err(ParseError::InvalidCharacters {
                type_name: "WorkflowName",
                ref invalid_chars
            }) if !invalid_chars.is_empty()
        ));
    }

    #[test]
    fn workflow_name_rejects_whitespace_only_with_invalid_chars_when_input_is_single_space() {
        assert_eq!(
            WorkflowName::parse(" "),
            Err(ParseError::InvalidCharacters {
                type_name: "WorkflowName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn workflow_name_rejects_consecutive_hyphens_with_specific_error() {
        assert_eq!(
            WorkflowName::parse("deploy--prod"),
            Err(ParseError::ConsecutiveHyphens {
                type_name: "WorkflowName"
            })
        );
    }

    #[test]
    fn workflow_name_rejects_consecutive_underscores_with_specific_error() {
        assert_eq!(
            WorkflowName::parse("deploy__prod"),
            Err(ParseError::ConsecutiveSeparators {
                type_name: "WorkflowName"
            })
        );
    }

    #[test]
    fn workflow_name_rejects_mixed_separators_hyphen_underscore_with_specific_error() {
        assert_eq!(
            WorkflowName::parse("deploy-_prod"),
            Err(ParseError::ConsecutiveSeparators {
                type_name: "WorkflowName"
            })
        );
    }

    #[test]
    fn workflow_name_rejects_mixed_separators_underscore_hyphen_with_specific_error() {
        assert_eq!(
            WorkflowName::parse("deploy_-prod"),
            Err(ParseError::ConsecutiveSeparators {
                type_name: "WorkflowName"
            })
        );
    }

    #[test]
    fn workflow_name_display_equals_inner_string() {
        let wn = WorkflowName::parse("deploy-prod").expect("valid");
        assert_eq!(format!("{wn}"), "deploy-prod");
    }

    #[test]
    fn workflow_name_display_round_trips_through_parse_when_valid() {
        let wn = WorkflowName::parse("deploy-prod").expect("valid");
        let s = format!("{wn}");
        assert_eq!(WorkflowName::parse(&s), Ok(wn));
    }

    // ========== NodeName ==========

    #[test]
    fn node_name_accepts_valid_identifier_when_chars_match_pattern() {
        let nn = NodeName::parse("compile-artifact").expect("valid");
        assert_eq!(nn.as_str(), "compile-artifact");
    }

    #[test]
    fn node_name_rejects_empty_with_empty_error_when_input_is_empty() {
        assert_eq!(
            NodeName::parse(""),
            Err(ParseError::Empty {
                type_name: "NodeName"
            })
        );
    }

    #[test]
    fn node_name_rejects_invalid_chars_when_input_contains_space() {
        assert_eq!(
            NodeName::parse("compile artifact"),
            Err(ParseError::InvalidCharacters {
                type_name: "NodeName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn node_name_rejects_exceeds_max_length_when_input_is_129_chars() {
        let input = "a".repeat(129);
        assert_eq!(
            NodeName::parse(&input),
            Err(ParseError::ExceedsMaxLength {
                type_name: "NodeName",
                max: 128,
                actual: 129,
            })
        );
    }

    #[test]
    fn node_name_accepts_exactly_128_chars_when_at_boundary() {
        let input = "a".repeat(128);
        let nn = NodeName::parse(&input).expect("valid");
        assert_eq!(nn.as_str().len(), 128);
    }

    #[test]
    fn node_name_rejects_leading_hyphen_with_boundary_violation_when_starts_with_hyphen() {
        let result = NodeName::parse("-compile");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "NodeName",
                ref reason
            }) if reason.contains("hyphen")
        ));
    }

    #[test]
    fn node_name_rejects_trailing_hyphen_with_boundary_violation_when_ends_with_hyphen() {
        let result = NodeName::parse("compile-");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "NodeName",
                ref reason
            }) if reason.contains("hyphen")
        ));
    }

    #[test]
    fn node_name_rejects_trailing_underscore_with_boundary_violation_when_ends_with_underscore() {
        let result = NodeName::parse("compile_");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "NodeName",
                ref reason
            }) if reason.contains("underscore")
        ));
    }

    #[test]
    fn node_name_rejects_hyphen_only_with_boundary_violation_when_input_is_single_hyphen() {
        let result = NodeName::parse("-");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "NodeName",
                ref reason
            }) if reason.contains("hyphen")
        ));
    }

    #[test]
    fn node_name_rejects_underscore_only_with_boundary_violation_when_input_is_single_underscore() {
        let result = NodeName::parse("_");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "NodeName",
                ref reason
            }) if reason.contains("underscore")
        ));
    }

    #[test]
    fn node_name_rejects_consecutive_hyphens_with_specific_error() {
        assert_eq!(
            NodeName::parse("compile--artifact"),
            Err(ParseError::ConsecutiveHyphens {
                type_name: "NodeName"
            })
        );
    }

    #[test]
    fn node_name_rejects_consecutive_underscores_with_specific_error() {
        assert_eq!(
            NodeName::parse("compile__artifact"),
            Err(ParseError::ConsecutiveSeparators {
                type_name: "NodeName"
            })
        );
    }

    #[test]
    fn node_name_rejects_mixed_separators_hyphen_underscore_with_specific_error() {
        assert_eq!(
            NodeName::parse("compile-_artifact"),
            Err(ParseError::ConsecutiveSeparators {
                type_name: "NodeName"
            })
        );
    }

    #[test]
    fn node_name_rejects_mixed_separators_underscore_hyphen_with_specific_error() {
        assert_eq!(
            NodeName::parse("compile_-artifact"),
            Err(ParseError::ConsecutiveSeparators {
                type_name: "NodeName"
            })
        );
    }

    // === CONTRACT VIOLATION FIX TESTS (vel-205) ===
    // NodeName uses the same check_identifier_boundaries function, so it also
    // gains underscore-prefix acceptance after the fix.

    #[test]
    fn node_name_returns_ok_when_input_starts_with_underscore() {
        // CONTRACT FIX: "_node" should parse successfully.
        // This test FAILS on buggy code, PASSES after fix.
        let nn = NodeName::parse("_node").expect("underscore prefix should be valid");
        assert_eq!(nn.as_str(), "_node");
    }

    #[test]
    fn node_name_returns_ok_when_input_is_underscore_prefixed_identifier() {
        // Additional underscore-prefix cases for NodeName
        assert_eq!(
            NodeName::parse("_compute").expect("valid").as_str(),
            "_compute"
        );
        assert_eq!(NodeName::parse("_node1").expect("valid").as_str(), "_node1");
        assert_eq!(
            NodeName::parse("_compute_node").expect("valid").as_str(),
            "_compute_node"
        );
        assert_eq!(
            NodeName::parse("_compute-node").expect("valid").as_str(),
            "_compute-node"
        );
    }

    #[test]
    fn node_name_single_underscore_returns_suffix_error_after_fix() {
        // After fix, single underscore "_" should fail with SUFFIX error (not prefix).
        let result = NodeName::parse("_");
        assert!(matches!(
            result,
            Err(ParseError::BoundaryViolation {
                type_name: "NodeName",
                ref reason
            }) if reason.contains("end with underscore")
        ));
    }

    #[test]
    fn node_name_rejects_leading_whitespace_with_invalid_chars_when_input_starts_with_space() {
        assert_eq!(
            NodeName::parse(" compile"),
            Err(ParseError::InvalidCharacters {
                type_name: "NodeName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn node_name_accepts_single_char_when_input_is_one_valid_character() {
        let nn = NodeName::parse("a").expect("valid");
        assert_eq!(nn.as_str(), "a");
    }

    #[test]
    fn node_name_accepts_valid_with_hyphen_when_input_contains_hyphen() {
        let nn = NodeName::parse("compile-artifact").expect("valid");
        assert_eq!(nn.as_str(), "compile-artifact");
    }

    #[test]
    fn node_name_accepts_valid_with_underscore_when_input_contains_underscore() {
        let nn = NodeName::parse("compile_artifact").expect("valid");
        assert_eq!(nn.as_str(), "compile_artifact");
    }

    #[test]
    fn node_name_accepts_valid_with_digits_when_input_contains_digits() {
        let nn = NodeName::parse("node-42").expect("valid");
        assert_eq!(nn.as_str(), "node-42");
    }

    #[test]
    fn node_name_rejects_trailing_whitespace_with_invalid_chars_when_input_ends_with_space() {
        assert_eq!(
            NodeName::parse("compile "),
            Err(ParseError::InvalidCharacters {
                type_name: "NodeName",
                invalid_chars: " ".to_string(),
            })
        );
    }

    #[test]
    fn node_name_rejects_null_byte_with_invalid_chars_when_input_contains_null() {
        let result = NodeName::parse("compile\x00");
        assert!(matches!(
            result,
            Err(ParseError::InvalidCharacters {
                type_name: "NodeName",
                ref invalid_chars
            }) if invalid_chars.contains('\x00')
        ));
    }

    #[test]
    fn node_name_display_equals_inner_string() {
        let nn = NodeName::parse("compile-artifact").expect("valid");
        assert_eq!(format!("{nn}"), "compile-artifact");
    }

    #[test]
    fn node_name_display_round_trips_through_parse_when_valid() {
        let nn = NodeName::parse("compile-artifact").expect("valid");
        let s = format!("{nn}");
        assert_eq!(NodeName::parse(&s), Ok(nn));
    }

    // ========== BinaryHash ==========

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

    // ========== TimerId ==========

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

    // ========== IdempotencyKey ==========

    #[test]
    fn idempotency_key_accepts_non_empty_string_when_within_length_limit() {
        let ik = IdempotencyKey::parse("key-20240101-abc").expect("valid");
        assert_eq!(ik.as_str(), "key-20240101-abc");
    }

    #[test]
    fn idempotency_key_accepts_any_non_empty_chars_when_opaque_string() {
        let ik = IdempotencyKey::parse("key@\t\n!()").expect("valid");
        assert_eq!(ik.as_str(), "key@\t\n!()");
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
    fn idempotency_key_accepts_unicode_when_input_has_non_ascii_chars() {
        let ik = IdempotencyKey::parse("key-\u{00e9}").expect("valid");
        assert_eq!(ik.as_str(), "key-\u{00e9}");
    }

    #[test]
    fn idempotency_key_accepts_trailing_whitespace_when_opaque_type_preserves_input() {
        let ik = IdempotencyKey::parse("key ").expect("valid");
        assert_eq!(ik.as_str(), "key ");
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

    // ========== to_bytes / from_bytes ==========

    #[test]
    fn instance_id_to_bytes_returns_correct_bytes_for_valid_ulid() {
        let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let bytes = id.to_bytes().expect("valid ULID should convert to bytes");
        assert_ne!(bytes, [0; 16]);
        assert_ne!(bytes, [1; 16]);
        let reconstructed = InstanceId::from_bytes(bytes);
        assert_eq!(reconstructed.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
    }

    #[test]
    fn instance_id_to_bytes_returns_error_when_ulid_invalid() {
        let id = InstanceId("invalid".to_string());
        assert!(matches!(
            id.to_bytes(),
            Err(ParseError::InvalidFormat { .. })
        ));
    }

    // ========== Serde round-trip (inline) ==========

    #[test]
    fn serde_round_trip_instance_id_inline() {
        let original = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: InstanceId = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_workflow_name_inline() {
        let original = WorkflowName::parse("deploy-prod").expect("valid");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: WorkflowName = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_node_name_inline() {
        let original = NodeName::parse("compile-artifact").expect("valid");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: NodeName = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_binary_hash_inline() {
        let original = BinaryHash::parse("abcdef0123456789").expect("valid");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: BinaryHash = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_timer_id_inline() {
        let original = TimerId::parse("timer-123").expect("valid");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: TimerId = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn serde_round_trip_idempotency_key_inline() {
        let original = IdempotencyKey::parse("key-abc").expect("valid");
        let json = serde_json::to_value(&original).expect("serialize");
        let restored: IdempotencyKey = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, original);
    }

    // ========== Proptest round-trips ==========

    #[cfg(feature = "proptest")]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn workflow_name_round_trip_proptest(s in "([a-zA-Z0-9_][a-zA-Z0-9_-]*[a-zA-Z0-9])|[a-zA-Z0-9]".prop_filter("no consecutive separators", |s| !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-"))) {
                prop_assume!(s.len() <= 128);
                let v = WorkflowName(s);
                prop_assert_eq!(WorkflowName::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn node_name_round_trip_proptest(s in "[a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9]") {
                let v = NodeName(s);
                prop_assert_eq!(NodeName::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn binary_hash_round_trip_proptest(byte_len in 4u32..128u32) {
                let hex_len = (byte_len * 2) as usize;
                let s: String = "0123456789abcdef".chars().cycle().take(hex_len).collect();
                let v = BinaryHash(s);
                prop_assert_eq!(BinaryHash::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn timer_id_round_trip_proptest(s in ".{1,256}") {
                let v = TimerId(s);
                prop_assert_eq!(TimerId::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn idempotency_key_round_trip_proptest(s in ".{1,1024}") {
                let v = IdempotencyKey(s);
                prop_assert_eq!(IdempotencyKey::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn instance_id_round_trip_proptest(s in "[0-7][0-9A-HJKMNP-TV-Z]{25}") {
                let v = InstanceId(s);
                prop_assert_eq!(InstanceId::parse(&v.to_string()), Ok(v));
            }

            #[test]
            fn serde_round_trip_workflow_name_proptest(s in "([a-zA-Z0-9_][a-zA-Z0-9_-]*[a-zA-Z0-9])|[a-zA-Z0-9]".prop_filter("no consecutive separators", |s| !s.contains("--") && !s.contains("__") && !s.contains("-_") && !s.contains("_-"))) {
                prop_assume!(s.len() <= 128);
                let v = WorkflowName(s);
                let json = serde_json::to_value(&v).expect("serialize");
                let restored: WorkflowName = serde_json::from_value(json).expect("deserialize");
                prop_assert_eq!(restored, v);
            }

            #[test]
            fn serde_round_trip_timer_id_proptest(s in ".{1,256}") {
                let v = TimerId(s);
                let json = serde_json::to_value(&v).expect("serialize");
                let restored: TimerId = serde_json::from_value(json).expect("deserialize");
                prop_assert_eq!(restored, v);
            }

            // === CONTRACT VIOLATION FIX PROPTESTS (vel-205) ===
            // These proptests verify the CORRECT behavior after the bug fix.

            #[test]
            fn node_name_underscore_prefix_round_trip_proptest(s in "_[a-zA-Z0-9][a-zA-Z0-9]") {
                let v = NodeName(s.clone());
                prop_assert_eq!(NodeName::parse(&v.to_string()), Ok(v));
            }
        }

        // === CONTRACT VIOLATION FIX PROPTEST (vel-205) ===
        // This test is outside proptest! because it uses fixed input (no strategy).
        // It runs as a regular unit test since it's in mod proptests but not proptest!
        #[test]
        fn boundary_consistency_invariant_underscore_prefix() {
            // Invariant: If is_identifier_char('_') == true,
            // then WorkflowName::parse("_valid") must succeed.
            // This is the core contract violation test (vel-205).
            assert_eq!(
                WorkflowName::parse("_valid"),
                Ok(WorkflowName("_valid".to_string()))
            );
        }
    }

    // === CONTRACT VIOLATION FIX UNIT TESTS (vel-205) ===
    // These are additional unit tests outside proptest to ensure
    // the fix is verified at compile time and in normal test runs.

    #[test]
    fn workflow_name_boundary_consistency_contract_underscore_prefix_is_valid() {
        // This is the PRIMARY contract violation test (B8 from test plan).
        // Since is_identifier_char('_') returns true, "_valid" MUST parse successfully.
        let result = WorkflowName::parse("_valid");
        assert_eq!(
            result,
            Ok(WorkflowName("_valid".to_string())),
            "CONTRACT VIOLATION: is_identifier_char('_') returns true, therefore \
             WorkflowName::parse(\"_valid\") MUST return Ok, but it returned an error. \
             This is the bug that vel-205 fixes."
        );
    }

    #[test]
    fn node_name_boundary_consistency_contract_underscore_prefix_is_valid() {
        // Same contract violation test for NodeName (shares check_identifier_boundaries).
        let result = NodeName::parse("_node");
        assert_eq!(
            result,
            Ok(NodeName("_node".to_string())),
            "CONTRACT VIOLATION: NodeName::parse(\"_node\") must succeed per contract"
        );
    }
}
