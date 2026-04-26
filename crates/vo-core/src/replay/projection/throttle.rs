//! Rebuild throttle — token-bucket rate limiter for concurrent rebuilds.
//!
//! ## Architecture
//!
//! - `RebuildThrottleConfig` — data layer: configuration parameters
//! - `RebuildThrottleState` — calc layer: token bucket logic

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct RebuildThrottleConfig {
    pub max_concurrent_rebuilds: usize,
    pub refill_interval_ms: u64,
    pub tokens_per_refill: usize,
}

impl Default for RebuildThrottleConfig {
    fn default() -> Self {
        Self {
            max_concurrent_rebuilds: 5,
            refill_interval_ms: 100,
            tokens_per_refill: 1,
        }
    }
}

impl RebuildThrottleConfig {
    #[must_use]
    pub const fn new(
        max_concurrent_rebuilds: usize,
        refill_interval_ms: u64,
        tokens_per_refill: usize,
    ) -> Self {
        Self {
            max_concurrent_rebuilds,
            refill_interval_ms,
            tokens_per_refill,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RebuildThrottleState {
    available_tokens: usize,
    max_tokens: usize,
    last_refill: Instant,
    refill_interval: Duration,
    tokens_per_refill: usize,
    #[allow(dead_code)]
    active_rebuilds: AtomicUsize,
}

impl RebuildThrottleState {
    pub(crate) fn new(config: RebuildThrottleConfig) -> Self {
        Self {
            available_tokens: config.max_concurrent_rebuilds,
            max_tokens: config.max_concurrent_rebuilds,
            last_refill: Instant::now(),
            refill_interval: Duration::from_millis(config.refill_interval_ms),
            tokens_per_refill: config.tokens_per_refill,
            active_rebuilds: AtomicUsize::new(0),
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed();
        if elapsed >= self.refill_interval {
            let intervals = (elapsed.as_millis() / self.refill_interval.as_millis()) as usize;
            let new_tokens = intervals * self.tokens_per_refill;
            self.available_tokens = (self.available_tokens + new_tokens).min(self.max_tokens);
            self.last_refill = Instant::now();
        }
    }

    pub(crate) fn try_acquire_slot(&mut self) -> Option<u64> {
        self.refill();
        if self.available_tokens > 0
            && self.active_rebuilds.load(Ordering::Relaxed) < self.max_tokens
        {
            self.available_tokens -= 1;
            self.active_rebuilds.fetch_add(1, Ordering::Relaxed);
            Some(0)
        } else {
            let wait_time = self.refill_interval.as_millis() as u64;
            Some(wait_time.max(10))
        }
    }

    pub(crate) fn release_slot(&self) {
        self.active_rebuilds.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.active_rebuilds.load(Ordering::Relaxed) == 0
    }

    pub(crate) fn active_count(&self) -> usize {
        self.active_rebuilds.load(Ordering::Relaxed)
    }
}
