#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintError};
use crate::{l001_time, l003_direct_io, l004, l005, l006, rules};

/// Rule abstraction used by the visitor-based linter engine.
pub trait VisitRule {
    fn lint(&self, source: &str) -> Result<Vec<Diagnostic>, LintError>;
}

struct RuleFn {
    f: fn(&str) -> Result<Vec<Diagnostic>, LintError>,
}

impl VisitRule for RuleFn {
    fn lint(&self, source: &str) -> Result<Vec<Diagnostic>, LintError> {
        (self.f)(source)
    }
}

/// Composable linter that runs all configured rules against source text.
#[derive(Default)]
pub struct Linter {
    rules: Vec<Box<dyn VisitRule + Send + Sync>>,
}

impl Linter {
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule<R>(&mut self, rule: R)
    where
        R: VisitRule + Send + Sync + 'static,
    {
        self.rules.push(Box::new(rule));
    }

    pub fn add_rule_fn(&mut self, rule: fn(&str) -> Result<Vec<Diagnostic>, LintError>) {
        self.add_rule(RuleFn { f: rule });
    }

    pub fn lint_source(&self, source: &str) -> Result<Vec<Diagnostic>, LintError> {
        self.rules.iter().try_fold(Vec::new(), |acc, rule| {
            rule.lint(source).map(|diags| {
                let mut next = acc;
                next.extend(diags);
                next
            })
        })
    }

    pub fn lint_file(&self, path: &std::path::Path) -> Result<Vec<Diagnostic>, LintError> {
        std::fs::read_to_string(path)
            .map_err(|e| LintError::ParseError(e.to_string()))
            .and_then(|src| self.lint_source(&src))
    }
}

fn lint_l002(source: &str) -> Result<Vec<Diagnostic>, LintError> {
    syn::parse_file(source)
        .map_err(|e| LintError::ParseError(e.to_string()))
        .map(|file| rules::check_random_in_workflow(&file))
}

#[must_use]
pub fn linter_with_all_rules() -> Linter {
    let mut linter = Linter::new();
    linter.add_rule_fn(l001_time::lint_workflow_code);
    linter.add_rule_fn(lint_l002);
    linter.add_rule_fn(l003_direct_io::lint_workflow_code);
    linter.add_rule_fn(l004::lint_workflow_code);
    linter.add_rule_fn(l005::lint_workflow_code);
    linter.add_rule_fn(l006::lint_workflow_code);
    linter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_source_returns_parse_error_for_invalid_rust() {
        let linter = linter_with_all_rules();
        let result = linter.lint_source("not rust code @#$");
        assert!(matches!(result, Err(LintError::ParseError(_))));
    }

    #[test]
    fn lint_source_handles_empty_input() {
        let linter = linter_with_all_rules();
        let result = linter.lint_source("");
        assert!(result.is_ok());
    }
}
