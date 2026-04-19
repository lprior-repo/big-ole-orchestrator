use rstest::rstest;
use vo_types::{ParseError, WorkflowName};

#[test]
fn workflow_name_parse_returns_ok_when_alphanumeric() {
    // Given
    let input = "validWorkflowName123";
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(result.unwrap().as_str(), input);
}

#[test]
fn workflow_name_parse_returns_ok_when_contains_single_hyphens() {
    // Given
    let input = "valid-workflow-name";
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(result.unwrap().as_str(), input);
}

#[test]
fn workflow_name_parse_returns_empty_error_when_input_is_empty() {
    // Given
    let input = "";
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(
        result,
        Err(ParseError::Empty {
            type_name: "WorkflowName"
        })
    );
}

#[test]
fn workflow_name_parse_returns_exceeds_max_length_error_when_input_is_too_long() {
    // Given
    let input = "a".repeat(129);
    // When
    let result = WorkflowName::parse(&input);
    // Then
    assert_eq!(
        result,
        Err(ParseError::ExceedsMaxLength {
            type_name: "WorkflowName",
            max: 128,
            actual: 129
        })
    );
}

#[test]
fn workflow_name_parse_returns_ok_when_contains_underscore() {
    // Given
    let input = "valid_workflow_name";
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(result.unwrap().as_str(), input);
}

#[test]
fn workflow_name_parse_returns_ok_when_starts_with_underscore() {
    // Given
    let input = "_validWorkflowName";
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(result.unwrap().as_str(), input);
}

#[rstest]
#[case("workflow name", " ")]
#[case("workflow@name", "@")]
#[case("workflow!name", "!")]
fn workflow_name_parse_returns_invalid_characters_error_when_input_has_invalid_chars(
    #[case] input: &str,
    #[case] invalid_chars: &str,
) {
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(
        result,
        Err(ParseError::InvalidCharacters {
            type_name: "WorkflowName",
            invalid_chars: invalid_chars.to_string()
        })
    );
}

#[rstest]
#[case("-leading", "must not start with hyphen")]
#[case("trailing-", "must not end with hyphen")]
#[case("trailing_", "must not end with underscore")]
fn workflow_name_parse_returns_boundary_violation_error_when_input_has_invalid_boundary_char(
    #[case] input: &str,
    #[case] reason: &str,
) {
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(
        result,
        Err(ParseError::BoundaryViolation {
            type_name: "WorkflowName",
            reason: reason.to_string()
        })
    );
}

#[rstest]
#[case("consecutive--hyphens")]
#[case("triple---hyphens")]
#[case("--leading")]
#[case("trailing--")]
fn workflow_name_parse_returns_consecutive_hyphens_error_when_input_has_double_hyphen(
    #[case] input: &str,
) {
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(
        result,
        Err(ParseError::ConsecutiveHyphens {
            type_name: "WorkflowName"
        })
    );
}

#[rstest]
#[case("consecutive__underscores")]
#[case("mixed-_separators")]
#[case("mixed_-separators")]
fn workflow_name_parse_returns_consecutive_separators_error_when_input_has_double_underscore_or_mixed(
    #[case] input: &str,
) {
    // When
    let result = WorkflowName::parse(input);
    // Then
    assert_eq!(
        result,
        Err(ParseError::ConsecutiveSeparators {
            type_name: "WorkflowName"
        })
    );
}

#[cfg(feature = "proptest")]
mod proptests {
    use proptest::prelude::*;
    use vo_types::{ParseError, WorkflowName};

    proptest! {
        #[test]
        fn workflow_name_parse_roundtrips_valid_input(
            s in "([a-zA-Z0-9_][a-zA-Z0-9_-]*[a-zA-Z0-9])|[a-zA-Z0-9]"
        ) {
            // Filter out strings with consecutive separators as they are invalid
            prop_assume!(!s.contains("--"));
            prop_assume!(!s.contains("__"));
            prop_assume!(!s.contains("-_"));
            prop_assume!(!s.contains("_-"));
            prop_assume!(s.len() <= 128);

            let result = WorkflowName::parse(&s);
            match result {
                Ok(val) => prop_assert_eq!(val.as_str(), s),
                Err(e) => prop_assert!(false, "Expected Ok, got {:?}", e),
            }
        }

        #[test]
        fn workflow_name_parse_rejects_consecutive_hyphens(
            s in "[a-zA-Z0-9_-]*--[a-zA-Z0-9_-]*"
        ) {
            prop_assume!(!s.is_empty());
            prop_assume!(s.len() <= 128);

            let result = WorkflowName::parse(&s);
            prop_assert_eq!(result, Err(ParseError::ConsecutiveHyphens {
                type_name: "WorkflowName"
            }));
        }

        #[test]
        fn workflow_name_parse_rejects_consecutive_separators(
            s in "[a-zA-Z0-9_-]*(__|-_|_-)[a-zA-Z0-9_-]*"
        ) {
            prop_assume!(!s.is_empty());
            prop_assume!(s.len() <= 128);
            prop_assume!(!s.contains("--"));

            let result = WorkflowName::parse(&s);
            prop_assert_eq!(result, Err(ParseError::ConsecutiveSeparators {
                type_name: "WorkflowName"
            }));
        }

        #[test]
        fn workflow_name_parse_rejects_invalid_chars(
            s in ".*[ !@#].*"
        ) {
            prop_assume!(!s.is_empty());
            prop_assume!(s.len() <= 128);

            let result = WorkflowName::parse(&s);
            let is_invalid = matches!(result, Err(ParseError::InvalidCharacters { .. }));
            prop_assert!(is_invalid, "Expected InvalidCharacters error, got {:?}", result);
        }
    }
}
