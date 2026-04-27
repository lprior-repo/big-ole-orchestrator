//! Append operations with per-write-class queue budgeting.
//!
//! This module provides the append path for storage writes, implementing
//! traffic isolation via bounded channels per write class.

use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vo_types::events::EventEnvelope;

/// Emit a rejection metric for monitoring.
fn emit_rejection(class: WriteClass, reason: &str) {
    let label = match class {
        WriteClass::CriticalControlPlane => "critical_control_plane",
        WriteClass::OperatorProjection => "operator_projection",
        WriteClass::BulkBlob => "bulk_blob",
    };
    metrics::counter!("vo_storage.write_rejected_total", "class" => label, "reason" => reason.to_string())
        .increment(1);
}

/// Emit a queue depth metric for monitoring.
fn emit_queue_depth(class: WriteClass, depth: usize) {
    let label = match class {
        WriteClass::CriticalControlPlane => "critical_control_plane",
        WriteClass::OperatorProjection => "projection",
        WriteClass::BulkBlob => "bulk_blob",
    };
    metrics::gauge!("vo_storage.queue_depth", "class" => label).set(depth as f64);
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteClass
// ─────────────────────────────────────────────────────────────────────────────

/// Defines the three-tier write class taxonomy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteClass {
    CriticalControlPlane,
    OperatorProjection,
    BulkBlob,
}

impl WriteClass {
    /// Returns the `QoS` tier number (1=critical, 2=projection, 3=blob).
    #[must_use]
    pub const fn tier(self) -> u8 {
        match self {
            Self::CriticalControlPlane => 1,
            Self::OperatorProjection => 2,
            Self::BulkBlob => 3,
        }
    }

    /// Returns `true` if writes of this class are never dropped under pressure.
    #[must_use]
    pub const fn never_drops(self) -> bool {
        matches!(self, Self::CriticalControlPlane)
    }
}

impl std::str::FromStr for WriteClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "critical_control_plane" => Ok(Self::CriticalControlPlane),
            "operator_projection" => Ok(Self::OperatorProjection),
            "bulk_blob" => Ok(Self::BulkBlob),
            _ => Err(format!("unknown write class: {s}")),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WriteBudget
// ─────────────────────────────────────────────────────────────────────────────

/// Associates a write budget per class for storage pressure management.
#[derive(Clone, Debug)]
pub struct WriteBudget {
    critical_limit: u64,
    projection_limit: u64,
    blob_limit: u64,
    critical_used: Cell<u64>,
    projection_used: Cell<u64>,
    blob_used: Cell<u64>,
}

impl WriteBudget {
    /// Creates a new budget with the given limits per class.
    #[must_use]
    pub const fn new(critical_limit: u64, projection_limit: u64, blob_limit: u64) -> Self {
        Self {
            critical_limit,
            projection_limit,
            blob_limit,
            critical_used: Cell::new(0),
            projection_used: Cell::new(0),
            blob_used: Cell::new(0),
        }
    }

    /// Returns the remaining budget for a given class.
    #[must_use]
    pub fn remaining(&self, class: WriteClass) -> u64 {
        match class {
            WriteClass::CriticalControlPlane => self
                .critical_limit
                .saturating_sub(self.critical_used.get()),
            WriteClass::OperatorProjection => self
                .projection_limit
                .saturating_sub(self.projection_used.get()),
            WriteClass::BulkBlob => self.blob_limit.saturating_sub(self.blob_used.get()),
        }
    }

    /// Checks if a write of the given class would exceed available budget.
    #[must_use]
    pub fn can_write(&self, class: WriteClass, size_bytes: u64) -> bool {
        self.remaining(class) >= size_bytes
    }

    /// Reserves budget for a write.
    ///
    /// # Errors
    /// Returns `BudgetError` if the write would exceed available budget.
    pub fn reserve(&self, class: WriteClass, size_bytes: u64) -> Result<(), BudgetError> {
        let remaining = self.remaining(class);
        if remaining < size_bytes {
            return Err(BudgetError {
                class,
                requested: size_bytes,
                available: remaining,
            });
        }
        match class {
            WriteClass::CriticalControlPlane => {
                self.critical_used.update(|v| v + size_bytes);
            }
            WriteClass::OperatorProjection => {
                self.projection_used.update(|v| v + size_bytes);
            }
            WriteClass::BulkBlob => {
                self.blob_used.update(|v| v + size_bytes);
            }
        }
        Ok(())
    }

    pub fn release(&self, class: WriteClass, size_bytes: u64) {
        match class {
            WriteClass::CriticalControlPlane => {
                self.critical_used
                    .set(self.critical_used.get().saturating_sub(size_bytes));
            }
            WriteClass::OperatorProjection => {
                self.projection_used
                    .set(self.projection_used.get().saturating_sub(size_bytes));
            }
            WriteClass::BulkBlob => {
                self.blob_used
                    .set(self.blob_used.get().saturating_sub(size_bytes));
            }
        }
    }
}

/// Budget exceeded error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
pub struct BudgetError {
    pub class: WriteClass,
    pub requested: u64,
    pub available: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// QueueConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for per-class queue capacities.
#[derive(Clone, Debug)]
pub struct QueueConfig {
    pub critical_capacity: usize,
    pub projection_capacity: usize,
    pub blob_capacity: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            critical_capacity: 1024,
            projection_capacity: 512,
            blob_capacity: 256,
        }
    }
}

impl QueueConfig {
    #[must_use]
    pub const fn capacity_for(&self, class: WriteClass) -> usize {
        match class {
            WriteClass::CriticalControlPlane => self.critical_capacity,
            WriteClass::OperatorProjection => self.projection_capacity,
            WriteClass::BulkBlob => self.blob_capacity,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// QueueStats
// ─────────────────────────────────────────────────────────────────────────────

/// Statistics about queue depth and capacity for monitoring.
#[derive(Clone, Debug)]
pub struct QueueStats {
    critical_depth: usize,
    projection_depth: usize,
    blob_depth: usize,
    config: QueueConfig,
}

impl QueueStats {
    #[must_use]
    pub const fn depth(&self, class: WriteClass) -> usize {
        match class {
            WriteClass::CriticalControlPlane => self.critical_depth,
            WriteClass::OperatorProjection => self.projection_depth,
            WriteClass::BulkBlob => self.blob_depth,
        }
    }

    #[must_use]
    pub const fn capacity(&self, class: WriteClass) -> usize {
        self.config.capacity_for(class)
    }

    #[must_use]
    pub const fn remaining(&self, class: WriteClass) -> usize {
        self.capacity(class).saturating_sub(self.depth(class))
    }

    #[must_use]
    pub const fn is_full(&self, class: WriteClass) -> bool {
        self.depth(class) >= self.capacity(class)
    }

    const fn increment(&mut self, class: WriteClass) {
        match class {
            WriteClass::CriticalControlPlane => self.critical_depth += 1,
            WriteClass::OperatorProjection => self.projection_depth += 1,
            WriteClass::BulkBlob => self.blob_depth += 1,
        }
    }

    const fn decrement(&mut self, class: WriteClass) {
        match class {
            WriteClass::CriticalControlPlane => {
                self.critical_depth = self.critical_depth.saturating_sub(1);
            }
            WriteClass::OperatorProjection => {
                self.projection_depth = self.projection_depth.saturating_sub(1);
            }
            WriteClass::BulkBlob => self.blob_depth = self.blob_depth.saturating_sub(1),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BackpressureSignal
// ─────────────────────────────────────────────────────────────────────────────

/// Signals for backpressure state changes.
///
/// Emitted when a queue transitions between non-full and full states,
/// allowing downstream consumers to apply backpressure or release it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackpressureEvent {
    /// A queue became full and can no longer accept writes.
    QueueFull {
        class: WriteClass,
        depth: usize,
        capacity: usize,
    },
    /// A queue had capacity available after being full.
    QueueWritable {
        class: WriteClass,
        remaining_capacity: usize,
    },
}

/// Thread-safe backpressure signal that notifies observers of queue state changes.
///
/// Observers can use this to apply backpressure to producers when queues are full,
/// and release it when capacity becomes available.
#[derive(Debug)]
pub struct BackpressureSignal {
    critical_full: AtomicBool,
    projection_full: AtomicBool,
    blob_full: AtomicBool,
    last_event: Mutex<Option<BackpressureEvent>>,
}

impl BackpressureSignal {
    /// Creates a new backpressure signal with all queues initially not full.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            critical_full: AtomicBool::new(false),
            projection_full: AtomicBool::new(false),
            blob_full: AtomicBool::new(false),
            last_event: Mutex::new(None),
        }
    }

    /// Returns `true` if the queue for the given class is experiencing backpressure.
    #[must_use]
    pub fn is_backpressured(&self, class: WriteClass) -> bool {
        match class {
            WriteClass::CriticalControlPlane => self.critical_full.load(Ordering::SeqCst),
            WriteClass::OperatorProjection => self.projection_full.load(Ordering::SeqCst),
            WriteClass::BulkBlob => self.blob_full.load(Ordering::SeqCst),
        }
    }

    /// Returns `true` if any queue is experiencing backpressure.
    #[must_use]
    pub fn any_backpressured(&self) -> bool {
        self.critical_full.load(Ordering::SeqCst)
            || self.projection_full.load(Ordering::SeqCst)
            || self.blob_full.load(Ordering::SeqCst)
    }

    /// Returns the most recent backpressure event, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn last_event(&self) -> Option<BackpressureEvent> {
        #[expect(clippy::unwrap_used)]
        self.last_event.lock().unwrap().clone()
    }

    /// Called when a queue becomes full.
    #[allow(clippy::unwrap_used)]
    pub(crate) fn set_full(&self, class: WriteClass, depth: usize, capacity: usize) {
        let was_full = match class {
            WriteClass::CriticalControlPlane => self.critical_full.swap(true, Ordering::SeqCst),
            WriteClass::OperatorProjection => self.projection_full.swap(true, Ordering::SeqCst),
            WriteClass::BulkBlob => self.blob_full.swap(true, Ordering::SeqCst),
        };

        if !was_full {
            let event = BackpressureEvent::QueueFull {
                class,
                depth,
                capacity,
            };
            #[expect(clippy::unwrap_used)]
            {
                *self.last_event.lock().unwrap() = Some(event);
            }
        }
    }

    /// Called when a queue becomes writable (was full, now has capacity).
    #[allow(clippy::unwrap_used)]
    pub(crate) fn set_writable(&self, class: WriteClass, remaining_capacity: usize) {
        let was_full = match class {
            WriteClass::CriticalControlPlane => self.critical_full.swap(false, Ordering::SeqCst),
            WriteClass::OperatorProjection => self.projection_full.swap(false, Ordering::SeqCst),
            WriteClass::BulkBlob => self.blob_full.swap(false, Ordering::SeqCst),
        };

        if was_full {
            let event = BackpressureEvent::QueueWritable {
                class,
                remaining_capacity,
            };
            #[expect(clippy::unwrap_used)]
            {
                *self.last_event.lock().unwrap() = Some(event);
            }
        }
    }

    /// Returns `true` if writes of this class should be rejected due to backpressure.
    ///
    /// Note: `CriticalControlPlane` writes are never rejected due to backpressure
    /// per ADR-032 (they must never be dropped).
    #[must_use]
    pub fn should_reject(&self, class: WriteClass) -> bool {
        match class {
            WriteClass::CriticalControlPlane => false,
            WriteClass::OperatorProjection | WriteClass::BulkBlob => self.is_backpressured(class),
        }
    }
}

impl Default for BackpressureSignal {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommitLatencyTracker
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks commit latency for monitoring and backpressure decisions.
#[derive(Debug, Default)]
pub struct CommitLatencyTracker {
    state: Mutex<CommitLatencyState>,
}

#[derive(Debug, Default)]
struct CommitLatencyState {
    last_commit_at: Option<Instant>,
    sample_count: u64,
    total_latency_ms: u128,
}

impl CommitLatencyTracker {
    /// Records a commit completion with the given latency in milliseconds.
    ///
    /// # Panics
    ///
    /// Panics if any internal mutex is poisoned.
    pub fn record_commit(&self, latency_ms: u64) {
        #[expect(clippy::unwrap_used)]
        let mut state = self.state.lock().unwrap();
        state.last_commit_at = Some(Instant::now());
        state.sample_count += 1;
        state.total_latency_ms += u128::from(latency_ms);
    }

    /// Returns the time since the last commit, if any.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn time_since_last_commit(&self) -> Option<std::time::Duration> {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        state.last_commit_at.map(|instant| instant.elapsed())
    }

    /// Returns the average commit latency in milliseconds, if samples exist.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn average_latency_ms(&self) -> Option<u64> {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        if state.sample_count == 0 {
            return None;
        }
        Some(
            u64::try_from(state.total_latency_ms / u128::from(state.sample_count))
                .unwrap_or(u64::MAX),
        )
    }

    /// Returns the number of commit samples recorded.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        #[expect(clippy::unwrap_used)]
        let state = self.state.lock().unwrap();
        state.sample_count
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BudgetQueues
// ─────────────────────────────────────────────────────────────────────────────

/// Errors returned by `BudgetQueues` operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BudgetQueuesError {
    #[error("queue full for {class:?}: {depth}/{capacity}")]
    QueueFull {
        class: WriteClass,
        depth: usize,
        capacity: usize,
    },
    #[error("budget exceeded for {class:?}: item size {item_size}, remaining {remaining}")]
    BudgetExceeded {
        class: WriteClass,
        item_size: u64,
        remaining: u64,
    },
}

/// Trait for items that can be queued with `WriteClass` awareness.
pub trait ClassifiedWrite {
    fn write_class(&self) -> WriteClass;
    fn size_bytes(&self) -> u64;
}

/// Inner queue implementation using `VecDeque`.
struct InnerQueue<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T> InnerQueue<T> {
    const fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::new(),
            capacity,
        }
    }

    fn push(&mut self, item: T) -> Option<T> {
        if self.items.len() >= self.capacity {
            return Some(item);
        }
        self.items.push_back(item);
        None
    }

    fn pop(&mut self) -> Option<T> {
        self.items.pop_front()
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    const fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Bounded write queues per `WriteClass` with integrated budget tracking.
pub struct BudgetQueues<T> {
    config: QueueConfig,
    stats: Arc<Mutex<QueueStats>>,
    budget: WriteBudget,
    backpressure: Arc<BackpressureSignal>,
    critical_queue: Mutex<InnerQueue<T>>,
    projection_queue: Mutex<InnerQueue<T>>,
    blob_queue: Mutex<InnerQueue<T>>,
}

impl<T> BudgetQueues<T> {
    /// Creates new budget queues with the given configuration and budget.
    pub fn new(config: &QueueConfig, budget: WriteBudget) -> Self {
        let critical_cap = config.critical_capacity;
        let projection_cap = config.projection_capacity;
        let blob_cap = config.blob_capacity;
        Self {
            config: config.clone(),
            stats: Arc::new(Mutex::new(QueueStats {
                critical_depth: 0,
                projection_depth: 0,
                blob_depth: 0,
                config: config.clone(),
            })),
            budget,
            backpressure: Arc::new(BackpressureSignal::new()),
            critical_queue: Mutex::new(InnerQueue::new(critical_cap)),
            projection_queue: Mutex::new(InnerQueue::new(projection_cap)),
            blob_queue: Mutex::new(InnerQueue::new(blob_cap)),
        }
    }

    /// Creates new budget queues with the given configuration, budget, and shared backpressure signal.
    ///
    /// This constructor allows multiple `BudgetQueues` instances to share the same
    /// backpressure signal, useful when coordinating multiple queue subsystems.
    pub fn new_with_backpressure(
        config: &QueueConfig,
        budget: WriteBudget,
        backpressure: Arc<BackpressureSignal>,
    ) -> Self {
        let critical_cap = config.critical_capacity;
        let projection_cap = config.projection_capacity;
        let blob_cap = config.blob_capacity;
        Self {
            config: config.clone(),
            stats: Arc::new(Mutex::new(QueueStats {
                critical_depth: 0,
                projection_depth: 0,
                blob_depth: 0,
                config: config.clone(),
            })),
            budget,
            backpressure,
            critical_queue: Mutex::new(InnerQueue::new(critical_cap)),
            projection_queue: Mutex::new(InnerQueue::new(projection_cap)),
            blob_queue: Mutex::new(InnerQueue::new(blob_cap)),
        }
    }

    /// Returns a reference to the queue statistics.
    #[must_use]
    pub fn stats(&self) -> Arc<Mutex<QueueStats>> {
        Arc::clone(&self.stats)
    }

    /// Returns a reference to the underlying budget.
    #[must_use]
    pub const fn budget(&self) -> &WriteBudget {
        &self.budget
    }

    /// Returns a reference to the backpressure signal.
    #[must_use]
    pub const fn backpressure(&self) -> &Arc<BackpressureSignal> {
        &self.backpressure
    }

    /// Attempts to enqueue an item if budget and queue capacity allow.
    ///
    /// Emits backpressure signals when queues become full.
    ///
    /// # Errors
    /// Returns `BudgetQueuesError` if budget is exceeded or queue is full.
    pub fn try_enqueue(&self, item: &T) -> Result<(), BudgetQueuesError>
    where
        T: ClassifiedWrite + Clone,
    {
        let class = item.write_class();
        let size = item.size_bytes();

        // Check budget first
        if !self.budget.can_write(class, size) {
            emit_rejection(class, "budget_exceeded");
            return Err(BudgetQueuesError::BudgetExceeded {
                class,
                item_size: size,
                remaining: self.budget.remaining(class),
            });
        }

        // Get the appropriate queue
        let queue: &Mutex<InnerQueue<T>> = match class {
            WriteClass::CriticalControlPlane => &self.critical_queue,
            WriteClass::OperatorProjection => &self.projection_queue,
            WriteClass::BulkBlob => &self.blob_queue,
        };

        // Try to push to the queue
        let overflow = {
            let mut q = match queue.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if q.is_full() {
                let depth = q.len();
                let capacity = q.capacity();
                emit_rejection(class, "queue_full");
                self.backpressure.set_full(class, depth, capacity);
                return Err(BudgetQueuesError::QueueFull {
                    class,
                    depth,
                    capacity,
                });
            }
            q.push((*item).clone())
        };

        // If overflow, return error
        if overflow.is_some() {
            emit_rejection(class, "queue_full");
            let depth = match self.stats.lock() {
                Ok(guard) => guard.depth(class),
                Err(poisoned) => poisoned.into_inner().depth(class),
            };
            let capacity = match self.stats.lock() {
                Ok(guard) => guard.capacity(class),
                Err(poisoned) => poisoned.into_inner().capacity(class),
            };
            self.backpressure.set_full(class, depth, capacity);
            return Err(BudgetQueuesError::QueueFull {
                class,
                depth,
                capacity,
            });
        }

        // Reserve budget
        if let Err(e) = self.budget.reserve(class, size) {
            // Rollback queue push - merge temporary with its single usage
            match queue.lock() {
                Ok(mut guard) => guard.pop(),
                Err(poisoned) => poisoned.into_inner().pop(),
            };
            emit_rejection(class, "budget_exceeded");
            return Err(BudgetQueuesError::BudgetExceeded {
                class,
                item_size: size,
                remaining: e.available,
            });
        }

        // Update stats
        let new_depth = {
            let mut guard = match self.stats.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.increment(class);
            guard.depth(class)
        };

        emit_queue_depth(class, new_depth);

        Ok(())
    }

    /// Dequeues an item from the front of the specified queue.
    ///
    /// Emits backpressure signals when queues transition from full to having capacity.
    pub fn dequeue(&self, class: WriteClass) -> Option<T>
    where
        T: ClassifiedWrite,
    {
        let queue: &Mutex<InnerQueue<T>> = match class {
            WriteClass::CriticalControlPlane => &self.critical_queue,
            WriteClass::OperatorProjection => &self.projection_queue,
            WriteClass::BulkBlob => &self.blob_queue,
        };

        let item = {
            let mut q = match queue.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.pop()
        };
        if item.is_some() {
            let was_full = match self.stats.lock() {
                Ok(guard) => guard.is_full(class),
                Err(poisoned) => poisoned.into_inner().is_full(class),
            };

            let new_depth = {
                let mut guard = match self.stats.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.decrement(class);
                guard.depth(class)
            };

            emit_queue_depth(class, new_depth);

            if was_full {
                let remaining = match self.stats.lock() {
                    Ok(guard) => guard.remaining(class),
                    Err(poisoned) => poisoned.into_inner().remaining(class),
                };
                self.backpressure.set_writable(class, remaining);
            }
        }
        item
    }

    /// Dequeues items in priority order: `CriticalControlPlane` → `OperatorProjection` → `BulkBlob`.
    ///
    /// Returns the next item available in priority order, or `None` if all queues are empty.
    ///
    /// This method implements ADR-032 priority-based write ordering, ensuring that
    /// critical control-plane writes are always serviced before lower-priority writes.
    pub fn dequeue_prioritized(&self) -> Option<(WriteClass, T)>
    where
        T: ClassifiedWrite,
    {
        // Try critical first (highest priority)
        if let Some(item) = self.dequeue(WriteClass::CriticalControlPlane) {
            return Some((WriteClass::CriticalControlPlane, item));
        }

        // Then projection
        if let Some(item) = self.dequeue(WriteClass::OperatorProjection) {
            return Some((WriteClass::OperatorProjection, item));
        }

        // Then blob (lowest priority)
        if let Some(item) = self.dequeue(WriteClass::BulkBlob) {
            return Some((WriteClass::BulkBlob, item));
        }

        None
    }

    /// Returns the queue configuration.
    #[must_use]
    pub const fn config(&self) -> &QueueConfig {
        &self.config
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AppendEntry
// ─────────────────────────────────────────────────────────────────────────────

/// Entry in the append queue.
#[derive(Debug, Clone)]
pub enum AppendEntry {
    ControlPlane(ControlPlaneWrite),
    Projection(ProjectionWrite),
    Blob(BlobWrite),
}

impl ClassifiedWrite for AppendEntry {
    fn write_class(&self) -> WriteClass {
        match self {
            Self::ControlPlane(w) => w.write_class(),
            Self::Projection(w) => w.write_class(),
            Self::Blob(w) => w.write_class(),
        }
    }

    fn size_bytes(&self) -> u64 {
        match self {
            Self::ControlPlane(w) => w.size_bytes(),
            Self::Projection(w) => w.size_bytes(),
            Self::Blob(w) => w.size_bytes(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Write Types
// ─────────────────────────────────────────────────────────────────────────────

/// A control-plane write.
#[derive(Debug, Clone)]
pub struct ControlPlaneWrite {
    pub event: EventEnvelope,
    size_bytes: u64,
}

impl ControlPlaneWrite {
    #[must_use]
    pub const fn new(event: EventEnvelope, size_bytes: u64) -> Self {
        Self { event, size_bytes }
    }
}

impl ClassifiedWrite for ControlPlaneWrite {
    fn write_class(&self) -> WriteClass {
        WriteClass::CriticalControlPlane
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

/// A projection write.
#[derive(Debug, Clone)]
pub struct ProjectionWrite {
    pub projection_id: String,
    size_bytes: u64,
}

impl ProjectionWrite {
    #[must_use]
    pub const fn new(projection_id: String, size_bytes: u64) -> Self {
        Self {
            projection_id,
            size_bytes,
        }
    }
}

impl ClassifiedWrite for ProjectionWrite {
    fn write_class(&self) -> WriteClass {
        WriteClass::OperatorProjection
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

/// A blob write.
#[derive(Debug, Clone)]
pub struct BlobWrite {
    pub blob_id: String,
    size_bytes: u64,
    class: WriteClass,
}

impl BlobWrite {
    #[must_use]
    pub const fn bulk(blob_id: String, size_bytes: u64) -> Self {
        Self {
            blob_id,
            size_bytes,
            class: WriteClass::BulkBlob,
        }
    }
}

impl ClassifiedWrite for BlobWrite {
    fn write_class(&self) -> WriteClass {
        self.class
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Appender
// ─────────────────────────────────────────────────────────────────────────────

/// Appender that queues writes by class with budget enforcement.
pub struct Appender {
    queues: BudgetQueues<AppendEntry>,
}

impl Appender {
    /// Creates a new `Appender` with the given queue configuration and budget.
    pub fn new(config: &QueueConfig, budget: WriteBudget) -> Self {
        Self {
            queues: BudgetQueues::new(&config, budget),
        }
    }

    #[must_use]
    pub fn stats(&self) -> Arc<Mutex<QueueStats>> {
        self.queues.stats()
    }

    #[must_use]
    pub const fn budget(&self) -> &WriteBudget {
        self.queues.budget()
    }

    #[must_use]
    pub const fn backpressure(&self) -> &Arc<BackpressureSignal> {
        self.queues.backpressure()
    }

    /// Appends a control-plane write.
    ///
    /// # Errors
    /// Returns `BudgetQueuesError` if budget is exceeded or queue is full.
    pub fn append_control_plane(&self, write: ControlPlaneWrite) -> Result<(), BudgetQueuesError> {
        self.queues.try_enqueue(&AppendEntry::ControlPlane(write))
    }

    /// Appends a projection write.
    ///
    /// # Errors
    /// Returns `BudgetQueuesError` if budget is exceeded or queue is full.
    pub fn append_projection(&self, write: ProjectionWrite) -> Result<(), BudgetQueuesError> {
        self.queues.try_enqueue(&AppendEntry::Projection(write))
    }

    /// Appends a blob write.
    ///
    /// # Errors
    /// Returns `BudgetQueuesError` if budget is exceeded or queue is full.
    pub fn append_blob(&self, write: BlobWrite) -> Result<(), BudgetQueuesError> {
        self.queues.try_enqueue(&AppendEntry::Blob(write))
    }

    /// Dequeues from the critical control-plane queue.
    pub fn dequeue_critical(&self) -> Option<AppendEntry> {
        self.queues.dequeue(WriteClass::CriticalControlPlane)
    }

    /// Dequeues from the projection queue.
    pub fn dequeue_projection(&self) -> Option<AppendEntry> {
        self.queues.dequeue(WriteClass::OperatorProjection)
    }

    /// Dequeues from the blob queue.
    pub fn dequeue_blob(&self) -> Option<AppendEntry> {
        self.queues.dequeue(WriteClass::BulkBlob)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;

pub use appender::Appender;
pub use backpressure::{BackpressureEvent, BackpressureSignal};
pub use budget::{BudgetError, WriteBudget};
pub use entries::{AppendEntry, BlobWrite, ControlPlaneWrite, ProjectionWrite};
pub use latency::CommitLatencyTracker;
pub use queue::{BudgetQueues, BudgetQueuesError, ClassifiedWrite, QueueConfig, QueueStats};
pub use write_class::WriteClass;
