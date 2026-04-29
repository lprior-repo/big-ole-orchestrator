//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

pub mod connection_pool;
pub mod error;
pub mod events;
pub mod pool;
mod structures;
pub mod types;

pub use error::{ExecutionError, RetryExhaustedError, SchedulerError, VoError};
pub use events::WorkflowEvent;
pub use structures::{Bounds, Octree, Vec3};
pub use types::{InstanceId, NamespaceId, TimerId};

#[cfg(feature = "telemetry")]
pub mod telemetry;

#[cfg(feature = "telemetry")]
pub use telemetry::{
    Counter, Gauge, Histogram, OtlpEndpoint, TelemetryConfig, TelemetryExporter, TelemetryMetrics,
    TelemetryTracer,
};
