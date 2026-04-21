//! Priority-aware semaphore wrapper for the scheduler.
//!
//! This module provides a [`PrioritySemaphore`] that extends tokio's semaphore
//! with priority-aware acquisition. When high-priority work requests a permit
//! and none are available, it can signal existing permit holders to yield.
//!
//! # Priority Inheritance
//!
//! True priority inheritance requires the ability to preempt running work when
//! higher priority work arrives. This implementation provides best-effort
//! priority handling via the [`yield_notify`] mechanism.
//!
//! # Limitations
//!
//! - tokio's semaphore uses FIFO ordering internally; we cannot control which
//!   waiter gets a permit when it becomes available
//! - Permits cannot be forcibly reclaimed from holders
//! - High priority work may still be briefly blocked when low priority work holds
//!   a permit, but the yield mechanism ensures it will eventually run

use std::sync::Arc;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

use super::types::JobPriority;

#[derive(Debug, Clone)]
pub struct PrioritySemaphore {
    inner: Arc<Semaphore>,
    yield_notify: Arc<Notify>,
    max_permits: usize,
}

impl PrioritySemaphore {
    pub fn new(max_permits: usize) -> Self {
        Self {
            inner: Arc::new(Semaphore::new(max_permits)),
            yield_notify: Arc::new(Notify::new()),
            max_permits,
        }
    }

    pub fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.inner.clone().try_acquire_owned().ok()
    }

    pub fn max_permits(&self) -> usize {
        self.max_permits
    }

    pub fn available_permits(&self) -> usize {
        self.inner.available_permits()
    }

    pub fn yield_waiter(&self) -> Arc<Notify> {
        self.yield_notify.clone()
    }

    pub fn notify_yield(&self) {
        self.yield_notify.notify_one();
    }

    #[allow(dead_code)]
    pub async fn acquire(&self) -> OwnedSemaphorePermit {
        self.inner.clone().acquire_owned().await.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn priority_semaphore_basic_acquire() {
        let sem = PrioritySemaphore::new(2);

        let p1 = sem.try_acquire();
        let p2 = sem.try_acquire();
        let p3 = sem.try_acquire();

        assert!(p1.is_some());
        assert!(p2.is_some());
        assert!(p3.is_none());
    }

    #[tokio::test]
    async fn priority_semaphore_yield_notify() {
        let sem = PrioritySemaphore::new(1);

        let _permit = sem.try_acquire().expect("first acquire");
        let notify = sem.yield_waiter();

        let notified = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let notified_clone = notified.clone();

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = notify.notified() => {
                    notified_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
            }
        });

        sem.notify_yield();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert!(notified.load(std::sync::atomic::Ordering::SeqCst));

        handle.await.unwrap();
    }
}
