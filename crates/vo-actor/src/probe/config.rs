use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::metrics::ProbeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeType {
    Http,
    Tcp,
    Exec,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_probe_config_http_creates_valid_config() {
        let config = ProbeConfig::http("http://localhost:8080/health");
        match config {
            ProbeConfig::Http {
                url,
                expected_status,
                timeout_ms,
            } => {
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
            ProbeConfig::Tcp {
                address,
                port,
                timeout_ms,
            } => {
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
            ProbeConfig::Exec {
                command,
                args,
                expected_exit_code,
                timeout_ms,
            } => {
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
        let config =
            ProbeConfig::http("http://localhost:8080/health").with_timeout(Duration::from_secs(30));
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
        assert_eq!(
            ProbeConfig::http("http://localhost").probe_type(),
            ProbeType::Http
        );
        assert_eq!(
            ProbeConfig::tcp("localhost", 8080).probe_type(),
            ProbeType::Tcp
        );
        assert_eq!(
            ProbeConfig::exec("echo", vec![]).probe_type(),
            ProbeType::Exec
        );
    }

    #[test]
    fn test_probe_config_serialization_roundtrip() {
        let config =
            ProbeConfig::http("http://localhost:8080/health").with_timeout(Duration::from_secs(10));
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProbeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.timeout(), config.timeout());
    }

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
}
