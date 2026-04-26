//! L003: Placeholder test anti-pattern detection.
//!
//! This module detects tests that do not actually exercise production code.
//!
//! # Patterns Detected
//!
//! - **L003-A**: `assert!(true)` / `assert_eq!` with only literal arguments — trivially passing
//! - **L003-B**: `#[ignore]` on test functions — unexecuted tests
//! - **L003-C**: `todo!()` inside test functions — incomplete tests
//! - **L003-D**: Commented-out handler/test function signatures — ghost code
//!
//! # Architecture
//!
//! Uses `syn` AST visitor for structural detection and raw source scanning
//! for comment-based patterns. Rules are stateless and independent.

mod detector;
mod rule;

pub use detector::check_placeholder_tests;
pub use rule::PlaceholderRule;

#[cfg(test)]
mod tests;
