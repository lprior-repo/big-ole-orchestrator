//! Telemetry pipeline for Veloxide.
//!
//! Provides unified metrics, tracing, and log correlation with OTLP export.
//!
//! # Architecture
//!
//! - [`metrics`] - Thread-safe metric instruments (Counter, Gauge, Histogram)
//! - [`traces`] - Span-based tracing with log correlation
//! - [`export`] - OTLP export pipeline

#[cfg(feature = "telemetry")]
pub mod metrics;

#[cfg(feature = "telemetry")]
pub mod traces;

#[cfg(feature = "telemetry")]
pub mod export;

#[cfg(feature = "telemetry")]
pub use metrics::{Counter, Gauge, Histogram, TelemetryMetrics};

#[cfg(feature = "telemetry")]
pub use traces::TelemetryTracer;

#[cfg(feature = "telemetry")]
pub use export::{OtlpEndpoint, TelemetryConfig, TelemetryExporter};

#[cfg(feature = "telemetry")]
use std::sync::Arc;

#[cfg(feature = "telemetry")]
#[derive(Debug, Default)]
pub(crate) struct TelemetryState {
    metrics: Arc<TelemetryMetrics>,
    tracer: TelemetryTracer,
}

#[cfg(feature = "telemetry")]
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

#[cfg(all(test, feature = "telemetry"))]
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
