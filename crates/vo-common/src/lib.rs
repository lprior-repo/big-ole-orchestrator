//! Common utilities and types for vo-engine.
//!
//! Shared functionality used across multiple crates including
//! type aliases and common event definitions.

pub mod backoff;
pub mod connection_pool;
pub mod error;
pub mod events;
pub mod pool;
pub mod timer_storage;
mod structures;
pub mod types;
pub mod slot;
#[cfg(test)]
mod namespace_id;

pub use error::{ExecutionError, RetryError, SchedulerError, VoError};
pub use events::WorkflowEvent;
pub use slot::{SlotAllocError, SlotAllocator, SlotIdx, SlotValue, MAX_SLOTS};
pub use structures::{Bounds, Octree, Vec3};
pub use types::{InstanceId, NamespaceId, NamespaceIdParseError, TimerId};

#[cfg(feature = "telemetry")]
pub mod telemetry;

#[cfg(feature = "telemetry")]
pub use telemetry::{
    Counter, Gauge, Histogram, OtlpEndpoint, TelemetryConfig, TelemetryExporter, TelemetryMetrics,
    TelemetryTracer,
};