//! Write pressure metrics module.
//!
//! Provides observable metrics for writer pressure state, exposing:
//! - Writer queue depth gauge
//! - Batch commit latency gauge
//! - Blob queue depth gauge
//! - Compaction stall indicator gauge
//! - Storage stall indicator gauge
//!
//! All gauges are thread-safe via atomic operations.

use std::sync::atomic::{AtomicU64, Ordering};

/// A thread-safe gauge that can be set to any u64 value.
///
/// Unlike a counter which only increments, a gauge represents a value
/// that can go up or down (e.g., queue depth).
#[derive(Debug, Default)]
pub struct Gauge {
    value: AtomicU64,
}

impl Gauge {
    /// Creates a new gauge with initial value of 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current value.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    /// Sets the gauge to a specific value.
    pub fn set(&self, value: u64) {
        self.value.store(value, Ordering::SeqCst);
    }
}

/// A thread-safe boolean gauge backed by AtomicU64 (0 = false, 1 = true).
#[derive(Debug, Default)]
pub struct BoolGauge {
    value: AtomicU64,
}

impl BoolGauge {
    /// Creates a new boolean gauge with initial value of false.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current boolean value.
    #[must_use]
    pub fn get(&self) -> bool {
        self.value.load(Ordering::SeqCst) != 0
    }

    /// Sets the boolean gauge to a specific value.
    pub fn set(&self, value: bool) {
        self.value
            .store(if value { 1 } else { 0 }, Ordering::SeqCst);
    }
}

/// Metrics for write pressure state.
///
/// Exposes all fields from `WritePressureState` as observable gauges
/// that operators can scrape via Prometheus, OTLP, or an internal registry.
#[derive(Debug, Default)]
pub struct WritePressureMetrics {
    /// Gauge for writer queue depth.
    pub writer_queue_depth: Gauge,
    /// Gauge for batch commit latency in milliseconds.
    pub batch_commit_latency_ms: Gauge,
    /// Gauge for blob queue depth.
    pub blob_queue_depth: Gauge,
    /// Gauge for compaction stall active indicator.
    pub compaction_stall_active: BoolGauge,
    /// Gauge for storage stall active indicator.
    pub storage_stall_active: BoolGauge,
}

impl WritePressureMetrics {
    /// Creates a new set of write pressure metrics with all gauges initialized to zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates all gauges from a `WritePressureState`.
    ///
    /// This method copies the current pressure state values into the gauges,
    /// making them available for scraping by monitoring systems.
    #[inline]
    pub fn update_from_state(&self, state: &crate::admission::types::WritePressureState) {
        self.writer_queue_depth.set(state.writer_queue_depth);
        self.batch_commit_latency_ms
            .set(state.batch_commit_latency_ms);
        self.blob_queue_depth.set(state.blob_queue_depth);
        self.compaction_stall_active
            .set(state.compaction_stall_active);
        self.storage_stall_active.set(state.storage_stall_active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_initial_value_is_zero() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0);
    }

    #[test]
    fn gauge_can_set_and_get_value() {
        let gauge = Gauge::new();
        gauge.set(42);
        assert_eq!(gauge.get(), 42);
    }

    #[test]
    fn gauge_can_update_value() {
        let gauge = Gauge::new();
        gauge.set(100);
        assert_eq!(gauge.get(), 100);
        gauge.set(50);
        assert_eq!(gauge.get(), 50);
    }

    #[test]
    fn gauge_handles_max_u64() {
        let gauge = Gauge::new();
        gauge.set(u64::MAX);
        assert_eq!(gauge.get(), u64::MAX);
    }

    #[test]
    fn bool_gauge_initial_value_is_false() {
        let gauge = BoolGauge::new();
        assert!(!gauge.get());
    }

    #[test]
    fn bool_gauge_can_set_true() {
        let gauge = BoolGauge::new();
        gauge.set(true);
        assert!(gauge.get());
    }

    #[test]
    fn bool_gauge_can_set_false() {
        let gauge = BoolGauge::new();
        gauge.set(true);
        gauge.set(false);
        assert!(!gauge.get());
    }

    #[test]
    fn write_pressure_metrics_initial_all_zero() {
        let metrics = WritePressureMetrics::new();
        assert_eq!(metrics.writer_queue_depth.get(), 0);
        assert_eq!(metrics.batch_commit_latency_ms.get(), 0);
        assert_eq!(metrics.blob_queue_depth.get(), 0);
        assert!(!metrics.compaction_stall_active.get());
        assert!(!metrics.storage_stall_active.get());
    }

    #[test]
    fn write_pressure_metrics_update_from_state() {
        use crate::admission::types::WritePressureState;

        let metrics = WritePressureMetrics::new();
        let state = WritePressureState {
            writer_queue_depth: 100,
            batch_commit_latency_ms: 500,
            blob_queue_depth: 25,
            compaction_stall_active: true,
            storage_stall_active: false,
        };

        metrics.update_from_state(&state);

        assert_eq!(metrics.writer_queue_depth.get(), 100);
        assert_eq!(metrics.batch_commit_latency_ms.get(), 500);
        assert_eq!(metrics.blob_queue_depth.get(), 25);
        assert!(metrics.compaction_stall_active.get());
        assert!(!metrics.storage_stall_active.get());
    }

    #[test]
    fn write_pressure_metrics_update_from_default_state() {
        use crate::admission::types::WritePressureState;

        let metrics = WritePressureMetrics::new();

        // Set some non-zero values first
        metrics.writer_queue_depth.set(999);

        // Update with default state (all zeros)
        let state = WritePressureState::default();
        metrics.update_from_state(&state);

        assert_eq!(metrics.writer_queue_depth.get(), 0);
    }

    #[test]
    fn write_pressure_metrics_max_u64_values() {
        use crate::admission::types::WritePressureState;

        let metrics = WritePressureMetrics::new();
        let state = WritePressureState {
            writer_queue_depth: u64::MAX,
            batch_commit_latency_ms: u64::MAX,
            blob_queue_depth: u64::MAX,
            compaction_stall_active: true,
            storage_stall_active: true,
        };

        metrics.update_from_state(&state);

        assert_eq!(metrics.writer_queue_depth.get(), u64::MAX);
        assert_eq!(metrics.batch_commit_latency_ms.get(), u64::MAX);
        assert_eq!(metrics.blob_queue_depth.get(), u64::MAX);
        assert!(metrics.compaction_stall_active.get());
        assert!(metrics.storage_stall_active.get());
    }
}
