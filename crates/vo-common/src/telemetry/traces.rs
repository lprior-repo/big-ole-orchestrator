//! Span-based tracing integration for telemetry.

#[derive(Debug, Default, Clone, Copy)]
pub struct TelemetryTracer;

impl TelemetryTracer {
    pub fn record_event(&self, name: &str) {
        tracing::info!(event = name, "telemetry event");
    }
}
