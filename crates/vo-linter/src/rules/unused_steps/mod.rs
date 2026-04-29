//! Linting rule for detecting unused (unreachable) steps in workflow DAGs.
//!
//! This rule identifies steps that have no incoming edges from the entry point
//! and would never execute during workflow runs.
//!
//! # Available Rules
//!
//! - [`check_unused_steps`] — **L004**: flags orphaned DAG nodes that are unreachable

mod detector;
mod graph;
mod rule;

pub use detector::check_unused_steps;
pub use graph::{DagGraph, Edge, Step};
pub use rule::UnusedStepsRule;

#[cfg(test)]
mod tests;
