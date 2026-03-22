#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

// ADR-020: Procedural workflow static linter.
// Implemented as syn AST visitors over workflow function bodies.
// Rules WTF-L001 through WTF-L006 — see individual rule modules.

pub mod diagnostic;
pub mod l005;
pub mod l006;
pub mod rules;
pub mod visitor;

pub use diagnostic::{Diagnostic, LintCode, Severity};
pub use l005::lint_workflow_code;
pub use l006::lint_workflow_code as lint_workflow_code_l006;
