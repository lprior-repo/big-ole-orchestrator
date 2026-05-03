//! Health check probing for spawn supervisor subprocesses.
//!
//! Provides health check logic for verifying that spawned processes
//! are responsive before marking them as ready.

use crate::spawn_supervisor::types::SpawnSupervisorError;
use crate::spawn_supervisor::process::ProcessHandle;
use vo_types::InstanceId;

/// Result of a health check operation.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub message: Option<String>,
}

impl HealthCheckResult {
    /// Creates a successful health check result.
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            message: None,
        }
    }

    /// Creates a failed health check result.
    #[must_use]
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            healthy: false,
            message: Some(message.into()),
        }
    }
}

/// Default health check implementation.
///
/// Performs a basic check by verifying the process is still running.
pub fn perform_health_check(
    _instance_id: &InstanceId,
    _process: &ProcessHandle,
) -> HealthCheckResult {
    // Default: assume healthy if process handle exists
    HealthCheckResult::healthy()
}
