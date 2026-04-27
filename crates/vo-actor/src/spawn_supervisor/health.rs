//! Spawn supervisor health check implementation.
//!
//! Handles multi-step health probing for spawned processes.

use std::time::Duration;

use super::traits::ProcessHandle;
use super::types::SpawnSupervisorError;
use super::Actor;
use super::SpawnSupervisorMetrics;
use vo_types::InstanceId;

impl Actor {
    /// Performs health checks on a process.
    pub(super) async fn perform_health_checks(
        &self,
        instance_id: &InstanceId,
        process: &ProcessHandle,
    ) -> Result<(), SpawnSupervisorError> {
        for i in 1..=self.max_health_checks {
            self.metrics.health_checks_performed.incr();

            tokio::time::sleep(Duration::from_millis(100)).await;

            match self.process_manager.check_health(process.pid).await {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    if i < self.max_health_checks {
                        continue;
                    }
                }
                Err(e) => {
                    return Err(SpawnSupervisorError::HealthCheckFailed {
                        instance_id: instance_id.clone(),
                        check_number: i,
                        error: e.to_string(),
                    });
                }
            }
        }

        Err(SpawnSupervisorError::HealthCheckFailed {
            instance_id: instance_id.clone(),
            check_number: self.max_health_checks,
            error: "Max health checks exceeded".to_string(),
        })
    }
}
