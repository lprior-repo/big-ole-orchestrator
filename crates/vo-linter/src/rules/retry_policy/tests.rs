//! Tests for retry policy bounds validation.

use crate::diagnostic::{LintCode, Severity};
use crate::rules::retry_policy::check_retry_policy_bounds;

#[test]
fn test_all_bounds_safe() {
    let diags = check_retry_policy_bounds(50, 60_000, 10.0, 3_600_000);
    assert!(diags.is_empty());
}

#[test]
fn test_max_attempts_warning() {
    let diags = check_retry_policy_bounds(51, 60_000, 10.0, 3_600_000);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code(), &LintCode::L003);
    assert_eq!(diags[0].severity(), Severity::Warning);
    assert_eq!(diags[0].field(), Some("max_attempts"));
}

#[test]
fn test_initial_delay_warning() {
    let diags = check_retry_policy_bounds(50, 60_001, 10.0, 3_600_000);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code(), &LintCode::L004);
    assert_eq!(diags[0].severity(), Severity::Warning);
    assert_eq!(diags[0].field(), Some("initial_delay"));
}

#[test]
fn test_backoff_multiplier_warning() {
    let diags = check_retry_policy_bounds(50, 60_000, 10.1, 3_600_000);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code(), &LintCode::L005);
    assert_eq!(diags[0].severity(), Severity::Warning);
    assert_eq!(diags[0].field(), Some("backoff_multiplier"));
}

#[test]
fn test_max_delay_error() {
    let diags = check_retry_policy_bounds(50, 60_000, 10.0, 3_600_001);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code(), &LintCode::L006);
    assert_eq!(diags[0].severity(), Severity::Error);
    assert_eq!(diags[0].field(), Some("max_delay"));
}

#[test]
fn test_multiple_violations() {
    let diags = check_retry_policy_bounds(100, 120_000, 20.0, 3_600_001);
    assert_eq!(diags.len(), 4);
}

#[test]
fn test_at_boundary_no_warning() {
    assert!(check_retry_policy_bounds(50, 60_000, 10.0, 3_600_000).is_empty());
}
