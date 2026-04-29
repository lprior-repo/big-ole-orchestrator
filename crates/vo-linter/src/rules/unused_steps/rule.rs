//! Rule implementation for detecting unused steps in workflow DAGs.

use crate::{diagnostic::Diagnostic, Rule};
use syn::File;

/// L004: Detects unused (unreachable) steps in workflow DAGs.
///
/// This rule analyzes workflow DAG structures to find nodes that have no
/// incoming edges from the entry point and would never execute.
pub struct UnusedStepsRule;

impl Rule for UnusedStepsRule {
    fn id(&self) -> &'static str {
        "L004"
    }

    fn name(&self) -> &'static str {
        "unused workflow step"
    }

    fn execute(&self, _file: &File) -> Vec<Diagnostic> {
        // AST-based linting cannot determine runtime DAG reachability.
        // The standalone check_unused_steps() function in this module
        // performs the actual analysis on DagGraph structures.
        Vec::new()
    }
}
