//! Traits for timer storage and work queue operations.

use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{ReanimatorError, TimerRecord};

/// Pending timer record for crash recovery tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTimer {
    pub instance_id: InstanceId,
    pub fire_at_ms: TimestampMs,
    pub scheduled_at_ms: TimestampMs,
    pub marked_at_ms: TimestampMs,
}

impl PendingTimer {
    pub fn new(
        instance_id: InstanceId,
        fire_at_ms: TimestampMs,
        scheduled_at_ms: TimestampMs,
    ) -> Self {
        Self {
            instance_id,
            fire_at_ms,
            scheduled_at_ms,
            marked_at_ms: TimestampMs::now(),
        }
    }
}

/// Trait for timer storage operations.
/// Abstracts the underlying storage implementation (e.g., fjall, rocksdb).
#[async_trait::async_trait]
pub trait TimerStorage: Send + Sync {
    /// Scans for timers that are due (fire_at_ms <= current_time).
    async fn scan_due_timers(
        &self,
        from_timestamp: TimestampMs,
        to_timestamp: TimestampMs,
        max_results: u32,
    ) -> Result<Vec<TimerRecord>, ReanimatorError>;

    /// Deletes a timer by its key.
    async fn delete_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError>;

    /// Records that a timer has fired (appends to events partition).
    async fn record_timer_fired(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError>;

    /// Marks a timer as in-flight (processing started but not completed).
    /// Used for crash recovery: if the reanimator crashes, pending timers can be replayed.
    async fn mark_timer_processing(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError>;

    /// Scans for timers that are pending (marked as in-flight but not completed).
    /// Used during crash recovery to find timers that need to be replayed.
    async fn scan_pending_timers(
        &self,
        max_results: u32,
    ) -> Result<Vec<PendingTimer>, ReanimatorError>;

    /// Completes a pending timer (marks it as no longer in-flight).
    async fn complete_timer_processing(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError>;

    /// Cleans up stale pending timers older than the given timestamp.
    /// Used during crash recovery to remove orphaned pending timers.
    async fn cleanup_stale_pending_timers(
        &self,
        older_than: TimestampMs,
    ) -> Result<u32, ReanimatorError>;
}

/// Trait for enqueuing resume work.
/// Abstracts the work queue implementation.
#[async_trait::async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a resume message for an instance.
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), ReanimatorError>;
}
