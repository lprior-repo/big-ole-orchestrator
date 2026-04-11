//! Append operations with per-write-class queue budgeting.
//!
//! This module provides the append path for storage writes, implementing
//! traffic isolation via bounded channels per write class.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use vo_types::events::EventEnvelope;
#[cfg(test)]
use vo_types::events::EventMetadata;

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
}

/// Budget exceeded error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetError {
    pub class: WriteClass,
    pub requested: u64,
    pub available: u64,
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "budget exceeded for {:?}: requested {}, available {}",
            self.class, self.requested, self.available
        )
    }
}

impl std::error::Error for BudgetError {}

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
// BudgetQueues
// ─────────────────────────────────────────────────────────────────────────────

/// Errors returned by `BudgetQueues` operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetQueuesError {
    QueueFull {
        class: WriteClass,
        depth: usize,
        capacity: usize,
    },
    BudgetExceeded {
        class: WriteClass,
        item_size: u64,
        remaining: u64,
    },
}

impl std::fmt::Display for BudgetQueuesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull {
                class,
                depth,
                capacity,
            } => write!(f, "queue full for {class:?}: {depth}/{capacity}"),
            Self::BudgetExceeded {
                class,
                item_size,
                remaining,
            } => write!(
                f,
                "budget exceeded for {class:?}: item size {item_size}, remaining {remaining}"
            ),
        }
    }
}

impl std::error::Error for BudgetQueuesError {}

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
    critical_queue: Mutex<InnerQueue<T>>,
    projection_queue: Mutex<InnerQueue<T>>,
    blob_queue: Mutex<InnerQueue<T>>,
}

impl<T> BudgetQueues<T> {
    /// Creates new budget queues with the given configuration and budget.
    pub fn new(config: QueueConfig, budget: WriteBudget) -> Self {
        let critical_cap = config.critical_capacity;
        let projection_cap = config.projection_capacity;
        let blob_cap = config.blob_capacity;
        Self {
            config: config.clone(),
            stats: Arc::new(Mutex::new(QueueStats {
                critical_depth: 0,
                projection_depth: 0,
                blob_depth: 0,
                config,
            })),
            budget,
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

    /// Attempts to enqueue an item if budget and queue capacity allow.
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
                return Err(BudgetQueuesError::QueueFull {
                    class,
                    depth: q.len(),
                    capacity: q.capacity(),
                });
            }
            q.push((*item).clone())
        };

        // If overflow, return error
        if overflow.is_some() {
            return Err(BudgetQueuesError::QueueFull {
                class,
                depth: match self.stats.lock() {
                    Ok(guard) => guard.depth(class),
                    Err(poisoned) => poisoned.into_inner().depth(class),
                },
                capacity: match self.stats.lock() {
                    Ok(guard) => guard.capacity(class),
                    Err(poisoned) => poisoned.into_inner().capacity(class),
                },
            });
        }

        // Reserve budget
        if let Err(e) = self.budget.reserve(class, size) {
            // Rollback queue push - merge temporary with its single usage
            match queue.lock() {
                Ok(mut guard) => guard.pop(),
                Err(poisoned) => poisoned.into_inner().pop(),
            };
            return Err(BudgetQueuesError::BudgetExceeded {
                class,
                item_size: size,
                remaining: e.available,
            });
        }

        // Update stats
        match self.stats.lock() {
            Ok(mut guard) => guard.increment(class),
            Err(poisoned) => poisoned.into_inner().increment(class),
        }

        Ok(())
    }

    /// Dequeues an item from the front of the specified queue.
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
            match self.stats.lock() {
                Ok(mut guard) => guard.decrement(class),
                Err(poisoned) => poisoned.into_inner().decrement(class),
            }
        }
        item
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
    pub fn new(config: QueueConfig, budget: WriteBudget) -> Self {
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
        let appender = Appender::new(config, budget);

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
        let appender = Appender::new(config, budget);

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
        let appender = Appender::new(config, budget);

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
        let appender = Appender::new(config, budget);

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
}
