#![allow(clippy::write_literal)]
use super::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn create_test_rules() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
rules:
  - id: SPEC-001
    name: test-rule
    severity: error
    description: "Test rule"
    banned_phrases: []
  - id: SPEC-002
    name: every-state-transition-has-invariant-check
    severity: warning
    description: "State transitions need invariants"
  - id: SPEC-004
    name: every-behavior-has-acceptance-criterion
    severity: warning
    description: "Behaviors need acceptance criteria"
  - id: SPEC-010
    name: no-ambiguous-language
    severity: warning
    description: "Test ambiguous language"
    banned_phrases:
      - "should probably"
      - "obviously"
      - "simply"
  - id: SPEC-011
    name: concrete-error-responses
    severity: error
    description: "Error responses need HTTP codes"
  - id: SPEC-020
    name: enumeration-prevention
    severity: error
    description: "Enumeration prevention"
  - id: SPEC-021
    name: rate-limiting-specified
    severity: warning
    description: "Rate limiting required"
  - id: SPEC-030
    name: behaviors-are-observable
    severity: warning
    description: "Behaviors must be observable"
  - id: SPEC-040
    name: canvas-behavior-requires-visual-feedback
    severity: warning
    description: "Canvas needs visual feedback"
"#
    )?;
    Ok(file)
}

fn create_test_spec_minimal() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-test
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: test-behavior
      description: "Test"
      then:
        - "HTTP response returned"
  acceptance_criteria:
    - id: ac-01
      behavior_ref: test-behavior
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

fn create_rules_for_completeness_underflow() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
rules:
  - id: SPEC-001
    name: dependency-failure-cases
    severity: error
    description: "Dependency failure edges"
    banned_phrases: []
  - id: SPEC-004
    name: every-behavior-has-acceptance-criterion
    severity: warning
    description: "Behaviors need acceptance"
  - id: SPEC-011
    name: concrete-error-responses
    severity: warning
    description: "Error responses"
"#
    )?;
    Ok(file)
}

fn create_rules_for_completeness_and_auth() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
rules:
  - id: SPEC-001
    name: dependency-failure-cases
    severity: error
    description: "Dependency failure edges"
  - id: SPEC-003
    name: explicit-auth
    severity: error
    description: "API endpoints need authentication"
  - id: SPEC-004
    name: every-behavior-has-acceptance-criterion
    severity: warning
    description: "Behaviors need acceptance"
  - id: SPEC-011
    name: concrete-error-responses
    severity: warning
    description: "Error responses"
"#
    )?;
    Ok(file)
}

fn create_invalid_rules(content: &str) -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(file, "{}", content)?;
    Ok(file)
}

fn create_spec_with_missing_dependency_error_edges() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-underflow
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies:
      - service: Redis
        purpose: cache
        twin_available: true
      - service: Postgres
        purpose: db
        twin_available: true
    invariants: []
  behaviors:
    - id: behavior-1
      description: "No edge cases"
      then:
        - "System processes request"
  acceptance_criteria:
    - id: ac-01
      behavior_ref: behavior-1
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

fn create_spec_with_ambiguous_language() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-clarity
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Ambiguous"
      then:
        - "System should probably respond"
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

fn create_spec_with_enumeration_risk() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-security
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "No enumeration checks"
      then:
        - "HTTP response is returned"
  api_contract:
    endpoints:
      - method: POST
        path: /users/email/login
        authentication: bearer
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

fn create_spec_with_missing_auth_endpoint() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-auth
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Auth path"
      then:
        - "HTTP response is returned"
  api_contract:
    endpoints:
      - method: GET
        path: /v1/private
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

fn create_spec_with_non_observable_then_clause() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-testability
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Non observable"
      then:
        - "System updates internal cache"
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

fn create_spec_with_state_transitions_no_invariants() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-data-model
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Simple"
      then:
        - "HTTP response is returned"
  data_model:
    state_transitions:
      - from: draft
        to: active
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

#[test]
fn test_lint_spec_minimal() -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_test_spec_minimal()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert_eq!(report.spec_id, "spec-test");
    assert!(report.overall_score >= 80);
    assert!(report.errors.is_empty());

    Ok(())
}

#[test]
fn given_multiple_dependency_failures_when_checking_completeness_then_score_saturates_to_zero(
) -> anyhow::Result<()> {
    let rules_file = create_rules_for_completeness_underflow()?;
    let spec_file = create_spec_with_missing_dependency_error_edges()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert_eq!(report.errors.len(), 3);
    let completeness = report.categories.get("Completeness");
    assert!(completeness.is_some_and(|score| score.score == 0));
    Ok(())
}

#[test]
fn given_ambiguous_then_clause_when_linting_then_clarity_warning_is_reported() -> anyhow::Result<()>
{
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_ambiguous_language()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.rule_id == "SPEC-010"));
    Ok(())
}

#[test]
fn given_user_identifier_endpoint_without_enumeration_edge_case_when_linting_then_security_error_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_enumeration_risk()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .errors
        .iter()
        .any(|error| error.rule_id == "SPEC-020"));
    Ok(())
}

#[test]
fn given_unknown_rule_id_when_loading_rules_then_linter_returns_explicit_error(
) -> anyhow::Result<()> {
    let rules_file = create_invalid_rules(
        r#"
rules:
  - id: SPEC-999
    name: unknown-rule
    severity: error
    description: "Unknown rule"
"#,
    )?;

    let result = SpecLinter::new(rules_file.path());

    assert!(matches!(result, Err(LintError::UnknownRuleId { .. })));
    Ok(())
}

#[test]
fn given_invalid_rule_severity_when_loading_rules_then_linter_returns_explicit_error(
) -> anyhow::Result<()> {
    let rules_file = create_invalid_rules(
        r#"
rules:
  - id: SPEC-001
    name: invalid-severity
    severity: critical
    description: "Invalid severity"
"#,
    )?;

    let result = SpecLinter::new(rules_file.path());

    assert!(matches!(result, Err(LintError::InvalidSeverity { .. })));
    Ok(())
}

#[test]
fn given_missing_required_rule_field_when_loading_rules_then_linter_returns_explicit_error(
) -> anyhow::Result<()> {
    let rules_file = create_invalid_rules(
        r#"
rules:
  - id: SPEC-001
    name: ""
    severity: error
    description: "Missing name"
"#,
    )?;

    let result = SpecLinter::new(rules_file.path());

    assert!(matches!(
        result,
        Err(LintError::MissingRequiredField { ref field, .. }) if field == "name"
    ));
    Ok(())
}

#[test]
fn given_api_endpoint_without_auth_when_linting_then_spec_003_error_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_rules_for_completeness_and_auth()?;
    let spec_file = create_spec_with_missing_auth_endpoint()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .errors
        .iter()
        .any(|issue| issue.rule_id == "SPEC-003"));
    Ok(())
}

#[test]
fn given_non_observable_then_clause_when_linting_then_spec_030_warning_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_non_observable_then_clause()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .warnings
        .iter()
        .any(|issue| issue.rule_id == "SPEC-030"));
    Ok(())
}

#[test]
fn given_state_transitions_without_invariants_when_linting_then_spec_002_warning_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_state_transitions_no_invariants()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .warnings
        .iter()
        .any(|issue| issue.rule_id == "SPEC-002"));
    Ok(())
}

fn create_spec_with_behavior_missing_acceptance_criterion() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-004
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Has criterion"
      then:
        - "HTTP response is returned"
    - id: behavior-2
      description: "Missing criterion"
      then:
        - "System processes"
  acceptance_criteria:
    - id: ac-01
      behavior_ref: behavior-1
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

#[test]
fn given_behavior_without_acceptance_criterion_when_linting_then_spec_004_warning_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_behavior_missing_acceptance_criterion()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .warnings
        .iter()
        .any(|issue| issue.rule_id == "SPEC-004"));
    Ok(())
}

fn create_spec_with_concrete_error_responses() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-011
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Error without concrete code"
      then:
        - "System returns error"
    - id: behavior-2
      description: "Error with concrete code"
      then:
        - "Returns HTTP 400 Bad Request"
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

#[test]
fn given_error_without_concrete_http_status_when_linting_then_spec_011_error_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_concrete_error_responses()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .errors
        .iter()
        .any(|issue| issue.rule_id == "SPEC-011"));
    Ok(())
}

fn create_spec_with_write_endpoint_no_rate_limit() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-021
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: behavior-1
      description: "Create user"
      then:
        - "User is created"
  api_contract:
    endpoints:
      - method: POST
        path: /users
        authentication: bearer
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

#[test]
fn given_write_endpoint_without_rate_limit_when_linting_then_spec_021_warning_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_write_endpoint_no_rate_limit()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .warnings
        .iter()
        .any(|issue| issue.rule_id == "SPEC-021"));
    Ok(())
}

fn create_spec_with_canvas_behavior_no_feedback() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
specification:
  identity:
    id: spec-040
    version: 1.0.0
    status: draft
    author: test
    created: "2026-01-01T00:00:00Z"
  intent:
    problem_statement: "Test problem"
    success_criteria:
      - "Test criteria"
  context:
    system_dependencies: []
    invariants: []
  behaviors:
    - id: draw-canvas
      description: "Draw on canvas"
      then:
        - "Canvas is drawn"
  acceptance_criteria:
    - id: ac-01
      criterion: "Test criterion"
"#
    )?;
    Ok(file)
}

#[test]
fn given_canvas_behavior_without_visual_feedback_when_linting_then_spec_040_warning_is_reported(
) -> anyhow::Result<()> {
    let rules_file = create_test_rules()?;
    let spec_file = create_spec_with_canvas_behavior_no_feedback()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .warnings
        .iter()
        .any(|issue| issue.rule_id == "SPEC-040"));
    Ok(())
}

fn create_rules_with_error_severity() -> anyhow::Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    writeln!(
        file,
        "{}",
        r#"
rules:
  - id: SPEC-002
    name: every-state-transition-has-invariant-check
    severity: error
    description: "State transitions must have invariants"
  - id: SPEC-030
    name: behaviors-are-observable
    severity: error
    description: "Behaviors must be observable"
"#
    )?;
    Ok(file)
}

#[test]
fn given_spec_002_with_error_severity_when_linting_then_issue_is_reported_as_error(
) -> anyhow::Result<()> {
    let rules_file = create_rules_with_error_severity()?;
    let spec_file = create_spec_with_state_transitions_no_invariants()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .errors
        .iter()
        .any(|issue| issue.rule_id == "SPEC-002" && issue.severity == "error"));
    Ok(())
}

#[test]
fn given_spec_030_with_error_severity_when_linting_then_issue_is_reported_as_error(
) -> anyhow::Result<()> {
    let rules_file = create_rules_with_error_severity()?;
    let spec_file = create_spec_with_non_observable_then_clause()?;

    let linter = SpecLinter::new(rules_file.path())?;
    let report = linter.lint(spec_file.path())?;

    assert!(report
        .errors
        .iter()
        .any(|issue| issue.rule_id == "SPEC-030" && issue.severity == "error"));
    Ok(())
}
