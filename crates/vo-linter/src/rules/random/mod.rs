//! Linting rules for detecting non-deterministic random calls in workflow source code.
//!
//! Each function in this module implements a single lint rule identified by a
//! [`LintCode`] from the [`diagnostic`](crate::diagnostic) module.
//!
//! # Available Rules
//!
//! - [`check_random_in_workflow`] — **L002**: flags non-deterministic random calls

mod detector;

pub use detector::check_random_in_workflow;

#[cfg(test)]
mod tests;
