//! Rule implementation for detecting unused steps in workflow DAGs.

use crate::{diagnostic::Diagnostic, Rule};
use syn::File;

pub struct UnusedStepsRule;

impl Rule for UnusedStepsRule {
    fn id(&self) -> &'static str {
        "L004"
    }

    fn name(&self) -> &'static str {
        "unused workflow step"
    }

    fn execute(&self, file: &File) -> Vec<Diagnostic> {
        super::check_unused_steps_ast(file)
    }
}
