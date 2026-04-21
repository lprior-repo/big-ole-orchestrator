//! Linting rules for detecting non-deterministic random calls in workflow source code.
//!
//! Each function in this module implements a single lint rule identified by a
//! [`LintCode`] from the [`diagnostic`](crate::diagnostic) module.
//!
//! # Available Rules
//!
//! - [`check_random_in_workflow`] — **L002**: flags non-deterministic random calls

mod detector;
mod rule;

pub use detector::check_random_in_workflow;
pub use rule::RandomRule;

#[cfg(test)]
mod tests;
