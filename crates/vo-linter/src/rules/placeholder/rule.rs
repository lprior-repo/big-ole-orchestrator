//! Rule implementation for detecting placeholder test anti-patterns.

use crate::diagnostic::Diagnostic;
use syn::File;

/// L003: Detects placeholder anti-patterns in test code.
///
/// Detects: `assert!(true)`, `#[ignore]`, `todo!()`, commented-out handlers,
/// duplicate test-module types, and constant-only assertions.
pub struct PlaceholderRule;

impl PlaceholderRule {
    #[must_use]
    pub fn check_placeholder_tests(file: &File, source: &str) -> Vec<Diagnostic> {
        super::detector::check_placeholder_tests(file, source)
    }
}
