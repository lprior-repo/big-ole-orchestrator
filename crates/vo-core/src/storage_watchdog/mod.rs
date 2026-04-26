//! Storage watchdog — background tokio task that monitors storage health and triggers degraded mode.
//!
//! Per ADR-013 §2, the watchdog monitors:
//! - filesystem free space,
//! - DbWriterActor commit latency,
//! - writer queue depth,
//! - flush timeout frequency,
//! - storage stall or compaction-backlog indicators.
//!
//! When critical thresholds are crossed, the engine enters Degraded Mode.

pub mod monitor;
pub mod types;
pub mod watchdog;

pub use monitor::StorageMonitor;
pub use types::{FlushTimeoutConfig, StorageHealth, StorageMetrics, StorageWatchdogConfig};
pub use watchdog::StorageWatchdog;
