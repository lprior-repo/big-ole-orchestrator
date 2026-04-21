//! Rule implementation for detecting random calls in workflow code.

use crate::{diagnostic::Diagnostic, Rule};
use syn::File;

/// L002: Detects non-deterministic random calls (Uuid::new_v4, rand::random).
pub struct RandomRule;

impl Rule for RandomRule {
    fn id(&self) -> &'static str {
        "L002"
    }

    fn name(&self) -> &'static str {
        "non-deterministic random call"
    }

    fn execute(&self, file: &File) -> Vec<Diagnostic> {
        super::check_random_in_workflow(file)
    }
}
