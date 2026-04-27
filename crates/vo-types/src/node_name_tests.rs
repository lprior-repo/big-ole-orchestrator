use super::*;

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

#[test]
fn node_name_returns_ok_when_input_starts_with_underscore() {
    let nn = NodeName::parse("_node").expect("underscore prefix should be valid");
    assert_eq!(nn.as_str(), "_node");
}

#[test]
fn node_name_returns_ok_when_input_is_underscore_prefixed_identifier() {
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