use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::types::{Probe, ProbeError, ProbeId, ProbeResult, ProbeStatus};

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
mod tests {
    use super::*;

    #[tokio::test]
    async fn qa_smoke_exec_probe_true_command() {
        let probe = ExecProbe::new("true", vec![]);
        let result = probe.check().await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.status, ProbeStatus::Healthy);
        assert_eq!(r.probe_id, probe.probe_id());
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_false_command() {
        let probe = ExecProbe::new("false", vec![]);
        let result = probe.check().await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.status, ProbeStatus::Unhealthy);
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_custom_exit_code() {
        let probe = ExecProbe::new("bash", vec!["-c".to_string(), "exit 42".to_string()])
            .with_expected_exit_code(42);
        let result = probe.check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ProbeStatus::Healthy);
    }

    #[tokio::test]
    async fn qa_smoke_exec_probe_nonexistent_command() {
        let probe = ExecProbe::new("/nonexistent/command/xyz", vec![]);
        let result = probe.check().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn qa_smoke_probe_result_fields_populated() {
        let probe = ExecProbe::new("echo", vec!["hello".to_string()]);
        let result = probe.check().await.unwrap();
        assert_eq!(result.status, ProbeStatus::Healthy);
        assert!(result.message.is_some());
        assert!(result.message.as_ref().unwrap().contains("echo"));
        assert!(result.last_check_ms > 0);
        assert_eq!(result.probe_id, probe.probe_id());
    }
}
