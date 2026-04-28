//! Ghost workflow lifecycle: Active → Deactivated → Deleted (ADR-021).
//!
//! When a file watcher detects binary deletion, the workflow transitions to
//! `Deactivated`. In-flight instances continue against the pinned version.
//! A background reaper sweeps Deactivated workflows with zero running
//! instances and transitions them to `Deleted` (terminal).

mod error;
mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
mod reaper;
mod registration;
mod store;
mod watcher;

pub use error::GhostWorkflowError;
pub use lifecycle::GhostLifecycle;
pub use reaper::{spawn_reaper, ReaperConfig};
pub use registration::WorkflowRegistration;
pub use store::GhostLifecycleStore;
pub use watcher::{GhostWorkflowWatcher, GhostWorkflowWatcherError};
