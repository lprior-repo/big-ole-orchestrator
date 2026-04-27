use std::collections::VecDeque;

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
    pub const fn capacity_for(&self, class: super::WriteClass) -> usize {
        match class {
            super::WriteClass::CriticalControlPlane => self.critical_capacity,
            super::WriteClass::OperatorProjection => self.projection_capacity,
            super::WriteClass::BulkBlob => self.blob_capacity,
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
    pub const fn depth(&self, class: super::WriteClass) -> usize {
        match class {
            super::WriteClass::CriticalControlPlane => self.critical_depth,
            super::WriteClass::OperatorProjection => self.projection_depth,
            super::WriteClass::BulkBlob => self.blob_depth,
        }
    }

    #[must_use]
    pub const fn capacity(&self, class: super::WriteClass) -> usize {
        self.config.capacity_for(class)
    }

    #[must_use]
    pub const fn remaining(&self, class: super::WriteClass) -> usize {
        self.capacity(class).saturating_sub(self.depth(class))
    }

    #[must_use]
    pub const fn is_full(&self, class: super::WriteClass) -> bool {
        self.depth(class) >= self.capacity(class)
    }

    fn increment(&mut self, class: super::WriteClass) {
        match class {
            super::WriteClass::CriticalControlPlane => self.critical_depth += 1,
            super::WriteClass::OperatorProjection => self.projection_depth += 1,
            super::WriteClass::BulkBlob => self.blob_depth += 1,
        }
    }

    fn decrement(&mut self, class: super::WriteClass) {
        match class {
            super::WriteClass::CriticalControlPlane => {
                self.critical_depth = self.critical_depth.saturating_sub(1);
            }
            super::WriteClass::OperatorProjection => {
                self.projection_depth = self.projection_depth.saturating_sub(1);
            }
            super::WriteClass::BulkBlob => {
                self.blob_depth = self.blob_depth.saturating_sub(1);
            }
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

pub(super) use InnerQueue;
