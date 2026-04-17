//! Probe types and configuration for health checking.

use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Types of health probes supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeType {
    Http,
    Tcp,
    Exec,
}

/// Status of a probe result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeStatus {
    Healthy,
    Unhealthy,
    Unknown,
}

/// Result of a single probe check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub probe_id: ProbeId,
    pub status: ProbeStatus,
    pub latency_ms: u64,
    pub consecutive_failures: u32,
    pub last_check_ms: u64,
    pub message: Option<String>,
}

/// HTTP-specific probe configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbeConfig {
    pub url: String,
    pub expected_status: Option<u16>,
    pub timeout: Duration,
}

/// TCP-specific probe configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpProbeConfig {
    pub address: String,
    pub port: u16,
    pub timeout: Duration,
}

/// Exec-specific probe configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecProbeConfig {
    pub command: String,
    pub args: Vec<String>,
    pub expected_exit_code: Option<i32>,
    pub timeout: Duration,
}

/// Combined probe configuration with tagged enum for different types.
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
    /// Create HTTP probe config.
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            expected_status: Some(200),
            timeout_ms: 5000,
        }
    }

    /// Create TCP probe config.
    pub fn tcp(address: impl Into<String>, port: u16) -> Self {
        Self::Tcp {
            address: address.into(),
            port,
            timeout_ms: 5000,
        }
    }

    /// Create Exec probe config.
    pub fn exec(command: impl Into<String>, args: Vec<String>) -> Self {
        Self::Exec {
            command: command.into(),
            args,
            expected_exit_code: Some(0),
            timeout_ms: 30000,
        }
    }

    /// Set timeout for the config.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        match &mut self {
            Self::Http { timeout_ms, .. } => *timeout_ms = timeout.as_millis() as u64,
            Self::Tcp { timeout_ms, .. } => *timeout_ms = timeout.as_millis() as u64,
            Self::Exec { timeout_ms, .. } => *timeout_ms = timeout.as_millis() as u64,
        }
        self
    }

    /// Get timeout duration.
    pub fn timeout(&self) -> Duration {
        match self {
            Self::Http { timeout_ms, .. }
            | Self::Tcp { timeout_ms, .. }
            | Self::Exec { timeout_ms, .. } => Duration::from_millis(*timeout_ms),
        }
    }

    /// Get the probe type.
    pub fn probe_type(&self) -> ProbeType {
        match self {
            Self::Http { .. } => ProbeType::Http,
            Self::Tcp { .. } => ProbeType::Tcp,
            Self::Exec { .. } => ProbeType::Exec,
        }
    }
}

/// Unique identifier for a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProbeId(pub ulid::Ulid);

impl ProbeId {
    /// Generate a new unique probe ID.
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }

    /// Parse from string with "probe-" prefix.
    pub fn from_string(s: &str) -> Option<Self> {
        s.strip_prefix("probe-")
            .and_then(|s| ulid::Ulid::from_string(s).ok())
            .map(Self)
    }

    /// Convert to string representation.
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
        ProbeId::from_string(&s).ok_or_else(|| serde::de::Error::custom("Invalid probe ID format"))
    }
}

/// Configuration for backoff behavior.
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
    /// Calculate backoff interval based on consecutive failures.
    pub fn calculate_interval(&self, consecutive_failures: u32) -> Duration {
        let failures = consecutive_failures.min(self.max_failures);
        let interval_ms =
            self.initial_interval.as_millis() as f64 * self.multiplier.powi(failures as i32);
        let interval_ms = interval_ms.min(self.max_interval.as_millis() as f64);
        Duration::from_millis(interval_ms as u64)
    }
}

/// Complete probe definition with all configuration.
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

/// Aggregated status from multiple probes.
#[derive(Debug, Clone)]
pub struct AggregatedStatus {
    pub overall: ProbeStatus,
    pub healthy_count: u32,
    pub unhealthy_count: u32,
    pub unknown_count: u32,
    pub results: std::collections::HashMap<ProbeId, ProbeResult>,
}

impl AggregatedStatus {
    /// Create new empty aggregated status.
    pub fn new() -> Self {
        Self {
            overall: ProbeStatus::Unknown,
            healthy_count: 0,
            unhealthy_count: 0,
            unknown_count: 0,
            results: std::collections::HashMap::new(),
        }
    }

    /// Update aggregated status with a new result.
    pub fn update(&mut self, result: ProbeResult) {
        if let Some(old_result) = self.results.get(&result.probe_id) {
            match old_result.status {
                ProbeStatus::Healthy => self.healthy_count -= 1,
                ProbeStatus::Unhealthy => self.unhealthy_count -= 1,
                ProbeStatus::Unknown => self.unknown_count -= 1,
            }
        }
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

    /// Check if overall status is healthy.
    pub fn is_healthy(&self) -> bool {
        self.overall == ProbeStatus::Healthy
    }
}

impl Default for AggregatedStatus {
    fn default() -> Self {
        Self::new()
    }
}
