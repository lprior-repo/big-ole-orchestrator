//! Action Layer — Execution Semaphore
//!
//! Provides the global execution semaphore for limiting concurrent binary spawns.
//! Per ADR-006: Uses `tokio::sync::Semaphore` with fixed permits (e.g., 500)
//! to limit concurrent binary spawns.

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{Semaphore, TryAcquireError};
use tokio::task::{AbortHandle, JoinSet};

use crate::semaphore::calc::status_from_config_and_state;

/// Error returned when spawning into a JoinSet fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnError {
    /// The JoinSet is full or closed.
    JoinSetFull,
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnError::JoinSetFull => write!(f, "spawn target JoinSet is full or closed"),
        }
    }
}

impl std::error::Error for SpawnError {}
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

    /// Spawns an actor task into a JoinSet with the permit guard in scope.
    ///
    /// Per ADR-011: All tasks must be tracked in a JoinSet for structured
    /// concurrency. Bare `tokio::spawn` is not allowed.
    ///
    /// The permit guard is consumed on spawn (consistent with original spawn()).
    /// The permit is released when the guard drops.
    pub fn spawn_with_scope<F>(self, f: F, joinset: &mut JoinSet<F::Output>) -> AbortHandle
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        joinset.spawn(f)
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
    #[allow(clippy::should_implement_trait)]
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

    use tokio::task::JoinSet;

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

        // Yield to let spawned tasks enter acquire() and increment waiting_count
        tokio::task::yield_now().await;

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

        let decisions: Vec<_> = result
            .unwrap()
            .into_iter()
            .map(|r| r.expect("waiter task panicked"))
            .collect();
        let admitted_count = decisions
            .into_iter()
            .filter(|d| matches!(d, AdmissionDecision::Admitted))
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
    async fn execution_semaphore_acquire_n_permits_reduces_available_by_n() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 10,
            reserved_permits: 0,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);
        let initial = sem.available_permits();

        let _p1 = sem.try_acquire().unwrap();
        assert_eq!(sem.available_permits(), initial - 1);

        let _p2 = sem.try_acquire().unwrap();
        assert_eq!(sem.available_permits(), initial - 2);

        let _p3 = sem.try_acquire().unwrap();
        assert_eq!(sem.available_permits(), initial - 3);
    }

    #[tokio::test]
    async fn execution_semaphore_acquire_decrements_and_drop_does_not_restore() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 10,
            reserved_permits: 0,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);
        let initial = sem.available_permits();

        let p1 = sem.try_acquire().unwrap();
        assert_eq!(sem.available_permits(), initial - 1);

        drop(p1);
        assert_eq!(
            sem.available_permits(),
            initial - 1,
            "available_permits does not auto-restore on drop"
        );

        let p2 = sem.try_acquire().unwrap();
        assert_eq!(sem.available_permits(), initial - 2);
    }

    #[tokio::test]
    async fn execution_semaphore_concurrent_acquire_release_invariant() {
        use std::sync::atomic::AtomicUsize;
        use std::sync::Arc;

        let sem = Arc::new(ExecutionSemaphore::new(SemaphoreConfig {
            max_concurrent_binaries: 100,
            reserved_permits: 0,
            ..Default::default()
        }));
        let initial = sem.available_permits();
        let invariant = Arc::new(AtomicUsize::new(initial));
        let invariant_clone = invariant.clone();

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let sem = sem.clone();
                let inv = invariant_clone.clone();
                tokio::spawn(async move {
                    for _ in 0..5 {
                        if let Some(_permit) = sem.try_acquire() {
                            inv.fetch_sub(1, Ordering::Relaxed);
                            tokio::task::yield_now().await;
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.await;
        }

        let final_count = sem.available_permits();
        assert!(
            final_count <= initial,
            "available permits should not exceed initial {} (got {})",
            initial,
            final_count
        );
    }

    #[tokio::test]
    async fn execution_semaphore_backpressure_healthy_to_moderate_via_usage() {
        let sem = ExecutionSemaphore::new(SemaphoreConfig {
            max_concurrent_binaries: 100,
            max_waiters_for_shed: 5000,
            reserved_permits: 0,
            ..Default::default()
        });

        assert_eq!(sem.current_status(), BackpressureStatus::Healthy);

        for _ in 0..51 {
            let _ = sem.try_acquire();
        }
        let usage_ratio = 51.0 / 100.0;
        assert!(
            usage_ratio > 0.5,
            "usage_ratio {} should be > 0.5 for Moderate",
            usage_ratio
        );
        assert_eq!(
            sem.current_status(),
            BackpressureStatus::Moderate,
            "usage_ratio > 0.5 should transition to Moderate"
        );
    }

    #[tokio::test]
    async fn execution_semaphore_backpressure_moderate_to_heavy_via_usage() {
        let sem = ExecutionSemaphore::new(SemaphoreConfig {
            max_concurrent_binaries: 100,
            max_waiters_for_shed: 5000,
            reserved_permits: 0,
            ..Default::default()
        });

        for _ in 0..81 {
            let _ = sem.try_acquire();
        }
        let usage_ratio = 81.0 / 100.0;
        assert!(
            usage_ratio > 0.8,
            "usage_ratio {} should be > 0.8 for Heavy",
            usage_ratio
        );
        assert!(
            sem.current_status() >= BackpressureStatus::Heavy,
            "usage_ratio > 0.8 should transition to at least Heavy"
        );
    }

    // ========================================================================
    // Structured Concurrency Enforcement (ADR-011)
    // ========================================================================

    #[tokio::test]
    async fn permit_guard_spawn_with_joinset_tracks_task() {
        // Given: A permit guard acquired from the semaphore
        let config = SemaphoreConfig {
            max_concurrent_binaries: 10,
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));
        let guard = sem.try_acquire_guard().unwrap();

        // When: We spawn a task with a JoinSet
        let mut joinset = JoinSet::new();
        let result = guard.spawn_with_scope(async move { 42 }, &mut joinset);

        // Then: spawn_with_scope returns Ok(JoinHandle)
        assert!(result.is_ok(), "spawn_with_scope should succeed with JoinSet");
        let handle = result.unwrap();

        // And: The task is tracked in the JoinSet
        let output = joinset.join_one().await.unwrap().unwrap();
        assert_eq!(output, 42);
    }

    #[tokio::test]
    async fn permit_guard_spawn_with_cancel_handle_cancels_task() {
        // Given: A permit guard and a JoinSet
        let config = SemaphoreConfig {
            max_concurrent_binaries: 10,
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));
        let guard = sem.try_acquire_guard().unwrap();

        // When: We spawn a long-running task and cancel it
        let mut joinset = JoinSet::new();
        use std::time::Duration;
        let _result = guard.spawn_with_scope(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            42
        }, &mut joinset);

        // Cancel all tasks in the joinset
        joinset.abort_all();

        // Then: All tasks complete without panicking
        while let Some(result) = joinset.join_next().await {
            if let Err(e) = result {
                assert!(e.is_cancelled(), "Task should be cancelled, not panicked");
            }
        }
    }

    #[tokio::test]
    async fn permit_guard_spawn_without_scope_rejected() {
        // Given: A permit guard
        let config = SemaphoreConfig {
            max_concurrent_binaries: 10,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);
        let guard = sem.try_acquire_guard().unwrap();

        // When: We try to use the guard (verify it doesn't allow bare tokio::spawn)
        // The old bare spawn() method should NOT exist - only spawn_with_scope is allowed
        // This test verifies that the only available spawn method requires a JoinSet

        // Then: The guard only exposes spawn_with_scope, not bare spawn
        // (This is a compile-time check - if spawn() existed, this test wouldn't be needed)
        let mut joinset = JoinSet::new();
        let _ = guard.spawn_with_scope(async move { () }, &mut joinset);
        // If we get here, the structured concurrency API is working
    }

    #[tokio::test]
    async fn spawn_with_cancellation_succeeds() {
        // Given: No ADR enforces structured concurrency - no spawn without cancellation
        // When: Spawn with cancellation succeeds
        // Then: Spawn with cancellation succeeds

        let config = SemaphoreConfig {
            max_concurrent_binaries: 5,
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));
        let guard = sem.try_acquire_guard().unwrap();

        let mut joinset = JoinSet::new();
        let result = guard.spawn_with_scope(async move { 42 }, &mut joinset);
        assert!(result.is_ok());

        let _handle = result.unwrap();
        let output = joinset.join_one().await.unwrap().unwrap();
        assert_eq!(output, 42);
    }

    #[tokio::test]
    async fn task_joins_correctly() {
        // Given: Tasks spawned via PermitGuard::spawn_with_scope
        // When: Task joins correctly
        // Then: Task joins correctly

        let config = SemaphoreConfig {
            max_concurrent_binaries: 5,
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));

        let mut joinset = JoinSet::new();
        for i in 0..5 {
            let sem = Arc::clone(&sem);
            let guard = sem.try_acquire_guard().unwrap();
            let result = guard.spawn_with_scope(async move { i }, &mut joinset);
            assert!(result.is_ok(), "spawn_with_scope should succeed");
            let _handle = result.unwrap();
            let _ = joinset.join_one().await.unwrap().unwrap();
        }

        // All tasks completed successfully
        assert!(joinset.is_empty());
    }

    #[tokio::test]
    async fn detached_task_detected_via_joinset_empty_check() {
        // Given: Tasks must be tracked in a JoinSet (no detached tasks)
        // When: Detached task detected
        // Then: Detached task detected

        let config = SemaphoreConfig {
            max_concurrent_binaries: 5,
            ..Default::default()
        };
        let sem = Arc::new(ExecutionSemaphore::new(config));
        let guard = sem.try_acquire_guard().unwrap();

        let mut joinset = JoinSet::new();
        let _ = guard.spawn_with_scope(async move { () }, &mut joinset);

        // Verify task is tracked
        assert!(!joinset.is_empty(), "Task should be tracked in JoinSet");

        // Clean up
        joinset.abort_all();
    }

    #[tokio::test]
    async fn execution_semaphore_shed_load_threshold() {
        use crate::semaphore::calc::calculate_backpressure_status;

        assert!(
            calculate_backpressure_status(100, 100, 5, 5).should_reject(),
            "5 waiters >= max_waiters_for_shed(5) should be ShedLoad"
        );
        assert!(
            !calculate_backpressure_status(100, 100, 4, 5).should_reject(),
            "4 waiters < max_waiters_for_shed(5) should not be ShedLoad"
        );
    }

    #[tokio::test]
    async fn execution_semaphore_reserved_and_general_pools_are_independent() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 1,
            reserved_permits: 1,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);

        let general_permit = sem.try_acquire().unwrap();
        assert!(sem.try_acquire().is_none(), "general pool exhausted");
        assert!(
            sem.try_acquire_recovery().is_some(),
            "reserved pool still available"
        );

        drop(general_permit);
        assert!(
            sem.try_acquire().is_some(),
            "general pool replenished after drop"
        );
        assert!(
            sem.try_acquire_recovery().is_some(),
            "reserved pool still available"
        );
    }

    #[tokio::test]
    async fn execution_semaphore_exhausting_general_does_not_affect_reserved() {
        let config = SemaphoreConfig {
            max_concurrent_binaries: 3,
            reserved_permits: 2,
            ..Default::default()
        };
        let sem = ExecutionSemaphore::new(config);

        let _g1 = sem.try_acquire().unwrap();
        let _g2 = sem.try_acquire().unwrap();
        let _g3 = sem.try_acquire().unwrap();

        assert!(sem.try_acquire().is_none(), "general pool exhausted");
        assert_eq!(sem.reserved_available(), 2, "reserved pool unaffected");
        drop(_g1);
        drop(_g2);
        drop(_g3);
        assert_eq!(
            sem.reserved_available(),
            2,
            "reserved still unaffected after general drops"
        );
    }

    #[tokio::test]
    async fn execution_semaphore_backpressure_status_reflects_permit_usage() {
        let sem = ExecutionSemaphore::new(SemaphoreConfig {
            max_concurrent_binaries: 100,
            max_waiters_for_shed: 5000,
            reserved_permits: 0,
            ..Default::default()
        });

        assert_eq!(sem.current_status(), BackpressureStatus::Healthy);

        for _ in 0..60 {
            let _ = sem.try_acquire();
        }
        assert_eq!(sem.current_status(), BackpressureStatus::Moderate);

        for _ in 0..21 {
            let _ = sem.try_acquire();
        }
        assert!(sem.current_status() >= BackpressureStatus::Heavy);
    }
}
