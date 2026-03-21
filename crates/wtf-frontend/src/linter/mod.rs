mod engine;
mod model;

#[cfg(test)]
mod tests;

pub use engine::SpecLinter;
pub use model::{CategoryScore, LintError, LintIssue, LintReport, LintRule, LintRules, Spec};
