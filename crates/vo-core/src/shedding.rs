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
            Err(TryAcquireError::Closed) => {
                panic!("semaphore closed unexpectedly")
            }
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
