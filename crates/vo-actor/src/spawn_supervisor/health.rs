//! Spawn supervisor health check implementation.
//!
//! Handles multi-step health probing for spawned processes.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::process::ProcessHandle;
use super::types::SpawnSupervisorError;
use super::SpawnSupervisor as Actor;
use super::{ProcessManager, SpawnSupervisorMetrics, SpawnStorage, WorkQueue};
use super::{CycleResult, ExecutionSemaphore, ProcessHandle, SpawnPhase, SpawnRecord};
use vo_types::InstanceId;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use ulid::Ulid;

    fn test_instance_id() -> InstanceId {
        let ulid = Ulid::new();
        InstanceId::from_bytes(ulid.to_bytes())
    }

    #[derive(Debug, Default)]
    struct MockStorage;

    #[async_trait::async_trait]
    impl SpawnStorage for MockStorage {
        async fn get_spawn_record(
            &self,
            _instance_id: &InstanceId,
        ) -> Option<SpawnRecord> {
            None
        }
        async fn save_spawn_record(
            &self,
            _record: &SpawnRecord,
        ) -> Result<(), SpawnSupervisorError> {
            Ok(())
        }
        async fn delete_spawn_record(
            &self,
            _instance_id: &InstanceId,
        ) -> Result<(), SpawnSupervisorError> {
            Ok(())
        }
        async fn scan_spawns_by_phase(
            &self,
            _phase: SpawnPhase,
            _max: u32,
        ) -> Vec<SpawnRecord> {
            vec![]
        }
        async fn transition_phase(
            &self,
            _instance_id: &InstanceId,
            _new_phase: SpawnPhase,
        ) -> Result<(), SpawnSupervisorError> {
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MockWorkQueue;

    #[async_trait::async_trait]
    impl WorkQueue for MockWorkQueue {
        async fn enqueue_spawn(
            &self,
            _instance_id: InstanceId,
            _executable: PathBuf,
            _args: Vec<String>,
        ) -> Result<(), SpawnSupervisorError> {
            Ok(())
        }
        async fn enqueue_resume(
            &self,
            _instance_id: InstanceId,
        ) -> Result<(), SpawnSupervisorError> {
            Ok(())
        }
    }

    struct MockProcessManager {
        health_check_results: Vec<Result<bool, SpawnSupervisorError>>,
        zombie_check_results: Vec<Result<bool, SpawnSupervisorError>>,
        call_counter: AtomicU32,
    }

    impl MockProcessManager {
        fn new(
            health_check_results: Vec<Result<bool, SpawnSupervisorError>>,
            zombie_check_results: Vec<Result<bool, SpawnSupervisorError>>,
        ) -> Self {
            Self {
                health_check_results,
                zombie_check_results,
                call_counter: AtomicU32::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl ProcessManager for MockProcessManager {
        async fn spawn_process(
            &self,
            _executable: &std::path::Path,
            _args: &[String],
        ) -> Result<ProcessHandle, SpawnSupervisorError> {
            Ok(ProcessHandle::new(1, PathBuf::from("test"), vec![]))
        }
        async fn check_health(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
            let idx = self.call_counter.fetch_add(1, Ordering::SeqCst) as usize;
            self.health_check_results
                .get(idx)
                .cloned()
                .unwrap_or(Ok(true))
        }
        async fn is_zombie(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
            let idx = self.call_counter.fetch_add(1, Ordering::SeqCst) as usize;
            self.zombie_check_results
                .get(idx)
                .cloned()
                .unwrap_or(Ok(false))
        }
        async fn terminate(&self, _pid: u32) -> Result<(), SpawnSupervisorError> {
            Ok(())
        }
        async fn wait(&self, _pid: u32) -> Result<i32, SpawnSupervisorError> {
            Ok(0)
        }
    }

    fn create_supervisor(
        pm: Arc<dyn ProcessManager>,
        max_health_checks: u32,
    ) -> Actor {
        Actor {
            health_check_interval: Duration::from_millis(1),
            max_health_checks,
            initial_backoff: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_spawn_attempts: 5,
            storage: Arc::new(MockStorage),
            process_manager: pm,
            work_queue: Arc::new(MockWorkQueue),
            metrics: SpawnSupervisorMetrics::default(),
            execution_semaphore: Arc::new(ExecutionSemaphore::default()),
        }
    }

    #[tokio::test]
    async fn health_check_passes_on_first_attempt() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(true)],
            vec![Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_ok());
        assert_eq!(pm.health_check_results.len(), 1);
    }

    #[tokio::test]
    async fn health_check_fails_after_max_retries() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false), Ok(false), Ok(false)],
            vec![Ok(false), Ok(false), Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SpawnSupervisorError::HealthCheckFailed {
                check_number: 3,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn health_check_succeeds_on_second_attempt() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false), Ok(true)],
            vec![Ok(false), Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn health_check_error_returns_health_check_failed() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Err(SpawnSupervisorError::DispatchError("test error".to_string()))],
            vec![Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SpawnSupervisorError::HealthCheckFailed {
                check_number: 1,
                error,
                ..
            } if error.contains("test error")
        ));
    }

    #[tokio::test]
    async fn zombie_detected_on_health_check_failure() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false)],
            vec![Ok(true)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SpawnSupervisorError::ZombieDetected { pid: 123, .. }
        ));
    }

    #[tokio::test]
    async fn zombie_detected_on_health_check_error() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Err(SpawnSupervisorError::DispatchError("test".to_string()))],
            vec![Ok(true)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(456, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SpawnSupervisorError::ZombieDetected { pid: 456, .. }
        ));
    }

    #[tokio::test]
    async fn zombie_detection_increments_metric() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false)],
            vec![Ok(true)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        assert_eq!(supervisor.metrics.zombies_detected.get(), 0);

        supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await
            .unwrap_err();

        assert_eq!(supervisor.metrics.zombies_detected.get(), 1);
    }

    #[tokio::test]
    async fn health_checks_performed_metric_increments() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false), Ok(false), Ok(true)],
            vec![Ok(false), Ok(false), Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 3);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        assert_eq!(supervisor.metrics.health_checks_performed.get(), 0);

        supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await
            .unwrap();

        assert_eq!(supervisor.metrics.health_checks_performed.get(), 2);
    }

    #[tokio::test]
    async fn max_health_checks_exceeded_error() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false); 5],
            vec![Ok(false); 5],
        ));
        let supervisor = create_supervisor(pm.clone(), 5);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SpawnSupervisorError::HealthCheckFailed {
                check_number: 5,
                error,
                ..
            } if error == "Max health checks exceeded"
        ));
    }

    #[tokio::test]
    async fn health_check_with_single_max() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(false)],
            vec![Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 1);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            SpawnSupervisorError::HealthCheckFailed {
                check_number: 1,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn health_check_with_single_max_passes() {
        let pm = Arc::new(MockProcessManager::new(
            vec![Ok(true)],
            vec![Ok(false)],
        ));
        let supervisor = create_supervisor(pm.clone(), 1);
        let process = ProcessHandle::new(123, PathBuf::from("/bin/true"), vec![]);

        let result = supervisor
            .perform_health_checks(&test_instance_id(), &process)
            .await;

        assert!(result.is_ok());
    }
}

impl Actor {
    /// Performs health checks on a process.
    ///
    /// Per ADR-046:
    /// - Performs up to `max_health_checks` checks spaced by health_check_interval
    /// - If health check fails, also checks if process is zombie via `is_zombie`
    /// - If zombie detected, increments `zombies_detected` metric and returns `ZombieDetected` error
    /// - If all checks pass, transitions to Running
    /// - If checks exhausted without zombie, returns `HealthCheckFailed` error
    pub(super) async fn perform_health_checks(
        &self,
        instance_id: &InstanceId,
        process: &ProcessHandle,
    ) -> Result<(), SpawnSupervisorError> {
        for i in 1..=self.max_health_checks {
            self.metrics.health_checks_performed.incr();

            tokio::time::sleep(self.health_check_interval).await;

            match self.process_manager.check_health(process.pid).await {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    if let Ok(true) = self.process_manager.is_zombie(process.pid).await {
                        self.metrics.zombies_detected.incr();
                        return Err(SpawnSupervisorError::ZombieDetected {
                            instance_id: instance_id.clone(),
                            pid: process.pid,
                        });
                    }
                    if i < self.max_health_checks {
                        continue;
                    }
                }
                Err(e) => {
                    if let Ok(true) = self.process_manager.is_zombie(process.pid).await {
                        self.metrics.zombies_detected.incr();
                        return Err(SpawnSupervisorError::ZombieDetected {
                            instance_id: instance_id.clone(),
                            pid: process.pid,
                        });
                    }
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
