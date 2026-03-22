#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

// ADR-020: Procedural workflow static linter.
// Implemented as syn AST visitors over workflow function bodies.
// Rules WTF-L001 through WTF-L006 — see individual rule modules.

pub mod diagnostic;
pub mod engine;
pub mod l001_time;
pub mod l003_direct_io;
pub mod l004;
pub mod l005;
pub mod l006;
pub mod rules;
pub mod visitor;

pub use diagnostic::{Diagnostic, LintCode, LintError, Severity};
pub use engine::{linter_with_all_rules, Linter, VisitRule};
pub use l004::lint_workflow_code as lint_workflow_code_l004;
pub use l005::lint_workflow_code;
pub use l006::lint_workflow_code as lint_workflow_code_l006;
pub use rules::check_random_in_workflow;

pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
}

impl LintResult {
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let has_errors = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        Self {
            diagnostics,
            has_errors,
        }
    }
}

/// Lint workflow source code with all registered rules (L001-L006).
///
/// # Errors
/// Returns `LintError::ParseError` if the source cannot be parsed.
pub fn lint_workflow_source(source: &str) -> Result<LintResult, LintError> {
    linter_with_all_rules()
        .lint_source(source)
        .map(deduplicate_diagnostics)
        .map(LintResult::new)
}

fn deduplicate_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .fold(
            (std::collections::HashSet::new(), Vec::new()),
            |(mut seen, mut unique), diagnostic| {
                let key = (diagnostic.code.as_str().to_owned(), diagnostic.span);
                if seen.insert(key) {
                    unique.push(diagnostic);
                }
                (seen, unique)
            },
        )
        .1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_result_has_errors_false_for_warning_only() {
        let diagnostics = vec![Diagnostic::new(LintCode::L004, "warning")];
        let result = LintResult::new(diagnostics);
        assert!(!result.has_errors);
    }

    #[test]
    fn lint_result_has_errors_true_for_error_diagnostic() {
        let diagnostics = vec![Diagnostic::new(LintCode::L001, "error")];
        let result = LintResult::new(diagnostics);
        assert!(result.has_errors);
    }
}
