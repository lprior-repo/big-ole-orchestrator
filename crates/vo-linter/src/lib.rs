//! Static analysis and linting tools for vo-engine.
//!
//! Provides AST-based linting of Rust source code via [`syn`] to detect
//! patterns that violate Veloxide's deterministic execution guarantees.
//!
//! # Modules
//!
//! - [`rules`] - Collection of linting rules for workflow validation
//! - [`Diagnostic`] and [`LintCode`] - Diagnostic types and lint codes for reporting issues
//!
//! # Lint Code Registry
//!
//! | Code | Category | Description |
//! |------|----------|-------------|
//! | L002 | Determinism | Non-deterministic random call in workflow function |
//!
//! ## L002 — Non-deterministic Random Call
//!
//! Detects calls to `Uuid::new_v4()` and `rand::random()` inside workflow
//! functions. Workflows must be deterministic; use `ctx.random_u64()` (or
//! `ctx.random_u32()` / `ctx.random_u128()`) from the execution context
//! instead.

mod diagnostic;
pub mod rules;

pub use diagnostic::{Diagnostic, LintCode};
