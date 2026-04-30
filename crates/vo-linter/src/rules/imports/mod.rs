//! Linting rules for detecting unused import statements in workflow source code.
//!
//! Each function in this module implements a single lint rule identified by a
//! [`LintCode`] from the [`diagnostic`](crate::diagnostic) module.
//!
//! # Available Rules
//!
//! - [`check_unused_imports`] — **L001**: flags unused import statements

mod detector;
mod rule;

pub use detector::check_unused_imports;
pub use rule::UnusedImportRule;

#[cfg(test)]
mod tests;