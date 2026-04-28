//! Shared work queue trait used by timer_supervisor, spawn_supervisor, and reanimator.
//!
//! Previously defined in four places:
//! - timer_supervisor/traits.rs (sync, enqueue_resume)
//! - spawn_supervisor/traits.rs (async, enqueue_spawn + enqueue_resume)
//! - reanimator/traits.rs (async, enqueue_resume + is_instance_terminal)
//! - timer_supervisor_tests.rs (sync, enqueue_resume)
//!
//! This unified async trait replaces all four copies.

use std::error::Error;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use vo_types::InstanceId;

/// Async work queue trait for dispatching supervisor work.
///
/// Used by timer_supervisor (enqueue_resume), spawn_supervisor (enqueue_spawn),
/// and reanimator (enqueue_resume + is_instance_terminal).
#[async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a spawn work item for the given instance.
    ///
    /// Used by spawn_supervisor to dispatch subprocess spawning.
    async fn enqueue_spawn(
        &self,
        instance_id: InstanceId,
        executable: PathBuf,
        args: Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Enqueues a resume work item for the given instance.
    ///
    /// Used by timer_supervisor, reanimator, and spawn_supervisor.
    async fn enqueue_resume(
        &self,
        instance_id: InstanceId,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Checks if an instance is in a terminal state (Completed, Failed, or Cancelled).
    ///
    /// Used by reanimator during crash recovery to skip timer replay for terminated instances.
    /// Returns `Ok(true)` if terminal, `Ok(false)` if still active, or an error if
    /// the check itself failed.
    async fn is_instance_terminal(&self, instance_id: &InstanceId) -> Result<bool, Box<dyn Error + Send + Sync>>;
}
