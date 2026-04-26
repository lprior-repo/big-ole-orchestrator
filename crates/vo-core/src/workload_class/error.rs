//! Errors for workload class operations.

use thiserror::Error;

use super::class::WorkloadClass;

/// Errors for workload class operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkloadClassError {
    /// Returned when an unknown workload class string is parsed.
    #[error("unknown workload class: {0}")]
    UnknownClass(String),

    /// Returned when a workload budget constraint is violated.
    #[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
    BudgetExceeded {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
}
