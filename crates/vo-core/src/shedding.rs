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
    semaphore: Arc<Semaphore>,
    permits: usize,
}

impl SemaphorePermit {
    fn new(semaphore: Arc<Semaphore>, permits: usize) -> Self {
        Self { semaphore, permits }
    }
}

impl Drop for SemaphorePermit {
    fn drop(&mut self) {
        self.semaphore.add_permits(self.permits);
    }
}

#[derive(Debug, Clone)]
pub struct LoadSheddingSemaphore {
    semaphore: Arc<Semaphore>,
    max_permits: usize,
}

impl LoadSheddingSemaphore {
    pub fn new(max_permits: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_permits)),
            max_permits,
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
        match self.semaphore.try_acquire_many(permits as u32) {
            Ok(_permit) => Ok(SemaphorePermit::new(self.semaphore.clone(), permits)),
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
        self.max_permits.saturating_sub(self.available_permits())
    }

    pub fn is_load_shedding_active(&self, threshold: usize) -> bool {
        let waiting = self.max_permits.saturating_sub(self.available_permits());
        waiting >= threshold
    }

    pub fn check_load_shedding(&self) -> Result<(), SemaphoreLimitError> {
        self.check_load_shedding_threshold(MAX_YIELDED_ACTORS)
    }

    pub fn check_load_shedding_threshold(
        &self,
        threshold: usize,
    ) -> Result<(), SemaphoreLimitError> {
        if self.is_load_shedding_active(threshold) {
            let waiting = self.max_permits.saturating_sub(self.available_permits());
            Err(SemaphoreLimitError::LoadSheddingActive {
                yielded_actors: waiting,
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
