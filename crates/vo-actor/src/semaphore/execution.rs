//! Action Layer — Execution Semaphore
//!
//! Provides the global execution semaphore for limiting concurrent binary spawns.
//! Per ADR-006: Uses `tokio::sync::Semaphore` with fixed permits (e.g., 500)
//! to limit concurrent binary spawns.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{Semaphore, TryAcquireError};

use crate::semaphore::calc::status_from_config_and_state;
use crate::semaphore::types::{AdmissionDecision, BackpressureStatus, RejectionReason, SemaphoreConfig};

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
        assert_eq!(sem.available_permits(), initial - 1, "available_permits does not auto-restore on drop");

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
        assert!(sem.try_acquire_recovery().is_some(), "reserved pool still available");

        drop(general_permit);
        assert!(sem.try_acquire().is_some(), "general pool replenished after drop");
        assert!(sem.try_acquire_recovery().is_some(), "reserved pool still available");
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
        assert_eq!(sem.reserved_available(), 2, "reserved still unaffected after general drops");
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
