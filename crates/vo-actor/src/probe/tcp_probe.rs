//! TCP probe implementation.

use std::net::SocketAddr;
use std::time::Duration;

use async_trait::async_trait;

use super::error::ProbeError;
use super::types::{ProbeId, ProbeResult, ProbeStatus};
use super::Probe;

/// TCP health probe.
pub struct TcpProbe {
    id: ProbeId,
    address: SocketAddr,
    timeout: Duration,
}

impl TcpProbe {
    /// Create a new TCP probe.
    pub fn new(address: SocketAddr) -> Self {
        Self {
            id: ProbeId::new(),
            address,
            timeout: Duration::from_secs(5),
        }
    }

    /// Set probe timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl Probe for TcpProbe {
    async fn check(&self) -> Result<ProbeResult, ProbeError> {
        let start = tokio::time::Instant::now();

        let connection =
            tokio::time::timeout(self.timeout, tokio::net::TcpStream::connect(self.address)).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        let (status, message) = match connection {
            Ok(Ok(_)) => (
                ProbeStatus::Healthy,
                format!("TCP connect to {}", self.address),
            ),
            Ok(Err(e)) => (
                ProbeStatus::Unhealthy,
                format!("TCP failed to {}: {}", self.address, e),
            ),
            Err(_) => (
                ProbeStatus::Unhealthy,
                format!(
                    "TCP connect to {} timed out after {:?}",
                    self.address, self.timeout
                ),
            ),
        };

        Ok(ProbeResult {
            probe_id: self.id,
            status,
            latency_ms,
            consecutive_failures: 0,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: Some(message),
        })
    }

    fn probe_id(&self) -> ProbeId {
        self.id
    }
}
