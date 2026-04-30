//! Timer storage trait and types.
//!
//! This module defines the unified `TimerStorage` trait used by `vo-actor`
//! for timer management operations.

use async_trait::async_trait;
use vo_types::{
    InstanceId as TypesInstanceId, TimerId as TypesTimerId, TimestampMs as TypesTimestampMs,
};

// Re-export types from vo_types for use by vo-actor
// These are the canonical types used across the vo-engine codebase
pub use vo_types::{InstanceId, TimerId};

/// Alias for `TimestampMs` from vo-types (millisecond-precision timestamp)
pub type TimestampMs = TypesTimestampMs;

// =============================================================================
// TimerRecord - Represents a scheduled timer
// =============================================================================

/// `TimerRecord` - Represents a scheduled timer in the storage system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerRecord {
    /// The instance this timer belongs to.
    pub instance_id: InstanceId,
    /// When the timer should fire (milliseconds since epoch).
    pub fire_at_ms: TimestampMs,
    /// Optional unique identifier for the timer.
    pub timer_id: Option<TimerId>,
    /// When the timer was scheduled (milliseconds since epoch).
    pub scheduled_at_ms: TimestampMs,
}

// =============================================================================
// TimerStorageError - Error types for timer storage operations
// =============================================================================

/// `TimerStorageError` - Error types for timer storage operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimerStorageError {
    /// The requested timer was not found.
    #[error("Timer not found")]
    NotFound,
    /// A storage-level error occurred.
    #[error("Storage error: {0}")]
    StorageError(String),
}

// =============================================================================
// TimerStorage - Async trait for timer storage operations
// =============================================================================

/// `TimerStorage` - Async trait for timer storage operations.
///
/// Implementors provide persistent storage for workflow timers.
#[async_trait]
pub trait TimerStorage: Send + Sync {
    /// Schedule a new timer.
    async fn schedule_timer(&self, record: TimerRecord) -> Result<(), TimerStorageError> {
        let _ = record;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Cancel a timer.
    async fn cancel_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), TimerStorageError> {
        let _ = instance_id;
        let _ = fire_at_ms;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Get a specific timer.
    async fn get_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<TimerRecord, TimerStorageError> {
        let _ = instance_id;
        let _ = fire_at_ms;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// List all timers for a given instance.
    async fn list_timers_for_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<Vec<TimerRecord>, TimerStorageError> {
        let _ = instance_id;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Scan for due timers in a time range.
    async fn scan_due_timers(
        &self,
        from_ms: TimestampMs,
        to_ms: TimestampMs,
        max: u32,
    ) -> Result<Vec<TimerRecord>, TimerStorageError> {
        let _ = from_ms;
        let _ = to_ms;
        let _ = max;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Delete a specific timer.
    async fn delete_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), TimerStorageError> {
        let _ = instance_id;
        let _ = fire_at_ms;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Get the total count of timers.
    async fn get_timer_count(&self) -> Result<u64, TimerStorageError> {
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    // -------------------------------------------------------------------------
    // Additional methods used by vo-actor supervisor
    // -------------------------------------------------------------------------

    /// List expired timers within a time range (used by TimerSupervisor).
    async fn list_expired_timers(
        &self,
        from: TimestampMs,
        to: TimestampMs,
        max: u32,
    ) -> Result<Vec<TimerRecord>, TimerStorageError> {
        let _ = from;
        let _ = to;
        let _ = max;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Retry a timer with a new fire time.
    async fn retry_timer(
        &self,
        timer: &TimerRecord,
        new_fire_at_ms: TimestampMs,
    ) -> Result<(), TimerStorageError> {
        let _ = timer;
        let _ = new_fire_at_ms;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }

    /// Delete all timers for a specific instance.
    async fn delete_all_timers_for_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<u32, TimerStorageError> {
        let _ = instance_id;
        Err(TimerStorageError::StorageError("not implemented".into()))
    }
}
