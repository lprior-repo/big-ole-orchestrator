//! Timer supervisor traits
//!
//! Contains the storage and work queue trait definitions.

use vo_types::InstanceId;

use super::types::{TimerRecord, TimerSupervisorError};

// =============================================================================
// Traits - Storage abstractions
// =============================================================================

/// Storage trait for timer operations
pub trait TimerStorage: Send + Sync {
    /// Scans for due timers in the given time range.
    fn scan_due_timers(&self, from: u64, to: u64, max: u32) -> Vec<TimerRecord>;

    /// Deletes a timer.
    ///
    /// # Errors
    /// Returns an error if the delete operation fails.
    fn delete_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: u64,
    ) -> Result<(), TimerSupervisorError>;

    /// Retries a timer by rescheduling it with a new fire_at time.
    ///
    /// Used when dispatch fails after successful delete to recover the timer.
    ///
    /// # Errors
    /// Returns an error if the retry operation fails.
    fn retry_timer(
        &self,
        timer: &TimerRecord,
        new_fire_at_ms: u64,
    ) -> Result<(), TimerSupervisorError>;
}
