//! Timer supervisor traits
//!
//! Contains the WorkQueue trait definition.
//! TimerStorage is now provided by vo_common::ports.

use vo_types::InstanceId;

use super::types::TimerSupervisorError;

/// Work queue trait for dispatching work
pub trait WorkQueue: Send + Sync {
    /// Enqueues a resume work item for the given instance.
    ///
    /// # Errors
    /// Returns an error if the enqueue operation fails.
    fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), TimerSupervisorError>;
}
