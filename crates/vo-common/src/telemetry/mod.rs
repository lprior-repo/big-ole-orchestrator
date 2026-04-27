//! Telemetry pipeline for Veloxide.
//!
//! Provides unified metrics, tracing, and log correlation with OTLP export.
//!
//! # Architecture
//!
//! - [`metrics`] - Thread-safe metric instruments (Counter, Gauge, Histogram)
//! - [`traces`] - Span-based tracing with log correlation
//! - [`export`] - OTLP export pipeline

pub mod metrics;
pub mod traces;
pub mod export;

pub use metrics::{Counter, Gauge, Histogram, TelemetryMetrics};
pub use traces::TelemetryTracer;
pub use export::{TelemetryExporter, OtlpEndpoint, TelemetryConfig};

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct TelemetryState {
    metrics: Arc<TelemetryMetrics>,
    tracer: TelemetryTracer,
}

impl TelemetryState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metrics(&self) -> &Arc<TelemetryMetrics> {
        &self.metrics
    }

    pub fn tracer(&self) -> &TelemetryTracer {
        &self.tracer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_state_default() {
        let state = TelemetryState::new();
        assert_eq!(state.metrics().counters.len(), 0);
    }

    #[test]
    fn telemetry_state_metrics_access() {
        let state = TelemetryState::new();
        let counter = state.metrics().counter("test_counter".into());
        counter.incr();
        assert_eq!(counter.get(), 1);
    }
}
