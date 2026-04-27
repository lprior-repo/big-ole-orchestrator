use crate::ParseError;
use crate::*;

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

#[test]
fn workflow_name_returns_ok_when_input_starts_with_underscore() {
    let wn = WorkflowName::parse("_valid").expect("underscore prefix should be valid");
    assert_eq!(wn.as_str(), "_valid");
}

#[test]
fn workflow_name_returns_ok_when_input_is_underscore_prefixed_identifier() {
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

#[test]
fn workflow_name_boundary_consistency_contract_underscore_prefix_is_valid() {
    let result = WorkflowName::parse("_valid");
    assert_eq!(
        result,
        Ok(WorkflowName("_valid".to_string())),
        "CONTRACT VIOLATION: is_identifier_char('_') returns true, therefore \
         WorkflowName::parse(\"_valid\") MUST return Ok, but it returned an error. \
         This is the bug that vel-205 fixes."
    );
}
