//! Linting rules for detecting local mirror types in API tests.
//!
//! Each function in this module implements a single lint rule identified by a
//! [`LintCode`] from the [`diagnostic`](crate::diagnostic) module.
//!
//! # Available Rules
//!
//! - [`check_mirror_types_in_api_test`] — **L003**: flags API tests that use local mirror
//!   types instead of production handlers

mod detector;
mod rule;

pub use detector::check_mirror_types_in_api_test;
pub use rule::MirrorRule;

#[cfg(test)]
mod tests;
