use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::config::ProbeDefinition;
use super::metrics::{Probe, ProbeError, ProbeId, ProbeResult, ProbeStatus};

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
    use crate::probe::config::{BackoffConfig, ProbeConfig};
    use crate::probe::metrics::ProbeStatus;

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
}
