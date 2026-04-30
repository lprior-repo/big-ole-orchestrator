//! Linting rule for detecting cyclic dependencies in workflow DAGs.
//!
//! This rule identifies cycles in workflow DAGs where a node depends on itself
//! either directly or transitively through a chain of dependencies.
//!
//! # Available Rules
//!
//! - [`CycleDetector::check`] — returns `Ok(())` for acyclic graphs, `Err(CycleError)` for cyclic
//! - [`check_cycles`] — **L008**: returns diagnostics for cyclic dependencies

mod detector;

pub use detector::{check_cycles, CycleDetector, CycleError};
pub use crate::rules::unused_steps::{DagGraph, Edge, Step};