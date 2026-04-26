use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::backpressure::BackpressureSignal;
use super::budget::WriteBudget;
use super::metrics;
use super::write_class::WriteClass;

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

pub trait ClassifiedWrite {
    fn write_class(&self) -> WriteClass;
    fn size_bytes(&self) -> u64;
}

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

    #[must_use]
    pub fn stats(&self) -> Arc<Mutex<QueueStats>> {
        Arc::clone(&self.stats)
    }

    #[must_use]
    pub const fn budget(&self) -> &WriteBudget {
        &self.budget
    }

    #[must_use]
    pub const fn backpressure(&self) -> &Arc<BackpressureSignal> {
        &self.backpressure
    }

    pub fn try_enqueue(&self, item: &T) -> Result<(), BudgetQueuesError>
    where
        T: ClassifiedWrite + Clone,
    {
        let class = item.write_class();
        let size = item.size_bytes();

        if !self.budget.can_write(class, size) {
            metrics::emit_rejection(class, "budget_exceeded");
            return Err(BudgetQueuesError::BudgetExceeded {
                class,
                item_size: size,
                remaining: self.budget.remaining(class),
            });
        }

        let queue: &Mutex<InnerQueue<T>> = match class {
            WriteClass::CriticalControlPlane => &self.critical_queue,
            WriteClass::OperatorProjection => &self.projection_queue,
            WriteClass::BulkBlob => &self.blob_queue,
        };

        let overflow = {
            let mut q = match queue.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if q.is_full() {
                let depth = q.len();
                let capacity = q.capacity();
                metrics::emit_rejection(class, "queue_full");
                self.backpressure.set_full(class, depth, capacity);
                return Err(BudgetQueuesError::QueueFull {
                    class,
                    depth,
                    capacity,
                });
            }
            q.push((*item).clone())
        };

        if overflow.is_some() {
            metrics::emit_rejection(class, "queue_full");
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

        if let Err(e) = self.budget.reserve(class, size) {
            match queue.lock() {
                Ok(mut guard) => guard.pop(),
                Err(poisoned) => poisoned.into_inner().pop(),
            };
            metrics::emit_rejection(class, "budget_exceeded");
            return Err(BudgetQueuesError::BudgetExceeded {
                class,
                item_size: size,
                remaining: e.available,
            });
        }

        let new_depth = {
            let mut guard = match self.stats.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.increment(class);
            guard.depth(class)
        };

        metrics::emit_queue_depth(class, new_depth);

        Ok(())
    }

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

            metrics::emit_queue_depth(class, new_depth);

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

    pub fn dequeue_prioritized(&self) -> Option<(WriteClass, T)>
    where
        T: ClassifiedWrite,
    {
        if let Some(item) = self.dequeue(WriteClass::CriticalControlPlane) {
            return Some((WriteClass::CriticalControlPlane, item));
        }

        if let Some(item) = self.dequeue(WriteClass::OperatorProjection) {
            return Some((WriteClass::OperatorProjection, item));
        }

        if let Some(item) = self.dequeue(WriteClass::BulkBlob) {
            return Some((WriteClass::BulkBlob, item));
        }

        None
    }

    #[must_use]
    pub const fn config(&self) -> &QueueConfig {
        &self.config
    }
}
