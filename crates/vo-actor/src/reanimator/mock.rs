//! Mock implementations for testing the Reanimator Loop.

use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::Mutex;
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    traits::{PendingTimer, TimerStorage},
    ReanimatorError, TimerRecord,
};
use crate::work_queue::WorkQueue;

/// A mock timer storage that stores timers in memory.
#[derive(Debug)]
pub struct MockTimerStorage {
    timers: Mutex<VecDeque<TimerRecord>>,
    pending_timers: Mutex<HashMap<InstanceId, PendingTimer>>,
    /// Tracks (instance_id, fire_at_ms, timer_id) tuples that have been fired
    fire_calls: Mutex<Vec<(InstanceId, TimestampMs, Option<vo_types::TimerId>)>>,
    /// Tracks (instance_id, fire_at_ms) tuples that have been deleted
    delete_calls: Mutex<Vec<(InstanceId, TimestampMs)>>,
    /// Tracks instances that had all timers deleted
    delete_all_calls: Mutex<Vec<InstanceId>>,
    should_fail: Mutex<bool>,
    /// Tracks timers that have been deleted but not yet fired
    /// Key: (instance_id, fire_at_ms, timer_id)
    deleted_timers: Mutex<HashSet<(InstanceId, TimestampMs, Option<vo_types::TimerId>)>>,
}

impl MockTimerStorage {
    /// Creates a new MockTimerStorage with the given initial timers.
    pub fn new(timers: Vec<TimerRecord>) -> Self {
        Self {
            timers: Mutex::new(timers.into()),
            pending_timers: Mutex::new(HashMap::new()),
            fire_calls: Mutex::new(Vec::new()),
            delete_calls: Mutex::new(Vec::new()),
            delete_all_calls: Mutex::new(Vec::new()),
            should_fail: Mutex::new(false),
            deleted_timers: Mutex::new(HashSet::new()),
        }
    }

    /// Creates a new empty MockTimerStorage.
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Adds a timer to the storage.
    pub async fn add_timer(&self, timer: TimerRecord) {
        self.timers.lock().await.push_back(timer);
    }

    /// Sets whether operations should fail.
    pub async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.lock().await = should_fail;
    }

    /// Gets the recorded fire calls (backward compatible format).
    /// This returns (instance_id, fire_at_ms) tuples for compatibility with existing tests.
    pub async fn fire_calls(&self) -> Vec<(InstanceId, TimestampMs)> {
        self.fire_calls
            .lock()
            .await
            .iter()
            .map(|(i, f, _)| (i.clone(), *f))
            .collect()
    }

    /// Gets the recorded fire calls with full detail including timer_id.
    pub async fn fire_calls_full(
        &self,
    ) -> Vec<(InstanceId, TimestampMs, Option<vo_types::TimerId>)> {
        self.fire_calls.lock().await.clone()
    }

    /// Gets the recorded delete calls.
    pub async fn delete_calls(&self) -> Vec<(InstanceId, TimestampMs)> {
        self.delete_calls.lock().await.clone()
    }

    /// Gets the recorded delete_all calls.
    pub async fn delete_all_calls(&self) -> Vec<InstanceId> {
        self.delete_all_calls.lock().await.clone()
    }

    /// Adds a pending timer directly (for testing purposes).
    pub async fn add_pending_timer(&self, pending: PendingTimer) {
        self.pending_timers
            .lock()
            .await
            .insert(pending.instance_id.clone(), pending);
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
        let mut seen = HashSet::new();
        let mut due: Vec<TimerRecord> = Vec::new();

        for t in timers.iter() {
            if t.fire_at_ms <= to_timestamp {
                let key = (t.instance_id.clone(), t.fire_at_ms, t.timer_id.clone());
                if seen.insert(key) && due.len() < max_results as usize {
                    due.push(t.clone());
                }
            }
        }

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
        if let Some(pos) = timers
            .iter()
            .position(|t| t.instance_id == *instance_id && t.fire_at_ms == fire_at_ms)
        {
            if let Some(removed_timer) = timers.remove(pos) {
                // Track the deleted timer with its timer_id for proper deduplication
                self.deleted_timers.lock().await.insert((
                    removed_timer.instance_id.clone(),
                    removed_timer.fire_at_ms,
                    removed_timer.timer_id.clone(),
                ));
            }
        }

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

        // Deduplicate by checking if we've already recorded this specific timer as fired.
        // We need to find which timer_id corresponds to this (instance_id, fire_at_ms)
        // that has been deleted but not yet marked as fired.
        let mut fire_calls = self.fire_calls.lock().await;
        let mut deleted_timers = self.deleted_timers.lock().await;

        // Find all deleted timers with this (instance_id, fire_at_ms)
        let candidates: Vec<_> = deleted_timers
            .iter()
            .filter(|(del_instance_id, del_fire_at, _)| {
                *del_instance_id == *instance_id && *del_fire_at == fire_at_ms
            })
            .cloned()
            .collect();

        // If no deleted timer was found, this is a direct record_timer_fired call
        // Track it with timer_id = None for backward compatibility
        if candidates.is_empty() {
            let key = (instance_id.clone(), fire_at_ms, None);
            if !fire_calls
                .iter()
                .any(|(fi, ff, fti)| fi == &key.0 && ff == &key.1 && fti == &key.2)
            {
                fire_calls.push(key);
            }
        } else {
            // Find the first candidate that hasn't been fired yet
            for (del_instance_id, del_fire_at, timer_id) in candidates {
                // Check if this specific (instance_id, fire_at_ms, timer_id) has been fired
                if !fire_calls.iter().any(|(fi, ff, fti)| {
                    fi == &del_instance_id && ff == &del_fire_at && fti == &timer_id
                }) {
                    // This timer hasn't been fired yet, so record it
                    fire_calls.push((del_instance_id.clone(), del_fire_at, timer_id.clone()));
                    // Remove from deleted_timers since it's now been fired
                    deleted_timers.remove(&(del_instance_id, del_fire_at, timer_id));
                    // Only fire one timer per (instance_id, fire_at_ms) call
                    break;
                }
            }
        }

        Ok(())
    }

    async fn mark_timer_processing(
        &self,
        instance_id: &InstanceId,
        fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        let pending = PendingTimer::new(instance_id.clone(), fire_at_ms, TimestampMs::now());
        self.pending_timers
            .lock()
            .await
            .insert(instance_id.clone(), pending);

        Ok(())
    }

    async fn scan_pending_timers(
        &self,
        max_results: u32,
    ) -> Result<Vec<PendingTimer>, ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        let pending: Vec<PendingTimer> = self
            .pending_timers
            .lock()
            .await
            .values()
            .take(max_results as usize)
            .cloned()
            .collect();

        Ok(pending)
    }

    async fn complete_timer_processing(
        &self,
        instance_id: &InstanceId,
        _fire_at_ms: TimestampMs,
    ) -> Result<(), ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        self.pending_timers.lock().await.remove(instance_id);

        Ok(())
    }

    async fn cleanup_stale_pending_timers(
        &self,
        older_than: TimestampMs,
    ) -> Result<u32, ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        let mut pending = self.pending_timers.lock().await;
        let before = pending.len();
        pending.retain(|_, v| v.marked_at_ms > older_than);
        let after = pending.len();

        Ok((before - after) as u32)
    }

    async fn delete_all_timers_for_instance(
        &self,
        instance_id: &InstanceId,
    ) -> Result<u32, ReanimatorError> {
        if *self.should_fail.lock().await {
            return Err(ReanimatorError::StorageError("Mock failure".to_string()));
        }

        self.delete_all_calls.lock().await.push(instance_id.clone());

        let mut timers = self.timers.lock().await;
        let before = timers.len();
        timers.retain(|t| t.instance_id != *instance_id);
        let after = timers.len();

        Ok((before - after) as u32)
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
    async fn enqueue_spawn(
        &self,
        _instance_id: InstanceId,
        _executable: std::path::PathBuf,
        _args: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    async fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if *self.should_fail.lock().await {
            return Err(Box::new(ReanimatorError::EnqueueFailed("Mock failure".to_string())));
        }
        self.enqueued.lock().await.push(instance_id);
        Ok(())
    }

    async fn is_instance_terminal(
        &self,
        _instance_id: &InstanceId,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(false)
    }
}
