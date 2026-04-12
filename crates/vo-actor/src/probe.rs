//! Health check probe framework for monitoring component health.
//!
//! Provides configurable health probes with:
//! - HTTP/TCP/exec probe types
//! - Interval and backoff configuration
//! - Status aggregation across multiple probes
//! - Alerting thresholds
//!
//! # Example
//!
//! ```ignore
//! use vo_actor::probe::{Probe, HttpProbe, ProbeConfig};
//!
//! let probe = HttpProbe::new("http://localhost:8080/health");
//! let config = ProbeConfig::default()
//!     .with_interval(Duration::from_secs(30))
//!     .with_failure_threshold(3);
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeType {
    Http,
    Tcp,
    Exec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub probe_id: ProbeId,
    pub status: ProbeStatus,
    pub latency_ms: u64,
    pub consecutive_failures: u32,
    pub last_check_ms: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeConfig {
    pub url: String,
    pub expected_status: Option<u16>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpProbeConfig {
    pub address: SocketAddr,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecProbeConfig {
    pub command: String,
    pub args: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProbeConfig {
    Http {
        url: String,
        expected_status: Option<u16>,
        timeout_ms: u64,
    },
    Tcp {
        address: String,
        port: u16,
        timeout_ms: u64,
    },
    Exec {
        command: String,
        args: Vec<String>,
        expected_exit_code: Option<i32>,
        timeout_ms: u64,
    },
}

impl ProbeConfig {
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            expected_status: Some(200),
            timeout_ms: 5000,
        }
    }

    pub fn tcp(address: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            address: address.into(),
            port,
            timeout_ms: 5000,
        }
    }

    pub fn exec(command: impl Into<String>, args: Vec<String>) -> Self {
        Self::Exec {
            command: command.into(),
            args,
            expected_exit_code: Some(0),
            timeout_ms: 30000,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        match &mut self {
            Self::Http { timeout_ms, .. } => *timeout_ms = timeout.as_millis() as u64,
            Self::Tcp { timeout_ms, .. } => *timeout_ms = timeout.as_millis() as u64,
            Self::Exec { timeout_ms, .. } => *timeout_ms = timeout.as_millis() as u64,
        }
        self
    }

    pub fn timeout(&self) -> Duration {
        match self {
            Self::Http { timeout_ms, .. } | Self::Tcp { timeout_ms, .. } | Self::Exec { timeout_ms, .. } => {
                Duration::from_millis(*timeout_ms)
            }
        }
    }

    pub fn probe_type(&self) -> ProbeType {
        match self {
            Self::Http { .. } => ProbeType::Http,
            Self::Tcp { .. } => ProbeType::Tcp,
            Self::Exec { .. } => ProbeType::Exec,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProbeId(pub ulid::Ulid);

impl ProbeId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }

    pub fn from_string(s: &str) -> Option<Self> {
        s.strip_prefix("probe-")
            .and_then(|s| ulid::Ulid::from_str(s).ok())
            .map(Self)
    }

    pub fn as_str(&self) -> String {
        format!("probe-{}", self.0)
    }
}

impl Default for ProbeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProbeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "probe-{}", self.0)
    }
}

impl serde::Serialize for ProbeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for ProbeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ProbeId::from_string(&s)
            .ok_or_else(|| serde::de::Error::custom("Invalid probe ID format"))
    }
}

#[derive(Debug, Clone)]
pub struct ProbeDefinition {
    pub id: ProbeId,
    pub name: String,
    pub config: ProbeConfig,
    pub interval: Duration,
    pub backoff: BackoffConfig,
    pub failure_threshold: u32,
    pub success_threshold: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BackoffConfig {
    pub initial_interval: Duration,
    pub max_interval: Duration,
    pub multiplier: f64,
    pub max_failures: u32,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(60),
            multiplier: 2.0,
            max_failures: 10,
        }
    }
}

impl BackoffConfig {
    pub fn calculate_interval(&self, consecutive_failures: u32) -> Duration {
        let failures = consecutive_failures.min(self.max_failures);
        let interval_ms = self.initial_interval.as_millis() as f64
            * self.multiplier.powi(failures as i32);
        let interval_ms = interval_ms.min(self.max_interval.as_millis() as f64);
        Duration::from_millis(interval_ms as u64)
    }
}

#[derive(Debug, Clone)]
pub struct AggregatedStatus {
    pub overall: ProbeStatus,
    pub healthy_count: u32,
    pub unhealthy_count: u32,
    pub unknown_count: u32,
    pub results: HashMap<ProbeId, ProbeResult>,
}

impl AggregatedStatus {
    pub fn new() -> Self {
        Self {
            overall: ProbeStatus::Unknown,
            healthy_count: 0,
            unhealthy_count: 0,
            unknown_count: 0,
            results: HashMap::new(),
        }
    }

    pub fn update(&mut self, result: ProbeResult) {
        match result.status {
            ProbeStatus::Healthy => self.healthy_count += 1,
            ProbeStatus::Unhealthy => self.unhealthy_count += 1,
            ProbeStatus::Unknown => self.unknown_count += 1,
        }
        self.results.insert(result.probe_id, result);

        self.overall = if self.unhealthy_count > 0 {
            ProbeStatus::Unhealthy
        } else if self.healthy_count > 0 && self.unknown_count == 0 {
            ProbeStatus::Healthy
        } else {
            ProbeStatus::Unknown
        };
    }

    pub fn is_healthy(&self) -> bool {
        self.overall == ProbeStatus::Healthy
    }
}

impl Default for AggregatedStatus {
    fn default() -> Self {
        Self::new()
    }
}

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

#[async_trait]
pub trait Probe: Send + Sync {
    async fn check(&self) -> Result<ProbeResult, ProbeError>;
    fn probe_id(&self) -> ProbeId;
}

pub struct HttpProbe {
    id: ProbeId,
    url: String,
    expected_status: Option<u16>,
    timeout: Duration,
    client: reqwest::Client,
}

impl HttpProbe {
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

    pub fn with_expected_status(mut self, status: u16) -> Self {
        self.expected_status = Some(status);
        self
    }

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

pub struct TcpProbe {
    id: ProbeId,
    address: SocketAddr,
    timeout: Duration,
}

impl TcpProbe {
    pub fn new(address: SocketAddr) -> Self {
        Self {
            id: ProbeId::new(),
            address,
            timeout: Duration::from_secs(5),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl Probe for TcpProbe {
    async fn check(&self) -> Result<ProbeResult, ProbeError> {
        let start = tokio::time::Instant::now();

        let connection = tokio::time::timeout(
            self.timeout,
            tokio::net::TcpStream::connect(self.address),
        )
        .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        let (status, message) = match connection {
            Ok(Ok(_)) => (ProbeStatus::Healthy, format!("TCP connect to {}", self.address)),
            Ok(Err(e)) => {
                (ProbeStatus::Unhealthy, format!("TCP failed to {}: {}", self.address, e))
            }
            Err(_) => (
                ProbeStatus::Unhealthy,
                format!("TCP connect to {} timed out after {:?}", self.address, self.timeout),
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

pub struct ExecProbe {
    id: ProbeId,
    command: String,
    args: Vec<String>,
    expected_exit_code: Option<i32>,
    timeout: Duration,
}

impl ExecProbe {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            id: ProbeId::new(),
            command: command.into(),
            args,
            expected_exit_code: Some(0),
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_expected_exit_code(mut self, code: i32) -> Self {
        self.expected_exit_code = Some(code);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl Probe for ExecProbe {
    async fn check(&self) -> Result<ProbeResult, ProbeError> {
        let start = tokio::time::Instant::now();

        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| ProbeError::Exec(e.to_string()))?;

        let latency_ms = start.elapsed().as_millis() as u64;

        let exit_code = output.status.code();
        let probe_status = if let Some(expected) = self.expected_exit_code {
            if exit_code == Some(expected) {
                ProbeStatus::Healthy
            } else {
                ProbeStatus::Unhealthy
            }
        } else if output.status.success() {
            ProbeStatus::Healthy
        } else {
            ProbeStatus::Unhealthy
        };

        let message = if let Some(code) = exit_code {
            format!("Exec '{}' exited with {}", self.command, code)
        } else {
            format!("Exec '{}' terminated by signal", self.command)
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
            message: Some(message),
        })
    }

    fn probe_id(&self) -> ProbeId {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct ProbeRegistry {
    probes: HashMap<ProbeId, ProbeDefinition>,
}

impl ProbeRegistry {
    pub fn new() -> Self {
        Self {
            probes: HashMap::new(),
        }
    }

    pub fn register(&mut self, definition: ProbeDefinition) -> ProbeId {
        let id = definition.id;
        self.probes.insert(id, definition);
        id
    }

    pub fn unregister(&mut self, id: ProbeId) -> Option<ProbeDefinition> {
        self.probes.remove(&id)
    }

    pub fn get(&self, id: &ProbeId) -> Option<&ProbeDefinition> {
        self.probes.get(id)
    }

    pub fn list(&self) -> Vec<&ProbeDefinition> {
        self.probes.values().collect()
    }

    pub fn len(&self) -> usize {
        self.probes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }
}

impl Default for ProbeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_config_calculate_interval() {
        let config = BackoffConfig::default();

        assert_eq!(config.calculate_interval(0), Duration::from_secs(1));
        assert_eq!(config.calculate_interval(1), Duration::from_secs(2));
        assert_eq!(config.calculate_interval(2), Duration::from_secs(4));
        assert_eq!(config.calculate_interval(3), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_config_max_interval() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
            max_failures: 10,
        };

        assert_eq!(config.calculate_interval(10), Duration::from_secs(10));
        assert_eq!(config.calculate_interval(100), Duration::from_secs(10));
    }

    #[test]
    fn test_aggregated_status_update() {
        let mut status = AggregatedStatus::new();

        let healthy_result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: None,
        };

        status.update(healthy_result);
        assert_eq!(status.healthy_count, 1);
        assert_eq!(status.unhealthy_count, 0);
        assert_eq!(status.overall, ProbeStatus::Healthy);

        let unhealthy_result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: None,
        };

        status.update(unhealthy_result);
        assert_eq!(status.healthy_count, 1);
        assert_eq!(status.unhealthy_count, 1);
        assert_eq!(status.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn test_probe_config_timeout() {
        let config = ProbeConfig::http("http://localhost:8080/health");
        assert_eq!(config.timeout(), Duration::from_millis(5000));

        let config = config.with_timeout(Duration::from_secs(10));
        assert_eq!(config.timeout(), Duration::from_secs(10));
    }

    #[test]
    fn test_probe_id_display() {
        let id = ProbeId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("probe-"));
    }
}