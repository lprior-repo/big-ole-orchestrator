//! Red Queen adversarial tests for vo-storage append writer
//!
//! Tests the append writer invariants against:
//! - Concurrent appends from multiple threads (via Arc<Mutex> wrapping)
//! - Budget exhaustion and rollback atomicity
//! - Queue capacity limits and backpressure
//! - Priority ordering (ADR-016: CriticalControlPlane first)
//! - Thread safety under stress
//!
//! Target: vo-storage/append

#![allow(clippy::unwrap_used)]

pub mod helpers;
pub mod concurrent_append;
pub mod budget_exhaustion;
pub mod queue_capacity;
pub mod backpressure;
pub mod dequeue_ordering;
pub mod atomicity;
pub mod write_classification;
pub mod shared_backpressure;
pub mod stress_fuzz;
pub mod recovery_simulation;

pub use vo_storage::append::{
    AppendEntry, Appender, BackpressureSignal, BlobWrite, BudgetQueues, BudgetQueuesError,
    ClassifiedWrite, ControlPlaneWrite, ProjectionWrite, QueueConfig, WriteBudget, WriteClass,
};
