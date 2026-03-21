#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

// ADR-020: Procedural workflow static linter.
// Implemented as syn AST visitors over workflow function bodies.
// Rules WTF-L001 through WTF-L006 — see individual rule modules.

pub mod diagnostic;
pub mod rules;
pub mod visitor;

pub use diagnostic::{Diagnostic, LintCode, Severity};
