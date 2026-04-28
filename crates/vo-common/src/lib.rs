//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

pub mod connection_pool;
pub mod error;
pub mod events;
mod structures;
pub mod types;

pub use error::{ExecutionError, JobRunError, RetryError, SchedulerError, VoError};
pub use events::{DuplicateResult, EventDedup, WorkflowEvent};
pub use types::{EventId, InstanceId, NamespaceId, TimerId};
pub use structures::{Bounds, Octree, Vec3};

#[cfg(feature = "telemetry")]
pub mod telemetry;

#[cfg(feature = "telemetry")]
pub use telemetry::{
    Counter, Gauge, Histogram, OtlpEndpoint, TelemetryConfig,
    TelemetryExporter, TelemetryMetrics, TelemetryTracer,
};
