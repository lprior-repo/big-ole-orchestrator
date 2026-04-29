//! Async spawn supervisor for subprocess lifecycle management.
//!
//! Manages subprocess lifecycle: spawn -> health-check -> ready -> running -> shutdown.
//!
//! # Structure
//!
//! - [`types`] - Type definitions (SpawnRecord, SpawnPhase, SpawnSupervisorError, SpawnSupervisorState, CycleResult)
//! - [`metrics`] - Metrics (Counter, SpawnSupervisorMetrics)
//! - [`traits`] - Trait definitions (SpawnStorage, ProcessManager)
//! - [`actor`] - SpawnSupervisor struct, constructor, spawn, and run_loop
//! - [`cycle`] - process_cycle implementation (Phases 1-3)
//! - [`health`] - Health check probing
//! - [`pure`] - Pure calculation functions (no side effects)
//!
//! WorkQueue is shared in [crate::work_queue].

pub mod actor;
pub mod cycle;
pub mod health;
pub mod metrics;
pub mod process;
pub mod pure;
pub mod traits;
pub mod types;

#[cfg(test)]
mod tests;

/// Type alias for cleaner imports in submodules.
pub(crate) use actor::SpawnSupervisor as Actor;

// Re-export commonly used types
pub use actor::{SpawnSupervisor, SpawnSupervisorHandle};
pub use metrics::{Counter, SpawnSupervisorMetrics};
pub use process::ProcessHandle;
pub use pure::{calculate_backoff_delay, is_zombie_state, should_respawn};
pub use traits::{ProcessManager, SpawnStorage, WorkQueue};
pub use types::{CycleResult, SpawnPhase, SpawnRecord, SpawnSupervisorError, SpawnSupervisorState};
