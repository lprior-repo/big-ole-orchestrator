//! Exec probe implementation.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::error::ProbeError;
use super::types::{ProbeId, ProbeResult, ProbeStatus};
use super::Probe;

/// Exec health probe.
pub struct ExecProbe {
    id: ProbeId,
    command: String,
    args: Vec<String>,
    expected_exit_code: Option<i32>,
    timeout: Duration,
}

impl ExecProbe {
    /// Create a new exec probe.
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            id: ProbeId::new(),
            command: command.into(),
            args,
            expected_exit_code: Some(0),
            timeout: Duration::from_secs(30),
        }
    }

    /// Set expected exit code.
    pub fn with_expected_exit_code(mut self, code: i32) -> Self {
        self.expected_exit_code = Some(code);
        self
    }

    /// Set probe timeout.
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
