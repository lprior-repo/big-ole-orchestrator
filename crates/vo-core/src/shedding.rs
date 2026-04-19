//! Load shedding semaphore for limiting concurrent binary spawns.
//!
//! Implements ADR-006: Backpressure and Load Shedding.
//!
//! When a ractor actor reaches a Task node and prepares to spawn a binary,
//! it must `acquire()` a permit from this semaphore. If all permits are in use,
//! the actor yields execution back to the runtime while waiting in the queue.
//!
//! # Constants
//!
//! - `MAX_CONCURRENT_BINARIES`: Maximum number of concurrent binary spawns (500)
//! - `MAX_YIELDED_ACTORS`: Threshold for ingress load shedding (5,000)

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Semaphore, TryAcquireError};

pub const MAX_CONCURRENT_BINARIES: usize = 500;
pub const MAX_YIELDED_ACTORS: usize = 5_000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SemaphoreLimitError {
    #[error("semaphore limit reached: {current_permits} permits available, {requested} requested")]
    LimitReached {
        current_permits: usize,
        requested: usize,
    },
    #[error("load shedding active: {yielded_actors} actors waiting, threshold {threshold}")]
    LoadSheddingActive {
        yielded_actors: usize,
        threshold: usize,
    },
    #[error("semaphore closed")]
    Closed,
}

impl SemaphoreLimitError {
    pub const fn is_load_shedding(&self) -> bool {
        matches!(self, SemaphoreLimitError::LoadSheddingActive { .. })
    }
}

pub struct SemaphorePermit {
    _permit: tokio::sync::OwnedSemaphorePermit,
    permits: usize,
    acquired: Arc<AtomicUsize>,
}

impl SemaphorePermit {
    fn new(
        permit: tokio::sync::OwnedSemaphorePermit,
        permits: usize,
        acquired: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            _permit: permit,
            permits,
            acquired,
        }
    }

    pub fn permits(&self) -> usize {
        self.permits
    }
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        self.acquired.fetch_sub(self.permits, Ordering::Relaxed);
    }
}

impl std::fmt::Debug for SemaphorePermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemaphorePermit")
            .field("permits", &self.permits)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct LoadSheddingSemaphore {
    semaphore: Arc<Semaphore>,
    max_permits: usize,
    acquired: Arc<AtomicUsize>,
}

impl LoadSheddingSemaphore {
    pub fn new(max_permits: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_permits)),
            max_permits,
            acquired: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn with_default_limit() -> Self {
        Self::new(MAX_CONCURRENT_BINARIES)
    }

    pub fn max_permits(&self) -> usize {
        self.max_permits
    }

    pub fn try_acquire(&self) -> Result<SemaphorePermit, SemaphoreLimitError> {
        self.try_acquire_many(1)
    }

    pub fn try_acquire_many(&self, permits: usize) -> Result<SemaphorePermit, SemaphoreLimitError> {
        match self
            .semaphore
            .clone()
            .try_acquire_many_owned(permits as u32)
        {
            Ok(permit) => {
                let acquired = self.acquired.clone();
                acquired.fetch_add(permits, Ordering::Relaxed);
                Ok(SemaphorePermit::new(permit, permits, acquired))
            }
            Err(TryAcquireError::NoPermits) => {
                let current_permits = self.available_permits();
                Err(SemaphoreLimitError::LimitReached {
                    current_permits,
                    requested: permits,
                })
            }
            Err(TryAcquireError::Closed) => Err(SemaphoreLimitError::Closed),
        }
    }

    pub async fn acquire(&self) -> Result<SemaphorePermit, SemaphoreLimitError> {
        self.acquire_many(1).await
    }

    pub async fn acquire_many(
        &self,
        permits: usize,
    ) -> Result<SemaphorePermit, SemaphoreLimitError> {
        match self
            .semaphore
            .clone()
            .acquire_many_owned(permits as u32)
            .await
        {
            Ok(permit) => {
                let acquired = self.acquired.clone();
                acquired.fetch_add(permits, Ordering::Relaxed);
                Ok(SemaphorePermit::new(permit, permits, acquired))
            }
            Err(_) => Err(SemaphoreLimitError::Closed),
        }
    }

    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    pub fn acquired_count(&self) -> usize {
        self.acquired.load(Ordering::Relaxed)
    }

    pub fn is_load_shedding_active(&self, threshold: usize) -> bool {
        self.acquired_count() >= threshold
    }

    pub fn check_load_shedding(&self) -> Result<(), SemaphoreLimitError> {
        self.check_load_shedding_threshold(MAX_YIELDED_ACTORS)
    }

    pub fn check_load_shedding_threshold(
        &self,
        threshold: usize,
    ) -> Result<(), SemaphoreLimitError> {
        if self.is_load_shedding_active(threshold) {
            Err(SemaphoreLimitError::LoadSheddingActive {
                yielded_actors: self.acquired_count(),
                threshold,
            })
        } else {
            Ok(())
        }
    }
}

impl Default for LoadSheddingSemaphore {
    fn default() -> Self {
        Self::with_default_limit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    // =========================================================================
    // 2. TIMEOUT ON FULL SEMAPHORE — async acquire with bounded wait
    // =========================================================================

    #[tokio::test]
    async fn async_acquire_times_out_when_semaphore_is_full() {
        let config = SheddingConfig {
            max_concurrent: 2,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_millis(50),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));
        let _p1 = sem.try_acquire().expect("acquire 1");
        let _p2 = sem.try_acquire().expect("acquire 2");

        let result = sem.acquire().await;
        assert!(
            matches!(result, Err(SemaphoreLimitError::Timeout { .. })),
            "expected Timeout error when semaphore is full and timeout expires, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn async_acquire_succeeds_when_permit_released_before_timeout() {
        let config = SheddingConfig {
            max_concurrent: 1,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_secs(5),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));

        let permit = sem.try_acquire().expect("acquire");
        let sem_clone = sem.clone();

        let release_handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            drop(permit);
        });

        let result = sem_clone.acquire().await;
        assert!(
            result.is_ok(),
            "acquire should succeed when permit released before timeout, got {:?}",
            result
        );

        release_handle.await.expect("release task should complete");
    }

    #[tokio::test]
    async fn async_acquire_respects_configured_timeout_duration() {
        let config = SheddingConfig {
            max_concurrent: 1,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_millis(100),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));
        let _p = sem.try_acquire().expect("acquire");

        let start = std::time::Instant::now();
        let _ = sem.acquire().await;
        let elapsed = start.elapsed();

        assert!(
            elapsed >= Duration::from_millis(80),
            "timeout should respect configured duration, elapsed: {:?}",
            elapsed
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "timeout should not take much longer than configured, elapsed: {:?}",
            elapsed
        );
    }

    // =========================================================================
    // 3. PRIORITY PREEMPTION — high-priority tasks preempt lower-priority waiters
    // =========================================================================

    #[tokio::test]
    async fn high_priority_task_preempts_low_priority_waiter() {
        let config = SheddingConfig {
            max_concurrent: 1,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_secs(5),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));

        let _low_permit = sem.try_acquire().expect("low priority acquire");

        let sem_low = sem.clone();
        let low_handle = tokio::spawn(async move {
            sem_low.acquire_with_priority(TaskPriority::Low).await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(_low_permit);
        tokio::time::sleep(Duration::from_millis(10)).await;

        let _new_permit = sem.try_acquire().expect("re-acquire");

        let sem_high = sem.clone();
        let high_handle = tokio::spawn(async move {
            sem_high.acquire_with_priority(TaskPriority::High).await
        });
        let sem_low2 = sem.clone();
        let low2_handle = tokio::spawn(async move {
            sem_low2.acquire_with_priority(TaskPriority::Low).await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(_new_permit);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let high_result = high_handle.await;
        assert!(
            high_result.is_ok() && high_result.as_ref().unwrap().is_ok(),
            "high-priority task should acquire permit, got {:?}",
            high_result
        );
        let _ = low2_handle.await;
        let _ = low_handle.await;
    }

    #[tokio::test]
    async fn critical_priority_bypasses_queue_when_load_shedding() {
        let config = SheddingConfig {
            max_concurrent: 2,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_secs(5),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));
        let _p1 = sem.try_acquire().expect("acquire 1");
        let _p2 = sem.try_acquire().expect("acquire 2");

        let result = sem.acquire_with_priority(TaskPriority::Critical).await;
        assert!(
            result.is_ok(),
            "critical priority should bypass load shedding and queue for permit, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn priority_ordering_is_respected_for_equal_priorities() {
        let sem = Arc::new(LoadSheddingSemaphore::new(1));
        let _p = sem.try_acquire().expect("exhaust permit");

        let sem_clone = sem.clone();
        let h1 = tokio::spawn(async move {
            sem_clone.acquire_with_priority(TaskPriority::Standard).await
        });
        let sem_clone = sem.clone();
        let h2 = tokio::spawn(async move {
            sem_clone.acquire_with_priority(TaskPriority::Standard).await
        });
        let sem_clone = sem.clone();
        let h3 = tokio::spawn(async move {
            sem_clone.acquire_with_priority(TaskPriority::Standard).await
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(_p);
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(h1.await.is_ok(), "first waiter should succeed");
        assert!(h2.await.is_ok(), "second waiter should succeed");
        assert!(h3.await.is_ok(), "third waiter should succeed");
    }

    // =========================================================================
    // 4. GRACEFUL REJECTION — structured rejection when shedding or shutting down
    // =========================================================================

    #[tokio::test]
    async fn graceful_rejection_when_shutting_down() {
        let sem = Arc::new(LoadSheddingSemaphore::new(10));
        sem.shutdown();

        let result = sem.try_acquire();
        assert!(
            matches!(result, Err(SemaphoreLimitError::ShuttingDown)),
            "acquire after shutdown should return ShuttingDown error, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn graceful_rejection_async_acquire_when_shutting_down() {
        let config = SheddingConfig {
            max_concurrent: 5,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_secs(5),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));
        sem.shutdown();

        let result = sem.acquire().await;
        assert!(
            matches!(result, Err(SemaphoreLimitError::ShuttingDown)),
            "async acquire after shutdown should return ShuttingDown, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn existing_permits_remain_valid_after_shutdown() {
        let sem = Arc::new(LoadSheddingSemaphore::new(5));

        let permit = sem.try_acquire().expect("should acquire before shutdown");
        assert_eq!(sem.acquired_count(), 1);

        sem.shutdown();

        assert_eq!(permit.permits(), 1);
        drop(permit);
        assert_eq!(sem.acquired_count(), 0);
    }

    #[tokio::test]
    async fn shutdown_is_irreversible() {
        let sem = LoadSheddingSemaphore::new(5);

        sem.shutdown();
        assert!(sem.try_acquire().is_err());

        sem.shutdown();
        assert!(sem.try_acquire().is_err());
    }

    #[tokio::test]
    async fn acquire_with_priority_rejected_gracefully_when_shutting_down() {
        let config = SheddingConfig {
            max_concurrent: 5,
            max_yielded_actors: 100,
            acquire_timeout: Duration::from_secs(1),
        };
        let sem = Arc::new(LoadSheddingSemaphore::with_config(config));
        sem.shutdown();

        let result = sem.acquire_with_priority(TaskPriority::Critical).await;
        assert!(
            matches!(result, Err(SemaphoreLimitError::ShuttingDown)),
            "critical priority should still be rejected during shutdown, got {:?}",
            result
        );
    }

    // =========================================================================
    // Edge cases and invariants
    // =========================================================================

    #[tokio::test]
    async fn acquired_count_never_exceeds_max_permits() {
        let max = 10;
        let sem = LoadSheddingSemaphore::new(max);

        let permits: Vec<_> = (0..max)
            .map(|_| sem.try_acquire().expect("should acquire"))
            .collect();

        assert_eq!(sem.acquired_count(), max);
        assert_eq!(sem.available_permits(), 0);
        assert!(sem.try_acquire().is_err());
        assert_eq!(sem.acquired_count(), max);

        drop(permits);
        assert_eq!(sem.acquired_count(), 0);
    }

    #[tokio::test]
    async fn semaphore_config_default_values_match_adr006() {
        let config = SheddingConfig::default();
        assert_eq!(config.max_concurrent, MAX_CONCURRENT_BINARIES);
        assert_eq!(config.max_yielded_actors, MAX_YIELDED_ACTORS);
        assert_eq!(config.acquire_timeout, Duration::from_secs(30));
    }
}
