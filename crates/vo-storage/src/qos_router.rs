//! QoS-aware router for dispatching items to separate per-class channels.
//!
//! This module provides the `QosRouter` which routes items to isolated bounded
//! channels based on their `WriteClass`, ensuring that control-plane writes
//! are never blocked by projection queue fullness.

use std::collections::VecDeque;

pub use crate::append::WriteClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapacityError {
    QueueFull {
        class: WriteClass,
        depth: usize,
        capacity: usize,
    },
}

impl std::fmt::Display for CapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull {
                class,
                depth,
                capacity,
            } => write!(f, "queue full for {class:?}: {depth}/{capacity}"),
        }
    }
}

impl std::error::Error for CapacityError {}

trait Classifiable {
    fn write_class(&self) -> WriteClass;
}

struct InnerChannel<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T> InnerChannel<T> {
    const fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::new(),
            capacity,
        }
    }

    fn push(&mut self, item: T) -> Result<(), T> {
        if self.items.len() >= self.capacity {
            return Err(item);
        }
        self.items.push_back(item);
        Ok(())
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

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    const fn capacity(&self) -> usize {
        self.capacity
    }
}

pub struct QosRouter<T> {
    control_plane: InnerChannel<T>,
    projection: InnerChannel<T>,
    blob: InnerChannel<T>,
}

#[derive(Debug, Clone)]
pub struct QosRouterConfig {
    pub control_plane_capacity: usize,
    pub projection_capacity: usize,
    pub blob_capacity: usize,
}

impl Default for QosRouterConfig {
    fn default() -> Self {
        Self {
            control_plane_capacity: 1024,
            projection_capacity: 512,
            blob_capacity: 256,
        }
    }
}

impl<T> QosRouter<T> {
    #[must_use]
    pub fn new(config: QosRouterConfig) -> Self {
        Self {
            control_plane: InnerChannel::new(config.control_plane_capacity),
            projection: InnerChannel::new(config.projection_capacity),
            blob: InnerChannel::new(config.blob_capacity),
        }
    }

    #[must_use]
    pub fn with_capacity(
        control_plane_capacity: usize,
        projection_capacity: usize,
        blob_capacity: usize,
    ) -> Self {
        Self {
            control_plane: InnerChannel::new(control_plane_capacity),
            projection: InnerChannel::new(projection_capacity),
            blob: InnerChannel::new(blob_capacity),
        }
    }

    fn channel_for_class(&mut self, class: WriteClass) -> &mut InnerChannel<T> {
        match class {
            WriteClass::CriticalControlPlane => &mut self.control_plane,
            WriteClass::OperatorProjection => &mut self.projection,
            WriteClass::BulkBlob => &mut self.blob,
        }
    }

    pub fn enqueue(&mut self, item: T) -> Result<(), CapacityError>
    where
        T: Classifiable,
    {
        let class = item.write_class();
        let channel = self.channel_for_class(class);
        channel.push(item).map_err(|_| CapacityError::QueueFull {
            class,
            depth: channel.len(),
            capacity: channel.capacity(),
        })
    }

    pub fn enqueue_control_plane(&mut self, item: T) -> Result<(), CapacityError>
    where
        T: Clone,
    {
        self.control_plane
            .push(item)
            .map_err(|_| CapacityError::QueueFull {
                class: WriteClass::CriticalControlPlane,
                depth: self.control_plane.len(),
                capacity: self.control_plane.capacity(),
            })
    }

    pub fn enqueue_projection(&mut self, item: T) -> Result<(), CapacityError>
    where
        T: Clone,
    {
        self.projection
            .push(item)
            .map_err(|_| CapacityError::QueueFull {
                class: WriteClass::OperatorProjection,
                depth: self.projection.len(),
                capacity: self.projection.capacity(),
            })
    }

    pub fn enqueue_blob(&mut self, item: T) -> Result<(), CapacityError>
    where
        T: Clone,
    {
        self.blob.push(item).map_err(|_| CapacityError::QueueFull {
            class: WriteClass::BulkBlob,
            depth: self.blob.len(),
            capacity: self.blob.capacity(),
        })
    }

    pub fn dequeue(&mut self, class: WriteClass) -> Option<T> {
        self.channel_for_class(class).pop()
    }

    pub fn dequeue_control_plane(&mut self) -> Option<T> {
        self.control_plane.pop()
    }

    pub fn dequeue_projection(&mut self) -> Option<T> {
        self.projection.pop()
    }

    pub fn dequeue_blob(&mut self) -> Option<T> {
        self.blob.pop()
    }

    #[must_use]
    pub fn depth(&self, class: WriteClass) -> usize {
        match class {
            WriteClass::CriticalControlPlane => self.control_plane.len(),
            WriteClass::OperatorProjection => self.projection.len(),
            WriteClass::BulkBlob => self.blob.len(),
        }
    }

    #[must_use]
    pub fn capacity(&self, class: WriteClass) -> usize {
        match class {
            WriteClass::CriticalControlPlane => self.control_plane.capacity(),
            WriteClass::OperatorProjection => self.projection.capacity(),
            WriteClass::BulkBlob => self.blob.capacity(),
        }
    }

    #[must_use]
    pub fn is_full(&self, class: WriteClass) -> bool {
        match class {
            WriteClass::CriticalControlPlane => self.control_plane.is_full(),
            WriteClass::OperatorProjection => self.projection.is_full(),
            WriteClass::BulkBlob => self.blob.is_full(),
        }
    }

    #[must_use]
    pub fn remaining_capacity(&self, class: WriteClass) -> usize {
        self.capacity(class).saturating_sub(self.depth(class))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.control_plane.is_empty() && self.projection.is_empty() && self.blob.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.control_plane.len() + self.projection.len() + self.blob.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestItem {
        class: WriteClass,
    }

    impl Classifiable for TestItem {
        fn write_class(&self) -> WriteClass {
            self.class
        }
    }

    #[test]
    fn qos_router_new_with_default_config() {
        let router: QosRouter<TestItem> = QosRouter::new(QosRouterConfig::default());
        assert_eq!(router.capacity(WriteClass::CriticalControlPlane), 1024);
        assert_eq!(router.capacity(WriteClass::OperatorProjection), 512);
        assert_eq!(router.capacity(WriteClass::BulkBlob), 256);
    }

    #[test]
    fn qos_router_with_capacity() {
        let router: QosRouter<TestItem> = QosRouter::with_capacity(100, 50, 25);
        assert_eq!(router.capacity(WriteClass::CriticalControlPlane), 100);
        assert_eq!(router.capacity(WriteClass::OperatorProjection), 50);
        assert_eq!(router.capacity(WriteClass::BulkBlob), 25);
    }

    #[test]
    fn qos_router_enqueue_control_plane_item() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(10, 10, 10);
        let item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        assert!(router.enqueue(item).is_ok());
        assert_eq!(router.depth(WriteClass::CriticalControlPlane), 1);
    }

    #[test]
    fn qos_router_enqueue_projection_item() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(10, 10, 10);
        let item = TestItem {
            class: WriteClass::OperatorProjection,
        };
        assert!(router.enqueue(item).is_ok());
        assert_eq!(router.depth(WriteClass::OperatorProjection), 1);
    }

    #[test]
    fn qos_router_enqueue_blob_item() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(10, 10, 10);
        let item = TestItem {
            class: WriteClass::BulkBlob,
        };
        assert!(router.enqueue(item).is_ok());
        assert_eq!(router.depth(WriteClass::BulkBlob), 1);
    }

    #[test]
    fn qos_router_returns_capacity_error_when_queue_full() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(1, 1, 1);
        let item = TestItem {
            class: WriteClass::OperatorProjection,
        };
        assert!(router.enqueue(item.clone()).is_ok());
        let err = router.enqueue(item).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::QueueFull {
                class: WriteClass::OperatorProjection,
                ..
            }
        ));
    }

    #[test]
    fn qos_router_dequeue_returns_items_in_fifo_order() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(10, 10, 10);
        let item1 = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        let item2 = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        router.enqueue(item1.clone()).unwrap();
        router.enqueue(item2.clone()).unwrap();
        assert_eq!(
            router.dequeue(WriteClass::CriticalControlPlane),
            Some(item1)
        );
        assert_eq!(
            router.dequeue(WriteClass::CriticalControlPlane),
            Some(item2)
        );
        assert_eq!(router.dequeue(WriteClass::CriticalControlPlane), None);
    }

    #[test]
    fn qos_router_isolated_queues_do_not_interfere() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(1, 1, 1);
        let control_item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        let projection_item = TestItem {
            class: WriteClass::OperatorProjection,
        };
        assert!(router.enqueue(control_item).is_ok());
        assert!(router.enqueue(projection_item).is_ok());
        assert_eq!(router.depth(WriteClass::CriticalControlPlane), 1);
        assert_eq!(router.depth(WriteClass::OperatorProjection), 1);
        assert_eq!(router.depth(WriteClass::BulkBlob), 0);
    }

    #[test]
    fn qos_router_projection_full_does_not_block_control_plane() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(2, 1, 1);
        let projection_item = TestItem {
            class: WriteClass::OperatorProjection,
        };
        router.enqueue(projection_item.clone()).unwrap();
        let err = router.enqueue(projection_item).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::QueueFull {
                class: WriteClass::OperatorProjection,
                ..
            }
        ));
        let control_item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        assert!(router.enqueue(control_item).is_ok());
        assert_eq!(router.depth(WriteClass::CriticalControlPlane), 1);
    }

    #[test]
    fn qos_router_blob_full_does_not_block_control_plane() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(2, 2, 1);
        let blob_item = TestItem {
            class: WriteClass::BulkBlob,
        };
        router.enqueue(blob_item.clone()).unwrap();
        let err = router.enqueue(blob_item).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::QueueFull {
                class: WriteClass::BulkBlob,
                ..
            }
        ));
        let control_item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        assert!(router.enqueue(control_item).is_ok());
    }

    #[test]
    fn qos_router_remaining_capacity() {
        let router: QosRouter<TestItem> = QosRouter::with_capacity(100, 50, 25);
        assert_eq!(
            router.remaining_capacity(WriteClass::CriticalControlPlane),
            100
        );
        assert_eq!(
            router.remaining_capacity(WriteClass::OperatorProjection),
            50
        );
        assert_eq!(router.remaining_capacity(WriteClass::BulkBlob), 25);
    }

    #[test]
    fn qos_router_is_empty() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(10, 10, 10);
        assert!(router.is_empty());
        let item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        router.enqueue(item).unwrap();
        assert!(!router.is_empty());
    }

    #[test]
    fn qos_router_total_len() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(10, 10, 10);
        assert_eq!(router.len(), 0);
        router
            .enqueue(TestItem {
                class: WriteClass::CriticalControlPlane,
            })
            .unwrap();
        router
            .enqueue(TestItem {
                class: WriteClass::OperatorProjection,
            })
            .unwrap();
        router
            .enqueue(TestItem {
                class: WriteClass::BulkBlob,
            })
            .unwrap();
        assert_eq!(router.len(), 3);
    }

    #[test]
    fn qos_router_is_full() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(1, 1, 1);
        assert!(!router.is_full(WriteClass::CriticalControlPlane));
        router
            .enqueue(TestItem {
                class: WriteClass::CriticalControlPlane,
            })
            .unwrap();
        assert!(router.is_full(WriteClass::CriticalControlPlane));
        assert!(!router.is_full(WriteClass::OperatorProjection));
    }

    #[test]
    fn qos_router_enqueue_specific_channel_methods() {
        let mut router: QosRouter<String> = QosRouter::with_capacity(10, 10, 10);
        assert!(router.enqueue_control_plane("cp1".to_string()).is_ok());
        assert!(router.enqueue_projection("proj1".to_string()).is_ok());
        assert!(router.enqueue_blob("blob1".to_string()).is_ok());
        assert_eq!(router.dequeue_control_plane(), Some("cp1".to_string()));
        assert_eq!(router.dequeue_projection(), Some("proj1".to_string()));
        assert_eq!(router.dequeue_blob(), Some("blob1".to_string()));
    }

    #[test]
    fn qos_router_capacity_error_preserves_item_for_retry() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(1, 1, 1);
        let item = TestItem {
            class: WriteClass::OperatorProjection,
        };
        assert!(router.enqueue(item.clone()).is_ok());
        let err = router.enqueue(item.clone()).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::QueueFull {
                class: WriteClass::OperatorProjection,
                depth: 1,
                capacity: 1,
            }
        ));
        let dequeued = router.dequeue(WriteClass::OperatorProjection);
        assert!(dequeued.is_some());
        assert!(router.enqueue(item).is_ok());
    }

    #[test]
    fn qos_router_control_plane_never_blocks_on_projection_full() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(1, 1, 1);
        let projection_item = TestItem {
            class: WriteClass::OperatorProjection,
        };
        assert!(router.enqueue(projection_item.clone()).is_ok());
        let err = router.enqueue(projection_item).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::QueueFull {
                class: WriteClass::OperatorProjection,
                ..
            }
        ));
        let control_item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        let result = router.enqueue(control_item);
        assert!(
            result.is_ok(),
            "Control plane should succeed even when projection is full"
        );
        assert_eq!(router.depth(WriteClass::CriticalControlPlane), 1);
    }

    #[test]
    fn qos_router_invariant_control_plane_never_waits_on_projection_capacity() {
        let mut router: QosRouter<TestItem> = QosRouter::with_capacity(1, 1, 1);
        for _ in 0..100 {
            let projection_item = TestItem {
                class: WriteClass::OperatorProjection,
            };
            if router.enqueue(projection_item).is_err() {
                break;
            }
        }
        assert!(router.is_full(WriteClass::OperatorProjection));
        let control_item = TestItem {
            class: WriteClass::CriticalControlPlane,
        };
        let result = router.enqueue(control_item);
        assert!(result.is_ok());
    }
}
