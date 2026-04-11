//! Mock implementations for testing the Reanimator Loop.

use std::collections::VecDeque;
use tokio::sync::Mutex;
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    traits::{TimerStorage, WorkQueue},
    ReanimatorError, TimerRecord,
};

/// A mock timer storage that stores timers in memory.
#[derive(Debug)]
pub struct MockTimerStorage {
    timers: Mutex<VecDeque<TimerRecord>>,
    fire_calls: Mutex<Vec<(InstanceId, TimestampMs)>>,
    delete_calls: Mutex<Vec<(InstanceId, TimestampMs)>>,
    should_fail: Mutex<bool>,
}

impl MockTimerStorage {
    /// Creates a new MockTimerStorage with the given initial timers.
    pub fn new(timers: Vec<TimerRecord>) -> Self {
        Self {
            timers: Mutex::new(timers.into()),
            fire_calls: Mutex::new(Vec::new()),
            delete_calls: Mutex::new(Vec::new()),
            should_fail: Mutex::new(false),
        }
    }

    /// Sets whether operations should fail.
    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().await = should_fail;
    }

    /// Gets the recorded fire calls.
    pub async fn fire_calls(&self) -> Vec<(InstanceId, TimestampMs)> {
        self.fire_calls.lock().await.clone()
    }

    /// Gets the recorded delete calls.
    pub async fn delete_calls(&self) -> Vec<(InstanceId, TimestampMs)> {
        self.delete_calls.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl TimerStorage for MockTimerStorage {
    async fn scan_due_timers(
        &self,
        _from_timestamp: TimestampMs,
        to_timestamp: TimestampMs,
        max_results: u32,
    ) -> Result<Vec<TimerRecord>, ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        let timers = self.timers.lock().await;
        let due: Vec<TimerRecord> = timers
            .iter()
            .filter(|t| t.fire_at_ms <= to_timestamp)
            .take(max_results as usize)
            .cloned()
            .collect();

        Ok(due)
    }

    async fn delete_timer(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        self.delete_calls
            .lock()
            .await
            .push((instance_id.clone(), fire_at_ms));

        let mut timers = self.timers.lock().await;
        timers.retain(|t| !(t.instance_id == *instance_id && t.fire_at_ms == fire_at_ms));

        Ok(())
    }

    async fn record_timer_fired(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        self.fire_calls
            .lock()
            .await
            .push((instance_id.clone(), fire_at_ms));

        Ok(())
    }
}

/// Mock work queue for testing.
#[derive(Debug)]
pub struct MockWorkQueue {
    enqueued: Mutex<Vec<InstanceId>>,
    should_fail: Mutex<bool>,
}

impl Default for MockWorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWorkQueue {
    /// Creates a new MockWorkQueue.
    pub fn new() -> Self {
        Self {
            enqueued: Mutex::new(Vec::new()),
            should_fail: Mutex::new(false),
        }
    }

    /// Sets whether operations should fail.
    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().await = should_fail;
    }

    /// Gets the enqueued instance IDs.
    pub async fn enqueued(&self) -> Vec<InstanceId> {
        self.enqueued.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl WorkQueue for MockWorkQueue {
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::EnqueueFailed("Mock failure".to_string()));
        }
        self.enqueued.lock().await.push(instance_id);
        Ok(())
    }
}
