use super::types::StepId;

#[test]
fn stepid_returns_success_when_parsed_from_typical_identifier() {
    let sid = StepId::parse("step-1_A").unwrap();
    assert_eq!(sid.as_str(), "step-1_A");
}

#[test]
fn stepid_returns_success_when_parsed_from_single_character_boundary_scenario() {
    let sid = StepId::parse("A").unwrap();
    assert_eq!(sid.as_str(), "A");
}

#[test]
fn stepid_returns_success_when_parsed_from_single_numeric_boundary() {
    let sid = StepId::parse("1").unwrap();
    assert_eq!(sid.as_str(), "1");
}

#[test]
fn stepid_returns_success_when_parsed_from_long_identifier() {
    let sid = StepId::parse("a-very-long-step-identifier-123").unwrap();
    assert_eq!(sid.as_str(), "a-very-long-step-identifier-123");
}

#[test]
fn stepid_returns_success_when_parsed_from_consecutive_special_chars() {
    let sid = StepId::parse("step--1__A").unwrap();
    assert_eq!(sid.as_str(), "step--1__A");
}

#[test]
fn stepid_returns_invalid_character_error_when_parsed_from_empty_string_scenario() {
    let err = StepId::parse("").unwrap_err();
    assert!(matches!(
        err,
        crate::ParseError::Empty { .. } | crate::ParseError::InvalidCharacters { .. }
    ));
}

#[test]
fn stepid_returns_boundary_violation_error_when_parsed_from_string_starting_with_hyphen() {
    let err = StepId::parse("-step1").unwrap_err();
    assert!(matches!(err, crate::ParseError::BoundaryViolation { .. }));
}

#[test]
fn stepid_returns_boundary_violation_error_when_parsed_from_string_starting_with_underscore() {
    let err = StepId::parse("_step1").unwrap_err();
    assert!(matches!(err, crate::ParseError::BoundaryViolation { .. }));
}

#[test]
fn stepid_returns_boundary_violation_error_when_parsed_from_string_ending_with_hyphen() {
    let err = StepId::parse("step1-").unwrap_err();
    assert!(matches!(err, crate::ParseError::BoundaryViolation { .. }));
}

#[test]
fn stepid_returns_boundary_violation_error_when_parsed_from_string_ending_with_underscore() {
    let err = StepId::parse("step1_").unwrap_err();
    assert!(matches!(err, crate::ParseError::BoundaryViolation { .. }));
}

#[test]
fn stepid_returns_boundary_violation_error_when_parsed_from_single_hyphen() {
    let err = StepId::parse("-").unwrap_err();
    assert!(matches!(err, crate::ParseError::BoundaryViolation { .. }));
}

#[test]
fn stepid_returns_boundary_violation_error_when_parsed_from_single_underscore() {
    let err = StepId::parse("_").unwrap_err();
    assert!(matches!(err, crate::ParseError::BoundaryViolation { .. }));
}

#[test]
fn stepid_returns_invalid_character_error_when_parsed_from_special_characters() {
    let err = StepId::parse("step@1").unwrap_err();
    assert!(matches!(err, crate::ParseError::InvalidCharacters { .. }));
}

#[test]
fn stepid_returns_exact_string_slice_when_as_str_called_on_typical_identifier() {
    let sid = StepId::parse("step-1_A").unwrap();
    assert_eq!(sid.as_str(), "step-1_A");
}

#[test]
fn stepid_returns_exact_string_slice_when_as_str_called_on_single_character_boundary() {
    let sid = StepId::parse("A").unwrap();
    assert_eq!(sid.as_str(), "A");
}

#[test]
fn stepid_returns_exact_string_slice_when_as_str_called_on_numeric_boundary() {
    let sid = StepId::parse("1").unwrap();
    assert_eq!(sid.as_str(), "1");
}

#[test]
fn stepid_returns_exact_string_slice_when_as_str_called_on_long_identifier() {
    let sid = StepId::parse("long-identifier-string").unwrap();
    assert_eq!(sid.as_str(), "long-identifier-string");
}

#[cfg(feature = "proptest")]
mod stepid_proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn stepid_valid_regex_invariant_proptest(s in "[a-zA-Z0-9]([a-zA-Z0-9_-]*[a-zA-Z0-9])?") {
            let sid = StepId::parse(&s).unwrap();
            prop_assert_eq!(sid.as_str(), s);
        }
    }
}
