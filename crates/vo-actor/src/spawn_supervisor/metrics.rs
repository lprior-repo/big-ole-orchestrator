//! Metrics: `Counter` and `SpawnSupervisorMetrics`

use std::sync::atomic::AtomicU64;

/// Simple counter for metrics using AtomicU64
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    /// Creates a new Counter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current value.
    pub fn get(&self) -> u64 {
        self.value.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Increments the counter.
    pub fn incr(&self) {
        self.value.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Metrics for `SpawnSupervisor`
#[derive(Debug, Default)]
pub struct SpawnSupervisorMetrics {
    /// Number of successful spawns.
    pub spawns_successful: Counter,
    /// Number of spawns that failed.
    pub spawns_failed: Counter,
    /// Number of health checks performed.
    pub health_checks_performed: Counter,
    /// Number of health checks that failed.
    pub health_checks_failed: Counter,
    /// Number of zombie processes detected.
    pub zombies_detected: Counter,
    /// Number of respawns.
    pub respawns: Counter,
    /// Number of dispatch errors.
    pub dispatch_errors: Counter,
}
