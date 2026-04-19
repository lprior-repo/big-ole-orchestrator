//! Workload class taxonomy for execution fairness per ADR-033.
//!
//! Defines the four workload classes that govern resume scheduling and
//! execution permit allocation:
//! - `ExactCritical` — never starved, highest dispatch priority
//! - `Standard` — normal workflow execution
//! - `UnsafeBulk` — lower priority, capped under contention
//! - `Recovery` — reserved capacity for crash recovery
//!
//! Also provides `WorkloadBudget` for per-class permit tracking and
//! `RejectionDetail` for load-shedding transparency.

pub mod budget;
pub mod proptest;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use budget::WorkloadBudget;
pub use types::{RejectionDetail, RejectionReason, WorkloadClass, WorkloadClassError};
