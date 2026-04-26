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

mod budget;
mod class;
mod degraded;
mod error;

pub use budget::{RejectionDetail, RejectionReason, WorkloadBudget};
pub use class::WorkloadClass;
pub use degraded::DegradedBudget;
pub use error::WorkloadClassError;
