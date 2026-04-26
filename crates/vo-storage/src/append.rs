//! Append operations with per-write-class queue budgeting.
//!
//! This module provides the append path for storage writes, implementing
//! traffic isolation via bounded channels per write class.

mod appender;
mod backpressure;
mod budget;
mod entries;
mod latency;
mod metrics;
mod queue;
mod write_class;

#[cfg(test)]
mod tests;

pub use appender::Appender;
pub use backpressure::{BackpressureEvent, BackpressureSignal};
pub use budget::{BudgetError, WriteBudget};
pub use entries::{AppendEntry, BlobWrite, ControlPlaneWrite, ProjectionWrite};
pub use latency::CommitLatencyTracker;
pub use queue::{BudgetQueues, BudgetQueuesError, ClassifiedWrite, QueueConfig, QueueStats};
pub use write_class::WriteClass;
