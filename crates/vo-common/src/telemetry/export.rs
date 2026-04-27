//! OTLP telemetry export configuration.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpEndpoint(String);

impl OtlpEndpoint {
    #[must_use]
    pub fn new(endpoint: String) -> Self {
        Self(endpoint)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryConfig {
    pub endpoint: OtlpEndpoint,
    pub service_name: String,
}

impl TelemetryConfig {
    #[must_use]
    pub fn new(endpoint: OtlpEndpoint, service_name: String) -> Self {
        Self {
            endpoint,
            service_name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetryExporter {
    config: TelemetryConfig,
}

impl TelemetryExporter {
    #[must_use]
    pub fn new(config: TelemetryConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }
}
