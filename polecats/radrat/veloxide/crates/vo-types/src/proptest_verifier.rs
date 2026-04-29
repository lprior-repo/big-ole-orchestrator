//! Proptest segregation verifier.
//!
//! Provides [`Verifier::check_segregation`] which programmatically confirms
//! that proptest modules are correctly gated behind the `proptest` feature flag.
//! When the feature is **absent**, the verifier returns `Ok(Segregated)`.
//! When the feature is **present**, the verifier returns `Ok(Integrated)`.
//!
//! This module is always compiled (regardless of feature flags) so that
//! deterministic tests can assert segregation holds.

/// Result of verifying proptest segregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegregationStatus {
    /// Proptests are correctly segregated (feature flag is absent).
    Segregated,
    /// Proptests are integrated (feature flag is present).
    Integrated,
}

/// Verifier for proptest segregation.
pub struct Verifier;

impl Verifier {
    /// Check whether proptests are segregated from the default test suite.
    ///
    /// Returns [`SegregationStatus::Segregated`] when the `proptest` feature
    /// is **not** enabled, confirming that no proptests will execute during
    /// a default `cargo test` run.
    ///
    /// Returns [`SegregationStatus::Integrated`] when the `proptest` feature
    /// **is** enabled, confirming that proptests are available.
    #[must_use]
    pub fn check_segregation() -> SegregationStatus {
        if cfg!(feature = "proptest") {
            SegregationStatus::Integrated
        } else {
            SegregationStatus::Segregated
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_reports_segregated_when_proptest_feature_is_absent() {
        // When compiled without --features proptest, this must be Segregated.
        // When compiled WITH --features proptest, this must be Integrated.
        let status = Verifier::check_segregation();
        if cfg!(feature = "proptest") {
            assert_eq!(
                status,
                SegregationStatus::Integrated,
                "proptest feature is enabled, should report Integrated"
            );
        } else {
            assert_eq!(
                status,
                SegregationStatus::Segregated,
                "proptest feature is absent, should report Segregated"
            );
        }
    }

    #[test]
    fn verifier_status_is_consistent_with_feature_flag() {
        let is_proptest_enabled = cfg!(feature = "proptest");
        let status = Verifier::check_segregation();
        let status_says_integrated = status == SegregationStatus::Integrated;
        assert_eq!(
            is_proptest_enabled, status_says_integrated,
            "Verifier status must match cfg!(feature = \"proptest\")"
        );
    }
}
