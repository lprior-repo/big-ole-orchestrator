//! Linting rules for validating retry policy values in workflow definitions.
//!
//! Each function in this module implements a single lint rule identified by a
//! [`LintCode`] from the [`diagnostic`](crate::diagnostic) module.
//!
//! # Available Rules
//!
//! - [`check_retry_policy_bounds`] — **L003-L006**: flags retry policies with unsafe values

mod detector;
mod rule;

pub use detector::check_retry_policy_bounds;
pub use rule::RetryPolicyRule;

#[cfg(test)]
mod tests;
