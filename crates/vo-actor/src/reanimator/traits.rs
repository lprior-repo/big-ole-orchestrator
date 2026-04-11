//! Traits for timer storage and work queue operations.

use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{ReanimatorError, TimerRecord};

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
}

/// Trait for enqueuing resume work.
/// Abstracts the work queue implementation.
#[async_trait::async_trait]
pub trait WorkQueue: Send + Sync {
    /// Enqueues a resume message for an instance.
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), ReanimatorError>;
}
