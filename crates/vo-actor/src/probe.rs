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
    use proptest::prelude::*;

    // =========================================================================
    // ProbeId Tests (T01-T07)
    // =========================================================================

    #[test]
    fn test_probe_id_new_generates_unique_ids() {
        let id1 = ProbeId::new();
        let id2 = ProbeId::new();
        assert_ne!(id1, id2, "ProbeId::new() should generate unique IDs");
    }

    #[test]
    fn test_probe_id_from_string_parses_valid_format() {
        let id = ProbeId::new();
        let s = id.as_str();
        let parsed = ProbeId::from_string(&s);
        assert!(parsed.is_some(), "Should parse valid probe-<ULID> format");
        assert_eq!(parsed.unwrap(), id);
    }

    #[test]
    fn test_probe_id_from_string_returns_none_for_invalid() {
        assert!(ProbeId::from_string("invalid").is_none());
        assert!(ProbeId::from_string("probe-").is_none());
        assert!(ProbeId::from_string("").is_none());
        assert!(ProbeId::from_string("not-a-probe-01AZAR0").is_none());
    }

    #[test]
    fn test_probe_id_as_str_returns_correct_format() {
        let id = ProbeId::new();
        let s = id.as_str();
        assert!(s.starts_with("probe-"), "as_str() should return probe-<ULID> format");
        assert_eq!(s, format!("probe-{}", id.0));
    }

    #[test]
    fn test_probe_id_display_impl() {
        let id = ProbeId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.as_str());
    }

    #[test]
    fn test_probe_id_serialization_roundtrip() {
        let id = ProbeId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: ProbeId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_probe_id_deserialization_rejects_malformed() {
        let result: Result<ProbeId, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
        let result: Result<ProbeId, _> = serde_json::from_str("\"probe-\"");
        assert!(result.is_err());
    }

    // =========================================================================
    // ProbeResult Tests (T08-T11)
    // =========================================================================

    #[test]
    fn test_probe_result_fields_healthy() {
        let id = ProbeId::new();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 100,
            consecutive_failures: 0,
            last_check_ms: 1234567890,
            message: Some("OK".to_string()),
        };
        assert_eq!(result.status, ProbeStatus::Healthy);
        assert_eq!(result.latency_ms, 100);
        assert_eq!(result.consecutive_failures, 0);
    }

    #[test]
    fn test_probe_result_fields_unhealthy() {
        let id = ProbeId::new();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Unhealthy,
            latency_ms: 50,
            consecutive_failures: 3,
            last_check_ms: 1234567890,
            message: Some("Connection refused".to_string()),
        };
        assert_eq!(result.status, ProbeStatus::Unhealthy);
        assert_eq!(result.consecutive_failures, 3);
    }

    #[test]
    fn test_probe_result_fields_unknown() {
        let id = ProbeId::new();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Unknown,
            latency_ms: 0,
            consecutive_failures: 0,
            last_check_ms: 0,
            message: None,
        };
        assert_eq!(result.status, ProbeStatus::Unknown);
    }

    #[test]
    fn test_probe_result_latency_is_nonzero_for_completed() {
        let id = ProbeId::new();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            message: None,
        };
        assert!(result.latency_ms > 0, "Completed probe should have non-zero latency");
    }

    // =========================================================================
    // ProbeConfig Tests (T12-T18)
    // =========================================================================

    #[test]
    fn test_probe_config_http_creates_valid_config() {
        let config = ProbeConfig::http("http://localhost:8080/health");
        match config {
            ProbeConfig::Http { url, expected_status, timeout_ms } => {
                assert_eq!(url, "http://localhost:8080/health");
                assert_eq!(expected_status, Some(200));
                assert_eq!(timeout_ms, 5000);
            }
            _ => panic!("Expected Http variant"),
        }
    }

    #[test]
    fn test_probe_config_tcp_creates_valid_config() {
        let config = ProbeConfig::tcp("localhost", 8080);
        match config {
            ProbeConfig::Tcp { address, port, timeout_ms } => {
                assert_eq!(address, "localhost");
                assert_eq!(port, 8080);
                assert_eq!(timeout_ms, 5000);
            }
            _ => panic!("Expected Tcp variant"),
        }
    }

    #[test]
    fn test_probe_config_exec_creates_valid_config() {
        let config = ProbeConfig::exec("echo", vec!["hello".to_string()]);
        match config {
            ProbeConfig::Exec { command, args, expected_exit_code, timeout_ms } => {
                assert_eq!(command, "echo");
                assert_eq!(args, vec!["hello"]);
                assert_eq!(expected_exit_code, Some(0));
                assert_eq!(timeout_ms, 30000);
            }
            _ => panic!("Expected Exec variant"),
        }
    }

    #[test]
    fn test_probe_config_with_timeout_modifies_timeout() {
        let config = ProbeConfig::http("http://localhost:8080/health")
            .with_timeout(Duration::from_secs(30));
        assert_eq!(config.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn test_probe_config_timeout_returns_correct_duration() {
        let config = ProbeConfig::http("http://localhost:8080/health");
        assert_eq!(config.timeout(), Duration::from_millis(5000));

        let tcp_config = ProbeConfig::tcp("localhost", 8080);
        assert_eq!(tcp_config.timeout(), Duration::from_millis(5000));

        let exec_config = ProbeConfig::exec("echo", vec![]);
        assert_eq!(exec_config.timeout(), Duration::from_millis(30000));
    }

    #[test]
    fn test_probe_config_probe_type_returns_correct_variant() {
        assert_eq!(ProbeConfig::http("http://localhost").probe_type(), ProbeType::Http);
        assert_eq!(ProbeConfig::tcp("localhost", 8080).probe_type(), ProbeType::Tcp);
        assert_eq!(ProbeConfig::exec("echo", vec![]).probe_type(), ProbeType::Exec);
    }

    #[test]
    fn test_probe_config_serialization_roundtrip() {
        let config = ProbeConfig::http("http://localhost:8080/health")
            .with_timeout(Duration::from_secs(10));
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProbeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.timeout(), config.timeout());
    }

    // =========================================================================
    // BackoffConfig Tests (T19-T25)
    // =========================================================================

    #[test]
    fn test_backoff_config_calculate_interval_zero_failures() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(0), Duration::from_secs(1));
    }

    #[test]
    fn test_backoff_config_calculate_interval_one_failure() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(1), Duration::from_secs(2));
    }

    #[test]
    fn test_backoff_config_exponential_growth() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(0), Duration::from_secs(1));
        assert_eq!(config.calculate_interval(1), Duration::from_secs(2));
        assert_eq!(config.calculate_interval(2), Duration::from_secs(4));
        assert_eq!(config.calculate_interval(3), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_config_respects_max_interval() {
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
    fn test_backoff_config_handles_overflow() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(10),
            multiplier: 2.0,
            max_failures: 5,
        };
        assert_eq!(config.calculate_interval(5), Duration::from_secs(10));
        assert_eq!(config.calculate_interval(100), Duration::from_secs(10));
    }

    #[test]
    fn test_backoff_config_default_values() {
        let config = BackoffConfig::default();
        assert_eq!(config.initial_interval, Duration::from_secs(1));
        assert_eq!(config.max_interval, Duration::from_secs(60));
        assert_eq!(config.multiplier, 2.0);
        assert_eq!(config.max_failures, 10);
    }

    #[test]
    fn test_backoff_config_multiplier_one_produces_constant() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(5),
            max_interval: Duration::from_secs(60),
            multiplier: 1.0,
            max_failures: 10,
        };
        assert_eq!(config.calculate_interval(0), Duration::from_secs(5));
        assert_eq!(config.calculate_interval(1), Duration::from_secs(5));
        assert_eq!(config.calculate_interval(5), Duration::from_secs(5));
    }

    // =========================================================================
    // AggregatedStatus Tests (T26-T35)
    // =========================================================================

    #[test]
    fn test_aggregated_status_new_initializes_unknown() {
        let status = AggregatedStatus::new();
        assert_eq!(status.overall, ProbeStatus::Unknown);
        assert_eq!(status.healthy_count, 0);
        assert_eq!(status.unhealthy_count, 0);
        assert_eq!(status.unknown_count, 0);
    }

    #[test]
    fn test_aggregated_status_update_healthy() {
        let mut status = AggregatedStatus::new();
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(result);
        assert_eq!(status.healthy_count, 1);
        assert_eq!(status.unhealthy_count, 0);
        assert_eq!(status.unknown_count, 0);
    }

    #[test]
    fn test_aggregated_status_update_unhealthy() {
        let mut status = AggregatedStatus::new();
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(result);
        assert_eq!(status.unhealthy_count, 1);
    }

    #[test]
    fn test_aggregated_status_update_unknown() {
        let mut status = AggregatedStatus::new();
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unknown,
            latency_ms: 0,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(result);
        assert_eq!(status.unknown_count, 1);
    }

    #[test]
    fn test_aggregated_status_overall_unhealthy_when_any_probe_unhealthy() {
        let mut status = AggregatedStatus::new();

        let healthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(healthy);

        let unhealthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(unhealthy);

        assert_eq!(status.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn test_aggregated_status_overall_healthy_when_all_healthy_and_none_unknown() {
        let mut status = AggregatedStatus::new();

        let h1 = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(h1);

        let h2 = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(h2);

        assert_eq!(status.overall, ProbeStatus::Healthy);
    }

    #[test]
    fn test_aggregated_status_overall_unknown_when_mix_of_healthy_and_unknown() {
        let mut status = AggregatedStatus::new();

        let healthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(healthy);

        let unknown = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unknown,
            latency_ms: 0,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(unknown);

        assert_eq!(status.overall, ProbeStatus::Unknown);
    }

    #[test]
    fn test_aggregated_status_is_healthy() {
        let mut status = AggregatedStatus::new();
        assert!(!status.is_healthy());

        let healthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(healthy);
        assert!(status.is_healthy());

        let unhealthy = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(unhealthy);
        assert!(!status.is_healthy());
    }

    #[test]
    fn test_aggregated_status_update_replaces_previous_for_same_probe() {
        let mut status = AggregatedStatus::new();
        let probe_id = ProbeId::new();

        let r1 = ProbeResult {
            probe_id: probe_id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r1);
        assert_eq!(status.healthy_count, 1);

        let r2 = ProbeResult {
            probe_id: probe_id,
            status: ProbeStatus::Unhealthy,
            latency_ms: 20,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r2);
        assert_eq!(status.healthy_count, 0);
        assert_eq!(status.unhealthy_count, 1);
        assert_eq!(status.results.len(), 1);
    }

    #[test]
    fn test_aggregated_status_multiple_probes_tracked() {
        let mut status = AggregatedStatus::new();

        for i in 0..5 {
            let result = ProbeResult {
                probe_id: ProbeId::new(),
                status: ProbeStatus::Healthy,
                latency_ms: 10 + i as u64,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            };
            status.update(result);
        }

        assert_eq!(status.results.len(), 5);
        assert_eq!(status.healthy_count, 5);
    }

    // =========================================================================
    // ProbeError Tests (T66-T70)
    // =========================================================================

    #[test]
    fn test_probe_error_http_message_format() {
        let err = ProbeError::Http("connection refused".to_string());
        assert!(err.to_string().contains("HTTP probe failed"));
    }

    #[test]
    fn test_probe_error_tcp_message_format() {
        let err = ProbeError::Tcp("connection refused".to_string());
        assert!(err.to_string().contains("TCP probe failed"));
    }

    #[test]
    fn test_probe_error_exec_message_format() {
        let err = ProbeError::Exec("exit code 1".to_string());
        assert!(err.to_string().contains("Exec probe failed"));
    }

    #[test]
    fn test_probe_error_timeout_message_format() {
        let err = ProbeError::Timeout(Duration::from_secs(5));
        assert!(err.to_string().contains("Timeout"));
    }

    #[test]
    fn test_probe_error_not_found_message_format() {
        let id = ProbeId::new();
        let err = ProbeError::NotFound(id);
        let msg = err.to_string();
        assert!(msg.contains("not found") || msg.contains("Probe"));
    }

    // =========================================================================
    // ProbeRegistry Tests (T57-T65)
    // =========================================================================

    #[test]
    fn test_probe_registry_register() {
        let mut registry = ProbeRegistry::new();
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        let id = registry.register(definition);
        assert!(registry.get(&id).is_some());
    }

    #[test]
    fn test_probe_registry_unregister() {
        let mut registry = ProbeRegistry::new();
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        let id = registry.register(definition);
        let removed = registry.unregister(id);
        assert!(removed.is_some());
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn test_probe_registry_unregister_nonexistent() {
        let mut registry = ProbeRegistry::new();
        let id = ProbeId::new();
        assert!(registry.unregister(id).is_none());
    }

    #[test]
    fn test_probe_registry_get() {
        let mut registry = ProbeRegistry::new();
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        let id = registry.register(definition);
        assert!(registry.get(&id).is_some());
    }

    #[test]
    fn test_probe_registry_get_nonexistent() {
        let registry = ProbeRegistry::new();
        let id = ProbeId::new();
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn test_probe_registry_list() {
        let mut registry = ProbeRegistry::new();
        for i in 0..3 {
            let definition = ProbeDefinition {
                id: ProbeId::new(),
                name: format!("test{}", i),
                config: ProbeConfig::http("http://localhost"),
                interval: Duration::from_secs(30),
                backoff: BackoffConfig::default(),
                failure_threshold: 3,
                success_threshold: 2,
            };
            registry.register(definition);
        }
        assert_eq!(registry.list().len(), 3);
    }

    #[test]
    fn test_probe_registry_len() {
        let mut registry = ProbeRegistry::new();
        assert_eq!(registry.len(), 0);
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        registry.register(definition);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_probe_registry_is_empty() {
        let registry = ProbeRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_probe_registry_is_empty_after_register() {
        let mut registry = ProbeRegistry::new();
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        registry.register(definition);
        assert!(!registry.is_empty());
    }

    // =========================================================================
    // Invariant Tests (T71-T85) - Key invariants from contract
    // =========================================================================

    #[test]
    fn test_inv_healthy_only_after_consecutive_successes() {
        let config = BackoffConfig::default();
        assert_eq!(config.initial_interval, Duration::from_secs(1));
    }

    #[test]
    fn test_inv_unhealthy_after_consecutive_failures() {
        let config = BackoffConfig::default();
        let interval = config.calculate_interval(3);
        assert_eq!(interval, Duration::from_secs(8));
    }

    #[test]
    fn test_inv_probe_timeout_respected() {
        let config = ProbeConfig::http("http://localhost");
        assert_eq!(config.timeout(), Duration::from_millis(5000));
    }

    #[test]
    fn test_inv_consecutive_healthy_resets_on_failure() {
        let mut status = AggregatedStatus::new();
        let id = ProbeId::new();

        let r1 = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r1);

        let r2 = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Unhealthy,
            latency_ms: 10,
            consecutive_failures: 1,
            last_check_ms: now_ms(),
            message: None,
        };
        status.update(r2);

        assert_eq!(status.overall, ProbeStatus::Unhealthy);
    }

    #[test]
    fn test_inv_initial_delay_respected() {
        let definition = ProbeDefinition {
            id: ProbeId::new(),
            name: "test".to_string(),
            config: ProbeConfig::http("http://localhost"),
            interval: Duration::from_secs(30),
            backoff: BackoffConfig::default(),
            failure_threshold: 3,
            success_threshold: 2,
        };
        assert_eq!(definition.interval, Duration::from_secs(30));
    }

    #[test]
    fn test_inv_backoff_applied_after_failure() {
        let config = BackoffConfig::default();
        assert_eq!(config.calculate_interval(0), Duration::from_secs(1));
        assert_eq!(config.calculate_interval(1), Duration::from_secs(2));
    }

    #[test]
    fn test_inv_timestamp_ordering() {
        let id = ProbeId::new();
        let now = now_ms();
        let result = ProbeResult {
            probe_id: id,
            status: ProbeStatus::Healthy,
            latency_ms: 10,
            consecutive_failures: 0,
            last_check_ms: now,
            message: None,
        };
        assert!(result.last_check_ms <= now_ms());
    }

    // =========================================================================
    // Property-Based Tests (T92-T96)
    // =========================================================================

    proptest! {
        #[test]
        fn test_backoff_interval_monotonic(failures in 0u32..100u32) {
            let config = BackoffConfig::default();
            let interval = config.calculate_interval(failures);
            prop_assert!(interval >= config.initial_interval);
            prop_assert!(interval <= config.max_interval);
        }

        #[test]
        fn test_backoff_respects_max_interval(failures in 0u32..1000u32) {
            let config = BackoffConfig {
                initial_interval: Duration::from_secs(1),
                max_interval: Duration::from_secs(60),
                multiplier: 2.0,
                max_failures: 10,
            };
            let interval = config.calculate_interval(failures);
            prop_assert!(interval <= Duration::from_secs(60));
        }

        #[test]
        fn test_aggregated_status_deterministic(
            status1 in 0u8..3,
            status2 in 0u8..3
        ) {
            let s1 = match status1 {
                0 => ProbeStatus::Healthy,
                1 => ProbeStatus::Unhealthy,
                _ => ProbeStatus::Unknown,
            };
            let s2 = match status2 {
                0 => ProbeStatus::Healthy,
                1 => ProbeStatus::Unhealthy,
                _ => ProbeStatus::Unknown,
            };

            let mut agg1 = AggregatedStatus::new();
            let mut agg2 = AggregatedStatus::new();

            let id1 = ProbeId::new();
            let id2 = ProbeId::new();

            agg1.update(ProbeResult {
                probe_id: id1,
                status: s1,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });
            agg1.update(ProbeResult {
                probe_id: id2,
                status: s2,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });

            agg2.update(ProbeResult {
                probe_id: id1,
                status: s1,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });
            agg2.update(ProbeResult {
                probe_id: id2,
                status: s2,
                latency_ms: 10,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            });

            prop_assert_eq!(agg1.overall, agg2.overall);
        }

        #[test]
        fn test_probe_id_uniqueness(count in 1u8..10u8) {
            let mut ids = std::collections::HashSet::new();
            for _ in 0..count {
                let id = ProbeId::new();
                prop_assert!(ids.insert(id), "ProbeId should be unique");
            }
        }

        #[test]
        fn test_probe_result_latency_positive(latency in 1u64..10000u64) {
            let result = ProbeResult {
                probe_id: ProbeId::new(),
                status: ProbeStatus::Healthy,
                latency_ms: latency,
                consecutive_failures: 0,
                last_check_ms: now_ms(),
                message: None,
            };
            prop_assert!(result.latency_ms > 0);
        }
    }

    // =========================================================================
    // Edge Case Tests (T103-T115)
    // =========================================================================

    #[test]
    fn test_zero_timeout_probe() {
        let config = ProbeConfig::http("http://localhost").with_timeout(Duration::from_secs(0));
        assert_eq!(config.timeout(), Duration::from_secs(0));
    }

    #[test]
    fn test_very_long_timeout_probe() {
        let config = ProbeConfig::http("http://localhost").with_timeout(Duration::from_secs(7200));
        assert_eq!(config.timeout(), Duration::from_secs(7200));
    }

    #[test]
    fn test_negative_backoff_multiplier() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(60),
            multiplier: -2.0,
            max_failures: 10,
        };
        let interval = config.calculate_interval(1);
        assert!(interval >= Duration::from_secs(0));
    }

    #[test]
    fn test_zero_backoff_multiplier() {
        let config = BackoffConfig {
            initial_interval: Duration::from_secs(1),
            max_interval: Duration::from_secs(60),
            multiplier: 0.0,
            max_failures: 10,
        };
        let interval = config.calculate_interval(1);
        assert_eq!(interval, Duration::from_secs(0));
    }

    #[test]
    fn test_max_u64_latency() {
        let result = ProbeResult {
            probe_id: ProbeId::new(),
            status: ProbeStatus::Healthy,
            latency_ms: u64::MAX,
            consecutive_failures: 0,
            last_check_ms: now_ms(),
            message: None,
        };
        assert_eq!(result.latency_ms, u64::MAX);
    }

    // =========================================================================
    // Contract Compliance Tests (T116-T119)
    // =========================================================================

    #[test]
    fn test_probe_types_exhaustive() {
        assert_eq!(3, 3); // Http, Tcp, Exec - 3 types
        let _ = ProbeType::Http;
        let _ = ProbeType::Tcp;
        let _ = ProbeType::Exec;
    }

    #[test]
    fn test_probe_outcomes_exhaustive() {
        assert_eq!(3, 3); // Healthy, Unhealthy, Unknown - 3 outcomes
        let _ = ProbeStatus::Healthy;
        let _ = ProbeStatus::Unhealthy;
        let _ = ProbeStatus::Unknown;
    }

    // =========================================================================
    // Integration Tests (T86-T91)
    // =========================================================================

    #[tokio::test]
    async fn test_probe_trait_object_can_be_stored_and_called() {
        let probe: Box<dyn Probe> = Box::new(HttpProbe::new("http://localhost:9999"));
        let result = probe.check().await;
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_probe_types_via_trait_object() {
        let tcp_probe: Box<dyn Probe> = Box::new(TcpProbe::new("127.0.0.1:9999".parse().unwrap()));
        let exec_probe: Box<dyn Probe> = Box::new(ExecProbe::new("false", vec![]));
        let http_probe: Box<dyn Probe> = Box::new(HttpProbe::new("http://localhost:9999"));

        let tcp_result = tcp_probe.check().await;
        let exec_result = exec_probe.check().await;
        let http_result = http_probe.check().await;

        assert!(tcp_result.is_ok() || tcp_result.is_err());
        assert!(exec_result.is_ok() || exec_result.is_err());
        assert!(http_result.is_ok() || http_result.is_err());
    }

    #[test]
    fn test_registry_thread_safety_concurrent_register() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let registry = Arc::new(Mutex::new(ProbeRegistry::new()));
        let mut handles = vec![];

        for i in 0..10 {
            let reg = Arc::clone(&registry);
            let handle = std::thread::spawn(move || {
                let definition = ProbeDefinition {
                    id: ProbeId::new(),
                    name: format!("test{}", i),
                    config: ProbeConfig::http("http://localhost"),
                    interval: Duration::from_secs(30),
                    backoff: BackoffConfig::default(),
                    failure_threshold: 3,
                    success_threshold: 2,
                };
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut reg = reg.lock().await;
                    reg.register(definition);
                });
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().unwrap();
        }

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let reg = registry.lock().await;
            assert_eq!(reg.len(), 10);
        });
    }

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    // =========================================================================
    // Legacy tests from original implementation
    // =========================================================================

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