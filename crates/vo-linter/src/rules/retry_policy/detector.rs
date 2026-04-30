#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode, Severity};

pub const MAX_ATTEMPTS_WARN: u8 = 50;
pub const INITIAL_DELAY_MS_WARN: u64 = 60_000;
pub const BACKOFF_MULTIPLIER_WARN: f64 = 10.0;
pub const MAX_DELAY_MS_ERROR: u64 = 3_600_000;

pub fn check_retry_policy_bounds(
    max_attempts: u8,
    initial_delay_ms: u64,
    backoff_multiplier: f64,
    max_delay_ms: u64,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if max_attempts > MAX_ATTEMPTS_WARN {
        diags.push(
            Diagnostic::new(LintCode::L003, "max_attempts exceeds safe bound")
                .with_severity(Severity::Warning)
                .with_field("max_attempts")
                .with_suggested_bound(format!("<= {}", MAX_ATTEMPTS_WARN)),
        );
    }

    if initial_delay_ms > INITIAL_DELAY_MS_WARN {
        diags.push(
            Diagnostic::new(LintCode::L004, "initial_delay exceeds safe bound")
                .with_severity(Severity::Warning)
                .with_field("initial_delay")
                .with_suggested_bound(format!("<= {}ms (60s)", INITIAL_DELAY_MS_WARN)),
        );
    }

    if backoff_multiplier > BACKOFF_MULTIPLIER_WARN {
        diags.push(
            Diagnostic::new(LintCode::L005, "backoff_multiplier exceeds safe bound")
                .with_severity(Severity::Warning)
                .with_field("backoff_multiplier")
                .with_suggested_bound(format!("<= {}", BACKOFF_MULTIPLIER_WARN)),
        );
    }

    if max_delay_ms > MAX_DELAY_MS_ERROR {
        diags.push(
            Diagnostic::new(LintCode::L006, "max_delay exceeds safe bound")
                .with_severity(Severity::Error)
                .with_field("max_delay")
                .with_suggested_bound(format!("<= {}ms (1 hour)", MAX_DELAY_MS_ERROR)),
        );
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_retry_policy_bounds_all_safe() {
        let diags = check_retry_policy_bounds(50, 60_000, 10.0, 3_600_000);
        assert!(diags.is_empty());
    }

    #[test]
    fn check_retry_policy_bounds_max_attempts_warning() {
        let diags = check_retry_policy_bounds(51, 60_000, 10.0, 3_600_000);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code(), &LintCode::L003);
        assert_eq!(diags[0].severity(), Severity::Warning);
        assert_eq!(diags[0].field(), Some("max_attempts"));
        assert_eq!(diags[0].suggested_bound(), Some("<= 50"));
    }

    #[test]
    fn check_retry_policy_bounds_initial_delay_warning() {
        let diags = check_retry_policy_bounds(50, 60_001, 10.0, 3_600_000);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code(), &LintCode::L004);
        assert_eq!(diags[0].severity(), Severity::Warning);
        assert_eq!(diags[0].field(), Some("initial_delay"));
        assert_eq!(diags[0].suggested_bound(), Some("<= 60000ms (60s)"));
    }

    #[test]
    fn check_retry_policy_bounds_backoff_multiplier_warning() {
        let diags = check_retry_policy_bounds(50, 60_000, 10.1, 3_600_000);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code(), &LintCode::L005);
        assert_eq!(diags[0].severity(), Severity::Warning);
        assert_eq!(diags[0].field(), Some("backoff_multiplier"));
        assert_eq!(diags[0].suggested_bound(), Some("<= 10"));
    }

    #[test]
    fn check_retry_policy_bounds_max_delay_error() {
        let diags = check_retry_policy_bounds(50, 60_000, 10.0, 3_600_001);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code(), &LintCode::L006);
        assert_eq!(diags[0].severity(), Severity::Error);
        assert_eq!(diags[0].field(), Some("max_delay"));
        assert_eq!(diags[0].suggested_bound(), Some("<= 3600000ms (1 hour)"));
    }

    #[test]
    fn check_retry_policy_bounds_multiple_warnings() {
        let diags = check_retry_policy_bounds(100, 120_000, 20.0, 3_600_001);
        assert_eq!(diags.len(), 4);
        assert!(diags
            .iter()
            .all(|d| d.severity() == Severity::Warning || d.code() == &LintCode::L006));
    }

    #[test]
    fn check_retry_policy_bounds_at_boundary() {
        let diags = check_retry_policy_bounds(50, 60_000, 10.0, 3_600_000);
        assert!(diags.is_empty());
    }

    #[test]
    fn check_retry_policy_bounds_just_over_max_attempts() {
        let diags = check_retry_policy_bounds(51, 60_000, 10.0, 3_600_000);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].field(), Some("max_attempts"));
    }

    #[test]
    fn check_retry_policy_bounds_just_over_initial_delay() {
        let diags = check_retry_policy_bounds(50, 60_001, 10.0, 3_600_000);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].field(), Some("initial_delay"));
    }

    #[test]
    fn check_retry_policy_bounds_just_over_backoff_multiplier() {
        let diags = check_retry_policy_bounds(50, 60_000, 10.01, 3_600_000);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].field(), Some("backoff_multiplier"));
    }

    #[test]
    fn check_retry_policy_bounds_just_over_max_delay() {
        let diags = check_retry_policy_bounds(50, 60_000, 10.0, 3_600_001);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].field(), Some("max_delay"));
        assert_eq!(diags[0].severity(), Severity::Error);
    }
}
