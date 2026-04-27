use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Enums
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

// Data Structures
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub probe_id: ProbeId,
    pub status: ProbeStatus,
    pub latency_ms: u64,
    pub consecutive_failures: u32,
    pub last_check_ms: u64,
    pub message: Option<String>,
}

// ProbeConfig enum
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
            Self::Http { timeout_ms, .. }
            | Self::Tcp { timeout_ms, .. }
            | Self::Exec { timeout_ms, .. } => Duration::from_millis(*timeout_ms),
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

// ProbeId
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

impl<'de> Deserialize<'de> for ProbeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ProbeId::from_string(&s).ok_or_else(|| serde::de::Error::custom("Invalid probe ID format"))
    }
}

// ProbeDefinition
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

// BackoffConfig
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
        let interval_ms =
            self.initial_interval.as_millis() as f64 * self.multiplier.powi(failures as i32);
        let interval_ms = interval_ms.min(self.max_interval.as_millis() as f64);
        Duration::from_millis(interval_ms as u64)
    }
}

// AggregatedStatus
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
    pub fn is_healthy(&self) -> bool {
        self.overall == ProbeStatus::Healthy
    }
}

impl Default for AggregatedStatus {
    fn default() -> Self {
        Self::new()
    }
}

// ProbeError
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

// ProbeRegistry
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
