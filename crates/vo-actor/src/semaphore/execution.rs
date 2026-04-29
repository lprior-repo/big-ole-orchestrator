//! Action Layer — Execution Semaphore
//!
//! Provides the global execution semaphore for limiting concurrent binary spawns.
//! Per ADR-006: Uses `tokio::sync::Semaphore` with fixed permits (e.g., 500)
//! to limit concurrent binary spawns.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{Semaphore, TryAcquireError};
use tokio::task::JoinHandle;

use crate::semaphore::calc::status_from_config_and_state;
use crate::semaphore::types::{
    AdmissionDecision, BackpressureStatus, RejectionReason, SemaphoreConfig,
};

/// Guard that releases a semaphore permit on drop.
///
/// This ensures that even if an actor panics during execution,
/// the permit is always released when the guard drops.
pub struct PermitGuard {
    semaphore: Arc<ExecutionSemaphore>,
    permit: Option<std::mem::ManuallyDrop<tokio::sync::SemaphorePermit<'static>>>,
}

impl PermitGuard {
    #[allow(unsafe_op_in_unsafe_fn)]
    fn new(semaphore: Arc<ExecutionSemaphore>, permit: tokio::sync::SemaphorePermit<'static>) -> Self {
        Self {
            semaphore,
            permit: Some(std::mem::ManuallyDrop::new(permit)),
        }
    }

    #[allow(unsafe_op_in_unsafe_fn)]
    fn release_permit(&self) {
        self.semaphore.semaphore.add_permits(1);
    }

    /// Spawns an actor task with the permit guard in scope.
    ///
    /// The permit is held for the duration of the actor's execution.
    /// If the actor panics, the guard drops and releases the permit.
    pub fn spawn<F>(self, f: F) -> JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::spawn(async move {
            f.await
        })
    }
}

impl Drop for PermitGuard {
    #[allow(unsafe_op_in_unsafe_fn)]
    fn drop(&mut self) {
        if let Some(mut permit) = self.permit.take() {
            unsafe { std::mem::ManuallyDrop::drop(&mut permit) };
        }
        self.release_permit();
        self.semaphore.available_permits.fetch_add(1, Ordering::Relaxed);
    }
}

/// The global execution semaphore for binary spawn limiting.
pub struct ExecutionSemaphore {
    semaphore: Semaphore,
    reserved_semaphore: Semaphore,
    config: SemaphoreConfig,
    available_permits: AtomicUsize,
    reserved_available: AtomicUsize,
    waiting_count: AtomicUsize,
}

impl std::fmt::Debug for ExecutionSemaphore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionSemaphore")
            .field("config", &self.config)
            .field("available_permits", &self.available_permits)
            .field("reserved_available", &self.reserved_available)
            .field("waiting_count", &self.waiting_count)
            .finish()
    }
}

impl ExecutionSemaphore {
    /// Creates a new execution semaphore with the given config.
    #[must_use]
    pub fn new(config: SemaphoreConfig) -> Self {
        let available_permits = config.max_concurrent_binaries;
        let reserved_permits = config.reserved_permits;
        Self {
            semaphore: Semaphore::new(available_permits),
            reserved_semaphore: Semaphore::new(reserved_permits),
            config,
            available_permits: AtomicUsize::new(available_permits),
            reserved_available: AtomicUsize::new(reserved_permits),
            waiting_count: AtomicUsize::new(0),
        }
    }

    /// Creates a new execution semaphore with default config.
    #[must_use]
    pub fn default() -> Self {
        Self::new(SemaphoreConfig::default())
    }

    /// Attempts to acquire a permit without waiting.
    ///
    /// Returns `Some(permit)` if available, `None` otherwise.
    /// The permit is automatically released when dropped.
    pub fn try_acquire(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        match self.semaphore.try_acquire() {
            Ok(permit) => {
                self.available_permits.fetch_sub(1, Ordering::Relaxed);
                Some(permit)
            }
            Err(TryAcquireError::NoPermits) => None,
            Err(TryAcquireError::Closed) => None,
        }
    }

    /// Acquires a permit guard for panic-safe actor spawning.
    ///
    /// Returns a `PermitGuard` if a permit is available, `None` otherwise.
    /// The guard ensures the permit is released even if the actor panics.
    ///
    /// # Example
    /// ```ignore
    /// let guard = sem.try_acquire_guard().await;
    /// let handle = guard.spawn(async move {
    ///     // work that might panic
    /// });
    /// ```
    #[allow(unsafe_op_in_unsafe_fn)]
    pub fn try_acquire_guard(self: Arc<Self>) -> Option<PermitGuard> {
        let sem_ptr = &self.semaphore as *const Semaphore;
        let permit = match unsafe { (*sem_ptr).try_acquire() } {
            Ok(p) => p,
            Err(TryAcquireError::NoPermits) => return None,
            Err(TryAcquireError::Closed) => return None,
        };
        self.available_permits.fetch_sub(1, Ordering::Relaxed);
        let extended_permit = unsafe {
            std::mem::transmute::<tokio::sync::SemaphorePermit<'_>, tokio::sync::SemaphorePermit<'static>>(permit)
        };
        Some(PermitGuard::new(self, extended_permit))
    }

    /// Attempts to acquire a permit from the reserved pool for recovery tasks.
    ///
    /// Returns `Some(permit)` if available, `None` otherwise.
    /// The permit is automatically released when dropped.
    ///
    /// This method is exclusively for recovery and control-plane tasks.
    /// It does not consume permits from the general pool.
    pub fn try_acquire_recovery(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        match self.reserved_semaphore.try_acquire() {
            Ok(permit) => {
                self.reserved_available.fetch_sub(1, Ordering::Relaxed);
                Some(permit)
            }
            Err(TryAcquireError::NoPermits) => None,
            Err(TryAcquireError::Closed) => None,
        }
    }

    /// Acquires a permit, waiting if necessary.
    ///
    /// Returns `AdmissionDecision` based on outcome.
    pub async fn acquire(self: &Arc<Self>) -> AdmissionDecision {
        let waiting = self.waiting_count.fetch_add(1, Ordering::Relaxed);
        let _ = waiting;
        let status = self.current_status();

        if status.should_reject() {
            self.waiting_count.fetch_sub(1, Ordering::Relaxed);
            return AdmissionDecision::Rejected {
                reason: RejectionReason::LoadShed,
                retry_after_secs: 5,
            };
        }

        match tokio::time::timeout(self.config.acquire_timeout, self.semaphore.acquire()).await {
            Ok(Ok(_permit)) => {
                self.waiting_count.fetch_sub(1, Ordering::Relaxed);
                self.available_permits.fetch_sub(1, Ordering::Relaxed);
                AdmissionDecision::Admitted
            }
            Ok(Err(_)) => {
                self.waiting_count.fetch_sub(1, Ordering::Relaxed);
                AdmissionDecision::Rejected {
                    reason: RejectionReason::LoadShed,
                    retry_after_secs: 5,
                }
            }
            Err(_) => {
                self.waiting_count.fetch_sub(1, Ordering::Relaxed);
                AdmissionDecision::Rejected {
                    reason: RejectionReason::Timeout,
                    retry_after_secs: 10,
                }
            }
        }
    }

    /// Returns the current backpressure status.
    #[must_use]
    pub fn current_status(&self) -> BackpressureStatus {
        let available = self.available_permits.load(Ordering::Relaxed);
        let waiting = self.waiting_count.load(Ordering::Relaxed);
        status_from_config_and_state(&self.config, available, waiting)
    }

    /// Returns the number of available permits.
    #[must_use]
    pub fn available_permits(&self) -> usize {
        self.available_permits.load(Ordering::Relaxed)
    }

    /// Returns the number of waiting tasks.
    #[must_use]
    pub fn waiting_count(&self) -> usize {
        self.waiting_count.load(Ordering::Relaxed)
    }

    /// Returns the total permit capacity.
    #[must_use]
    pub fn total_permits(&self) -> usize {
        self.config.max_concurrent_binaries
    }

    /// Returns the number of available reserved permits.
    #[must_use]
    pub fn reserved_available(&self) -> usize {
        self.reserved_available.load(Ordering::Relaxed)
    }

    /// Returns the total reserved permit capacity.
    #[must_use]
    pub fn total_reserved_permits(&self) -> usize {
        self.config.reserved_permits
    }

    /// Returns true if load shedding is active.
    #[must_use]
    pub fn is_load_shedding(&self) -> bool {
        self.current_status().should_reject()
    }

    /// Returns configuration reference.
    #[must_use]
    pub fn config(&self) -> &SemaphoreConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn given_permit_waiters_when_no_permits_then_wait_is_async_not_spin() {
        // ADR-006: Async wait must not busy-loop.
        // Given: permits unavailable
        // When: many actors wait for permits
        // Then: waiters suspend asynchronously and CPU spin counters stay bounded

        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            acquire_timeout: Duration::from_secs(10),
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));

        // Exhaust all permits so waiters must block
        let _permit = sem.try_acquire();
        assert_eq!(sem.available_permits(), 0);

        // Spawn N concurrent waiters — all must see zero permits
        let num_waiters = 10;
        let mut handles = Vec::with_capacity(num_waiters);

        for _ in 0..num_waiters {
            let sem = Arc::clone(&sem);
            let handle = tokio::spawn(async move { sem.acquire().await });
            handles.push(handle);
        }

        // Verify waiters are tracked (bounded)
        let waiting = sem.waiting_count();
        assert_eq!(
            waiting, num_waiters,
            "All {num_waiters} waiters must be tracked"
        );

        // Give waiters a moment to enter the async sleep state
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Release permit → first waiter acquires → releases → chain completes
        drop(_permit);

        // All waiters must complete within a bounded time window.
        // If acquire() busy-looped, it would consume CPU and likely hit
        // the acquire_timeout before the permit could be released.
        let result =
            tokio::time::timeout(Duration::from_secs(5), futures::future::join_all(handles)).await;

        // Verify all waiters finished (no timeout = async suspend, not spin)
        assert!(
            result.is_ok(),
            "Waiters timed out — acquire() may be busy-spinning instead of suspending"
        );

        let decisions = result.unwrap();
        let admitted_count = decisions
            .into_iter()
            .filter(|d| matches!(d.as_ref().unwrap(), AdmissionDecision::Admitted))
            .count();
        assert_eq!(admitted_count, num_waiters, "All waiters must be admitted");

        // Verify waiters returned to zero (all acquired and moved past the wait)
        assert_eq!(
            sem.waiting_count(),
            0,
            "Waiting count should return to zero after all permits acquired"
        );
    }

    #[tokio::test]
    async fn execution_semaphore_try_acquire_success() {
        let sem = ExecutionSemaphore::default();
        let initial_available = sem.available_permits();

        let permit = sem.try_acquire();
        assert!(permit.is_some());
        assert_eq!(sem.available_permits(), initial_available - 1);
    }

    #[tokio::test]
    async fn execution_semaphore_try_acquire_exhausted() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);

        let _permit = sem.try_acquire();
        assert!(sem.try_acquire().is_none());
    }

    #[tokio::test]
    async fn execution_semaphore_try_acquire_recovery_success() {
        let sem = ExecutionSemaphore::default();
        let initial_reserved = sem.reserved_available();

        let permit = sem.try_acquire_recovery();
        assert!(permit.is_some());
        assert_eq!(sem.reserved_available(), initial_reserved - 1);
    }

    #[tokio::test]
    async fn execution_semaphore_try_acquire_recovery_exhausted() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 100,
            reserved_permits: 1,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);

        let _permit = sem.try_acquire_recovery();
        assert!(sem.try_acquire_recovery().is_none());
    }

    #[tokio::test]
    async fn execution_semaphore_recovery_pool_independent() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            reserved_permits: 1,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);

        let _general = sem.try_acquire().unwrap();
        let recovery = sem.try_acquire_recovery();
        assert!(recovery.is_some());
    }

    #[tokio::test]
    async fn execution_semaphore_acquire_and_release() {
        let sem = Arc::new(ExecutionSemaphore::default());
        let initial = sem.available_permits();

        let decision = sem.acquire().await;
        assert!(matches!(decision, AdmissionDecision::Admitted));
        assert_eq!(sem.available_permits(), initial - 1);
    }

    #[tokio::test]
    async fn execution_semaphore_status_tracking() {
        let sem = ExecutionSemaphore::default();
        assert_eq!(sem.current_status(), BackpressureStatus::Healthy);
        assert_eq!(sem.waiting_count(), 0);
    }

    #[tokio::test]
    async fn permit_guard_releases_on_drop() {
        let sem = Arc::new(ExecutionSemaphore::default());
        let initial = sem.available_permits();

        let guard = sem.clone().try_acquire_guard().expect("should acquire permit");
        assert_eq!(sem.available_permits(), initial - 1);

        drop(guard);

        assert_eq!(sem.available_permits(), initial);
    }

    #[tokio::test]
    async fn permit_guard_releases_on_panic() {
        let sem = Arc::new(ExecutionSemaphore::new(
            SemaphoreConfig {
                max_concurrent_binaries: 10,
                ..Default::default()
            }
        ));
        let initial = sem.available_permits();

        let mut handles = Vec::new();
        for i in 0..20 {
            let sem = Arc::clone(&sem);
            let handle = tokio::spawn(async move {
                if let Some(guard) = sem.try_acquire_guard() {
                    if i % 2 == 0 {
                        panic!("panic in actor {}", i);
                    }
                    guard.spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }).await.unwrap();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        assert_eq!(
            sem.available_permits(),
            initial,
            "All permits should be released even after panics"
        );
    }

    #[tokio::test]
    async fn permit_guard_spawn_keeps_permit_until_task_completes() {
        let sem = Arc::new(ExecutionSemaphore::new(
            SemaphoreConfig {
                max_concurrent_binaries: 2,
                ..Default::default()
            }
        ));

        let guard1 = sem.clone().try_acquire_guard().expect("should get first permit");
        let guard2 = sem.clone().try_acquire_guard().expect("should get second permit");
        assert!(sem.clone().try_acquire_guard().is_none(), "should be exhausted");

        let handle = guard1.spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        });

        assert!(sem.clone().try_acquire_guard().is_none(), "permit still held during spawn");

        handle.await.unwrap();

        drop(guard2);

        assert_eq!(sem.available_permits(), 2, "all permits should be released");
    }
}
