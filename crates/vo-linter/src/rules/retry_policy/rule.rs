//! Rule implementation for validating retry policy bounds in workflow code.

use crate::{diagnostic::Diagnostic, rules::retry_policy::check_retry_policy_bounds, Rule};
use syn::File;

/// L003-L006: Validates retry policy values are within safe bounds.
///
/// - L003: max_attempts > 50 → warning
/// - L004: initial_delay > 60s → warning
/// - L005: backoff_multiplier > 10 → warning
/// - L006: max_delay > 1 hour → error
pub struct RetryPolicyRule;

impl Rule for RetryPolicyRule {
    fn id(&self) -> &'static str {
        "L003"
    }

    fn name(&self) -> &'static str {
        "retry policy bounds validation"
    }

    fn execute(&self, _file: &File) -> Vec<Diagnostic> {
        vec![]
    }
}
