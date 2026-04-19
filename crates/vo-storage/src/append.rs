//! Append operations with per-write-class queue budgeting.
//!
//! This module provides the append path for storage writes, implementing
//! traffic isolation via bounded channels per write class.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
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
    // Casting usize to f64 for metrics - precision loss is acceptable for large queue depths
    #[allow(clippy::cast_precision_loss)]
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
    critical_used: RefCell<u64>,
    projection_used: RefCell<u64>,
    blob_used: RefCell<u64>,
}

impl WriteBudget {
    /// Creates a new budget with the given limits per class.
    #[must_use]
    pub const fn new(critical_limit: u64, projection_limit: u64, blob_limit: u64) -> Self {
        Self {
            critical_limit,
            projection_limit,
            blob_limit,
            critical_used: RefCell::new(0),
            projection_used: RefCell::new(0),
            blob_used: RefCell::new(0),
        }
    }

    /// Returns the remaining budget for a given class.
    #[must_use]
    pub fn remaining(&self, class: WriteClass) -> u64 {
        match class {
            WriteClass::CriticalControlPlane => self
                .critical_limit
                .saturating_sub(*self.critical_used.borrow()),
            WriteClass::OperatorProjection => self
                .projection_limit
                .saturating_sub(*self.projection_used.borrow()),
            WriteClass::BulkBlob => self.blob_limit.saturating_sub(*self.blob_used.borrow()),
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
                *self.critical_used.borrow_mut() += size_bytes;
            }
            WriteClass::OperatorProjection => {
                *self.projection_used.borrow_mut() += size_bytes;
            }
            WriteClass::BulkBlob => {
                *self.blob_used.borrow_mut() += size_bytes;
            }
        }
        Ok(())
    }

    pub fn release(&self, class: WriteClass, size_bytes: u64) {
        match class {
            WriteClass::CriticalControlPlane => {
                let current = self.critical_used.borrow().saturating_sub(size_bytes);
                *self.critical_used.borrow_mut() = current;
            }
            WriteClass::OperatorProjection => {
                let current = self.projection_used.borrow().saturating_sub(size_bytes);
                *self.projection_used.borrow_mut() = current;
            }
            WriteClass::BulkBlob => {
                let current = self.blob_used.borrow().saturating_sub(size_bytes);
                *self.blob_used.borrow_mut() = current;
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
    pub fn new() -> Self {
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
    #[must_use]
    pub fn last_event(&self) -> Option<BackpressureEvent> {
        #[expect(clippy::unwrap_used)]
        self.last_event.lock().unwrap().clone()
    }

    /// Called when a queue becomes full.
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
    last_commit_at: Mutex<Option<Instant>>,
    sample_count: Mutex<u64>,
    total_latency_ms: Mutex<u128>,
}

impl CommitLatencyTracker {
    /// Records a commit completion with the given latency in milliseconds.
    pub fn record_commit(&self, latency_ms: u64) {
        #[expect(clippy::unwrap_used)]
        let mut last_commit = self.last_commit_at.lock().unwrap();
        *last_commit = Some(Instant::now());

        #[expect(clippy::unwrap_used)]
        let mut count = self.sample_count.lock().unwrap();
        *count += 1;

        #[expect(clippy::unwrap_used)]
        let mut total = self.total_latency_ms.lock().unwrap();
        *total += latency_ms as u128;
    }

    /// Returns the time since the last commit, if any.
    #[must_use]
    pub fn time_since_last_commit(&self) -> Option<std::time::Duration> {
        #[expect(clippy::unwrap_used)]
        let last_commit = self.last_commit_at.lock().unwrap();
        last_commit.map(|instant| instant.elapsed())
    }

    /// Returns the average commit latency in milliseconds, if samples exist.
    #[must_use]
    pub fn average_latency_ms(&self) -> Option<u64> {
        #[expect(clippy::unwrap_used)]
        let count = *self.sample_count.lock().unwrap();
        if count == 0 {
            return None;
        }
        #[expect(clippy::unwrap_used)]
        let total = *self.total_latency_ms.lock().unwrap();
        Some((total / count as u128) as u64)
    }

    #[must_use]
    pub fn sample_count(&self) -> u64 {
        #[expect(clippy::unwrap_used)]
        *self.sample_count.lock().unwrap()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BudgetQueues
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
        config: QueueConfig,
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
    pub fn backpressure(&self) -> &Arc<BackpressureSignal> {
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
        {
            let mut guard = match self.stats.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.increment(class);
        }

        Ok(())
    }

    /// Dequeues an item from the front of the specified queue.
    ///
    /// Emits backpressure signals when queues transition from full to having capacity.
    pub fn dequeue(&self, class: WriteClass) -> Option<T> {
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

            match self.stats.lock() {
                Ok(mut guard) => guard.decrement(class),
                Err(poisoned) => poisoned.into_inner().decrement(class),
            }

            if was_full {
                let remaining = match self.stats.lock() {
                    Ok(guard) => guard.remaining(class),
                    Err(poisoned) => poisoned.into_inner().remaining(class),
                };
                self.backpressure.set_writable(class, remaining);
            }

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

    /// Dequeues items in priority order: CriticalControlPlane → OperatorProjection → BulkBlob.
    ///
    /// Returns the next item available in priority order, or `None` if all queues are empty.
    ///
    /// This method implements ADR-032 priority-based write ordering, ensuring that
    /// critical control-plane writes are always serviced before lower-priority writes.
    pub fn dequeue_prioritized(&self) -> Option<(WriteClass, T)> {
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
            queues: BudgetQueues::new(config, budget),
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
    pub fn backpressure(&self) -> &Arc<BackpressureSignal> {
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
mod tests {
    use super::*;
    use vo_types::events::EventMetadata;

    #[test]
    fn write_class_tier() {
        assert_eq!(WriteClass::CriticalControlPlane.tier(), 1);
        assert_eq!(WriteClass::OperatorProjection.tier(), 2);
        assert_eq!(WriteClass::BulkBlob.tier(), 3);
    }

    #[test]
    fn write_class_never_drops() {
        assert!(WriteClass::CriticalControlPlane.never_drops());
        assert!(!WriteClass::OperatorProjection.never_drops());
        assert!(!WriteClass::BulkBlob.never_drops());
    }

    #[test]
    fn write_class_from_str() {
        assert_eq!(
            "critical_control_plane".parse::<WriteClass>().unwrap(),
            WriteClass::CriticalControlPlane
        );
        assert_eq!(
            "operator_projection".parse::<WriteClass>().unwrap(),
            WriteClass::OperatorProjection
        );
        assert_eq!(
            "bulk_blob".parse::<WriteClass>().unwrap(),
            WriteClass::BulkBlob
        );
        assert!("invalid".parse::<WriteClass>().is_err());
    }

    #[test]
    fn write_budget_remaining() {
        let budget = WriteBudget::new(100, 200, 300);
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
        assert_eq!(budget.remaining(WriteClass::OperatorProjection), 200);
        assert_eq!(budget.remaining(WriteClass::BulkBlob), 300);
    }

    #[test]
    fn write_budget_reserve() {
        let budget = WriteBudget::new(100, 200, 300);
        assert!(budget.reserve(WriteClass::CriticalControlPlane, 50).is_ok());
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 50);
    }

    #[test]
    fn write_budget_exceeded() {
        let budget = WriteBudget::new(100, 200, 300);
        let result = budget.reserve(WriteClass::CriticalControlPlane, 150);
        assert!(result.is_err());
        assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 100);
    }

    #[test]
    fn queue_config_default() {
        let config = QueueConfig::default();
        assert_eq!(config.critical_capacity, 1024);
        assert_eq!(config.projection_capacity, 512);
        assert_eq!(config.blob_capacity, 256);
    }

    #[test]
    fn append_entry_classification() {
        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        let cp_write = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
        assert_eq!(cp_write.write_class(), WriteClass::CriticalControlPlane);

        let proj_write = AppendEntry::Projection(ProjectionWrite {
            projection_id: "proj-1".to_string(),
            size_bytes: 200,
        });
        assert_eq!(proj_write.write_class(), WriteClass::OperatorProjection);

        let blob_write = AppendEntry::Blob(BlobWrite::bulk("blob-1".to_string(), 300));
        assert_eq!(blob_write.write_class(), WriteClass::BulkBlob);
    }

    #[test]
    fn appender_queues_control_plane_write() {
        let config = QueueConfig::default();
        let budget = WriteBudget::new(10000, 10000, 10000);
        let appender = Appender::new(&config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };
        let write = ControlPlaneWrite::new(event, 100);

        let result = appender.append_control_plane(write);
        assert!(result.is_ok());
    }

    #[test]
    fn appender_rejects_when_budget_exhausted() {
        let config = QueueConfig::default();
        let budget = WriteBudget::new(50, 50, 50);
        let appender = Appender::new(&config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({"data": "this is larger than 50 bytes to force budget exceeded"}),
            metadata: EventMetadata::default(),
        };
        let write = ControlPlaneWrite::new(event, 100);

        let result = appender.append_control_plane(write);
        assert!(matches!(
            result,
            Err(BudgetQueuesError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn appender_rejects_when_queue_full() {
        let config = QueueConfig {
            critical_capacity: 1,
            projection_capacity: 1,
            blob_capacity: 1,
        };
        let budget = WriteBudget::new(10000, 10000, 10000);
        let appender = Appender::new(&config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        let write1 = ControlPlaneWrite::new(event.clone(), 100);
        assert!(appender.append_control_plane(write1).is_ok());

        let write2 = ControlPlaneWrite::new(event, 100);
        let result = appender.append_control_plane(write2);
        assert!(matches!(result, Err(BudgetQueuesError::QueueFull { .. })));
    }

    #[test]
    fn appender_dequeue_returns_queued_items() {
        let config = QueueConfig::default();
        let budget = WriteBudget::new(10000, 10000, 10000);
        let appender = Appender::new(&config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };
        let write = ControlPlaneWrite::new(event, 100);
        assert!(appender.append_control_plane(write).is_ok());

        let dequeued = appender.dequeue_critical();
        assert!(dequeued.is_some());

        let dequeued2 = appender.dequeue_critical();
        assert!(dequeued2.is_none());
    }

    // ── BackpressureSignal Tests ────────────────────────────────────────────────

    #[test]
    fn backpressure_signal_initial_not_backpressured() {
        let signal = BackpressureSignal::new();
        assert!(!signal.is_backpressured(WriteClass::CriticalControlPlane));
        assert!(!signal.is_backpressured(WriteClass::OperatorProjection));
        assert!(!signal.is_backpressured(WriteClass::BulkBlob));
        assert!(!signal.any_backpressured());
    }

    #[test]
    fn backpressure_signal_set_full_emits_event() {
        let signal = BackpressureSignal::new();
        signal.set_full(WriteClass::OperatorProjection, 50, 100);

        assert!(signal.is_backpressured(WriteClass::OperatorProjection));
        assert!(!signal.is_backpressured(WriteClass::CriticalControlPlane));
        assert!(!signal.is_backpressured(WriteClass::BulkBlob));
        assert!(signal.any_backpressured()); // Projection IS backpressured

        let event = signal.last_event();
        assert!(matches!(
            event,
            Some(BackpressureEvent::QueueFull {
                class: WriteClass::OperatorProjection,
                depth: 50,
                capacity: 100,
            })
        ));
    }

    #[test]
    fn backpressure_signal_set_writable_clears_backpressure() {
        let signal = BackpressureSignal::new();
        signal.set_full(WriteClass::OperatorProjection, 50, 50);
        assert!(signal.is_backpressured(WriteClass::OperatorProjection));

        signal.set_writable(WriteClass::OperatorProjection, 10);
        assert!(!signal.is_backpressured(WriteClass::OperatorProjection));

        let event = signal.last_event();
        assert!(matches!(
            event,
            Some(BackpressureEvent::QueueWritable {
                class: WriteClass::OperatorProjection,
                remaining_capacity: 10,
            })
        ));
    }

    #[test]
    fn backpressure_signal_critical_never_rejects() {
        let signal = BackpressureSignal::new();
        // Set critical and projection (but not blob) to full
        signal.set_full(WriteClass::CriticalControlPlane, 1024, 1024);
        signal.set_full(WriteClass::OperatorProjection, 100, 100);

        // Critical writes should never be rejected even when full
        assert!(!signal.should_reject(WriteClass::CriticalControlPlane));
        // But projection should be rejected when full
        assert!(signal.should_reject(WriteClass::OperatorProjection));
        // Blob is NOT full, so should not be rejected
        assert!(!signal.should_reject(WriteClass::BulkBlob));
    }

    #[test]
    fn backpressure_signal_any_backpressured() {
        let signal = BackpressureSignal::new();
        assert!(!signal.any_backpressured());

        signal.set_full(WriteClass::BulkBlob, 256, 256);
        assert!(signal.any_backpressured());

        signal.set_writable(WriteClass::BulkBlob, 1);
        assert!(!signal.any_backpressured());
    }

    // ── CommitLatencyTracker Tests ─────────────────────────────────────────────

    #[test]
    fn commit_latency_tracker_initial_no_data() {
        let tracker = CommitLatencyTracker::default();
        assert!(tracker.average_latency_ms().is_none());
        assert!(tracker.time_since_last_commit().is_none());
        assert_eq!(tracker.sample_count(), 0);
    }

    #[test]
    fn commit_latency_tracker_records_commits() {
        let tracker = CommitLatencyTracker::default();
        tracker.record_commit(100);
        tracker.record_commit(200);

        assert_eq!(tracker.sample_count(), 2);
        assert_eq!(tracker.average_latency_ms(), Some(150)); // (100+200)/2
        assert!(tracker.time_since_last_commit().is_some());
    }

    // ── BudgetQueues Backpressure Integration Tests ─────────────────────────────

    #[test]
    fn budget_queues_emits_backpressure_on_full() {
        let config = QueueConfig {
            critical_capacity: 1,
            projection_capacity: 1,
            blob_capacity: 1,
        };
        let budget = WriteBudget::new(10000, 10000, 10000);
        let queues = BudgetQueues::new(config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        // First write should succeed
        let write1 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
        assert!(queues.try_enqueue(&write1).is_ok());
        assert!(!queues
            .backpressure()
            .is_backpressured(WriteClass::CriticalControlPlane));

        // Second write should fail and emit backpressure
        let write2 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event, 100));
        let result = queues.try_enqueue(&write2);
        assert!(matches!(result, Err(BudgetQueuesError::QueueFull { .. })));
        assert!(queues
            .backpressure()
            .is_backpressured(WriteClass::CriticalControlPlane));
    }

    #[test]
    fn budget_queues_clears_backpressure_on_dequeue() {
        let config = QueueConfig {
            critical_capacity: 1,
            projection_capacity: 1,
            blob_capacity: 1,
        };
        let budget = WriteBudget::new(10000, 10000, 10000);
        let queues = BudgetQueues::new(config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        // Fill the queue
        let write1 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
        assert!(queues.try_enqueue(&write1).is_ok());

        let write2 = AppendEntry::ControlPlane(ControlPlaneWrite::new(event.clone(), 100));
        assert!(matches!(
            queues.try_enqueue(&write2),
            Err(BudgetQueuesError::QueueFull { .. })
        ));

        // Backpressure should be set
        assert!(queues
            .backpressure()
            .is_backpressured(WriteClass::CriticalControlPlane));

        // Dequeue should clear backpressure
        let dequeued = queues.dequeue(WriteClass::CriticalControlPlane);
        assert!(dequeued.is_some());
        assert!(!queues
            .backpressure()
            .is_backpressured(WriteClass::CriticalControlPlane));
    }

    // ── Priority Dequeue Tests ─────────────────────────────────────────────────

    #[test]
    fn dequeue_prioritized_returns_critical_first() {
        let config = QueueConfig::default();
        let budget = WriteBudget::new(10000, 10000, 10000);
        let queues = BudgetQueues::new(config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        // Enqueue in reverse priority order
        queues
            .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk(
                "blob-1".to_string(),
                100,
            )))
            .unwrap();
        queues
            .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
                projection_id: "proj-1".to_string(),
                size_bytes: 100,
            }))
            .unwrap();
        queues
            .try_enqueue(&AppendEntry::ControlPlane(ControlPlaneWrite::new(
                event.clone(),
                100,
            )))
            .unwrap();

        // Should get critical first
        let (class, _) = queues.dequeue_prioritized().unwrap();
        assert_eq!(class, WriteClass::CriticalControlPlane);

        // Then projection
        let (class, _) = queues.dequeue_prioritized().unwrap();
        assert_eq!(class, WriteClass::OperatorProjection);

        // Then blob
        let (class, _) = queues.dequeue_prioritized().unwrap();
        assert_eq!(class, WriteClass::BulkBlob);

        // Then none
        assert!(queues.dequeue_prioritized().is_none());
    }

    #[test]
    fn dequeue_prioritized_skips_empty_queues() {
        let config = QueueConfig::default();
        let budget = WriteBudget::new(10000, 10000, 10000);
        let queues = BudgetQueues::new(config, budget);

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        // Only enqueue projection and blob (no critical)
        queues
            .try_enqueue(&AppendEntry::Blob(BlobWrite::bulk(
                "blob-1".to_string(),
                100,
            )))
            .unwrap();
        queues
            .try_enqueue(&AppendEntry::Projection(ProjectionWrite {
                projection_id: "proj-1".to_string(),
                size_bytes: 100,
            }))
            .unwrap();

        // Should get projection first (critical is empty)
        let (class, _) = queues.dequeue_prioritized().unwrap();
        assert_eq!(class, WriteClass::OperatorProjection);

        // Then blob
        let (class, _) = queues.dequeue_prioritized().unwrap();
        assert_eq!(class, WriteClass::BulkBlob);
    }

    // ── Appender Backpressure Tests ───────────────────────────────────────────

    #[test]
    fn appender_backpressure_signal_integrated() {
        let config = QueueConfig {
            critical_capacity: 2,
            projection_capacity: 1,
            blob_capacity: 1,
        };
        let budget = WriteBudget::new(10000, 10000, 10000);
        let appender = Appender::new(config, budget);

        let signal = appender.backpressure().clone();

        let event = EventEnvelope {
            schema_version: 1,
            instance_id: "inst-1".to_string(),
            sequence: 1,
            timestamp_ms: 1000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        };

        // Fill projection queue
        let write1 = ProjectionWrite {
            projection_id: "proj-1".to_string(),
            size_bytes: 100,
        };
        assert!(appender.append_projection(write1).is_ok());

        let write2 = ProjectionWrite {
            projection_id: "proj-2".to_string(),
            size_bytes: 100,
        };
        assert!(matches!(
            appender.append_projection(write2),
            Err(BudgetQueuesError::QueueFull { .. })
        ));

        // Backpressure should be set on projection
        assert!(signal.is_backpressured(WriteClass::OperatorProjection));
    }
}
