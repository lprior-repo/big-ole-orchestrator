use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
        ProbeId::from_string(&s).ok_or_else(|| serde::de::Error::custom("Invalid probe ID format"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
        assert!(
            s.starts_with("probe-"),
            "as_str() should return probe-<ULID> format"
        );
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
        assert!(
            result.latency_ms > 0,
            "Completed probe should have non-zero latency"
        );
    }

    #[test]
    fn test_probe_error_http_message_format() {
        let err = ProbeError::Http("connection refused".to_string());
        assert!(matches!(err, ProbeError::Http(ref msg) if msg == "connection refused"));
        assert!(err.to_string().contains("HTTP probe failed"));
    }

    #[test]
    fn test_probe_error_tcp_message_format() {
        let err = ProbeError::Tcp("connection refused".to_string());
        assert!(matches!(err, ProbeError::Tcp(ref msg) if msg == "connection refused"));
        assert!(err.to_string().contains("TCP probe failed"));
    }

    #[test]
    fn test_probe_error_exec_message_format() {
        let err = ProbeError::Exec("exit code 1".to_string());
        assert!(matches!(err, ProbeError::Exec(ref msg) if msg == "exit code 1"));
        assert!(err.to_string().contains("Exec probe failed"));
    }

    #[test]
    fn test_probe_error_timeout_message_format() {
        let err = ProbeError::Timeout(Duration::from_secs(5));
        assert!(matches!(err, ProbeError::Timeout(d) if d == Duration::from_secs(5)));
        assert!(err.to_string().contains("Timeout"));
    }

    #[test]
    fn test_probe_error_not_found_message_format() {
        let id = ProbeId::new();
        let err = ProbeError::NotFound(id);
        assert!(matches!(err, ProbeError::NotFound(_)));
        let msg = err.to_string();
        assert!(msg.contains("not found") || msg.contains("Probe"));
    }
}
