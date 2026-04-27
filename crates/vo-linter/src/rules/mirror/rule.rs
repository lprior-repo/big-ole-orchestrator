//! Rule implementation for detecting mirror types in API tests.

use crate::{diagnostic::Diagnostic, rules::mirror::check_mirror_types_in_tests, Rule};
use syn::File;

/// L003: Detects mirror types in API tests (local duplicates of production handlers).
pub struct MirrorRule;

impl Rule for MirrorRule {
    fn id(&self) -> &'static str {
        "L003"
    }

    fn name(&self) -> &'static str {
        "mirror type in test (not production handler)"
    }

    fn execute(&self, file: &File) -> Vec<Diagnostic> {
        check_mirror_types_in_tests(file)
    }
}