//! Rule implementation for detecting unused imports in workflow code.

use crate::{diagnostic::Diagnostic, Rule};
use syn::File;

/// L001: Detects unused import statements.
pub struct UnusedImportRule;

impl Rule for UnusedImportRule {
    fn id(&self) -> &'static str {
        "L001"
    }

    fn name(&self) -> &'static str {
        "unused import"
    }

    fn execute(&self, file: &File) -> Vec<Diagnostic> {
        super::check_unused_imports(file)
    }
}