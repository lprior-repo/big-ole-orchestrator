//! Static analysis and linting tools for vo-engine.
//!
//! Provides linting functionality for workflow definitions and
//! Rust source code analysis.
//!
//! # Crate Overview
//!
//! This crate provides static analysis tools for the Veloxide workflow engine,
//! including linting rules for workflow definitions and Rust source code.
//!
//! # Modules
//!
//! - [`rules`] - Collection of linting rules for workflow validation
//! - [`Diagnostic`] and [`LintCode`] - Diagnostic types and lint codes for reporting issues
//!
//! # Rules
//!
//! The linting rules cover:
//! - Workflow structure validation
//! - Step dependency checking
//! - Signal and handler compatibility
//! - Resource quota compliance
//! - Encryption and security checks

mod diagnostic;
pub mod rules;

pub use diagnostic::{Diagnostic, LintCode};
pub use rules::Rule;
