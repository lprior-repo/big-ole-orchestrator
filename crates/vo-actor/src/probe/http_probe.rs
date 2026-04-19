//! HTTP probe implementation.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::error::ProbeError;
use super::types::{ProbeId, ProbeResult, ProbeStatus};
use super::Probe;

/// HTTP health probe.
pub struct HttpProbe {
    id: ProbeId,
    url: String,
    expected_status: Option<u16>,
    timeout: Duration,
    client: Client,
}

impl HttpProbe {
    /// Create a new HTTP probe.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            id: ProbeId::new(),
            url: url.into(),
            expected_status: Some(200),
            timeout: Duration::from_secs(5),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Set expected HTTP status code.
    pub fn with_expected_status(mut self, status: u16) -> Self {
        self.expected_status = Some(status);
        self
    }

    /// Set probe timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        self
    }
}

#[async_trait]
impl Probe for HttpProbe {
    async fn check(&self) -> Result<ProbeResult, ProbeError> {
        let start = tokio::time::Instant::now();

        let response = self
            .client
            .get(&self.url)
            .send()
            .await
            .map_err(|e| ProbeError::Http(e.to_string()))?;

        let latency_ms = start.elapsed().as_millis() as u64;
        let status = response.status().as_u16();

        let probe_status = if let Some(expected) = self.expected_status {
            if status == expected {
                ProbeStatus::Healthy
            } else {
                ProbeStatus::Unhealthy
            }
        } else {
            if response.status().is_success() {
                ProbeStatus::Healthy
            } else {
                ProbeStatus::Unhealthy
            }
        };

        Ok(ProbeResult {
            probe_id: self.id,
            status: probe_status,
            latency_ms,
            consecutive_failures: 0,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: Some(format!("HTTP {} -> {}", self.url, status)),
        })
    }

    fn probe_id(&self) -> ProbeId {
        self.id
    }
}
