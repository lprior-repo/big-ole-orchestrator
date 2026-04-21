//! Probe error types and trait definitions.

use std::time::Duration;

use thiserror::Error;

use super::types::ProbeId;

/// Errors that can occur during probe operations.
#[derive(Debug, Clone, Error)]
pub enum ProbeError {
    #[error("HTTP probe failed: {0}")]
    Http(String),

    #[error("TCP probe failed: {0}")]
    Tcp(String),

    #[error("Exec probe failed: {0}")]
    Exec(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Probe {0} not found")]
    NotFound(ProbeId),
}

/// Trait for health probes.
#[async_trait::async_trait]
pub trait Probe: Send + Sync {
    /// Perform the health check.
    async fn check(&self) -> Result<super::types::ProbeResult, ProbeError>;

    /// Get the probe's unique identifier.
    fn probe_id(&self) -> super::types::ProbeId;
}
