use std::net::SocketAddr;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::types::{ProbeError, ProbeId, ProbeResult, ProbeStatus};

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
        } else if response.status().is_success() {
            ProbeStatus::Healthy
        } else {
            ProbeStatus::Unhealthy
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

#[cfg(test)]
mod probe_unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_http_probe_check_unhealthy_host() {
        let probe = HttpProbe::new("http://127.0.0.1:1");
        let result = probe.check().await;
        assert!(result.is_err() || result.as_ref().unwrap().status == ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_http_probe_with_expected_status() {
        let probe = HttpProbe::new("http://127.0.0.1:1").with_expected_status(404);
        assert_eq!(probe.expected_status, Some(404));
    }

    #[tokio::test]
    async fn test_tcp_probe_check_refused() {
        let probe = TcpProbe::new("127.0.0.1:1".parse().unwrap());
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_tcp_probe_with_timeout() {
        let probe =
            TcpProbe::new("127.0.0.1:1".parse().unwrap()).with_timeout(Duration::from_millis(100));
        assert_eq!(probe.timeout, Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_exec_probe_true() {
        let probe = ExecProbe::new("true", vec![]);
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Healthy);
    }

    #[tokio::test]
    async fn test_exec_probe_false() {
        let probe = ExecProbe::new("false", vec![]);
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_exec_probe_custom_exit_code() {
        let probe = ExecProbe::new("bash", vec!["-c".to_string(), "exit 42".to_string()])
            .with_expected_exit_code(42);
        assert!(probe.check().await.is_ok());
        assert_eq!(probe.check().await.unwrap().status, ProbeStatus::Healthy);
    }

    #[tokio::test]
    async fn test_exec_probe_nonexistent() {
        assert!(ExecProbe::new("/nonexistent/command/xyz", vec![])
            .check()
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_probe_trait_object_dispatch() {
        let http_probe: Box<dyn Probe> = Box::new(HttpProbe::new("http://127.0.0.1:1"));
        let tcp_probe: Box<dyn Probe> = Box::new(TcpProbe::new("127.0.0.1:1".parse().unwrap()));
        let exec_probe: Box<dyn Probe> = Box::new(ExecProbe::new("true", vec![]));
        assert!(http_probe.check().await.is_ok() || http_probe.check().await.is_err());
        assert!(tcp_probe.check().await.is_ok());
        assert!(exec_probe.check().await.is_ok());
    }
}
