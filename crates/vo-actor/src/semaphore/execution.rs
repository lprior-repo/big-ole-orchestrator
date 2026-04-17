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
}
