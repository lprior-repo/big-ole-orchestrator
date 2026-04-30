//! SpawnSupervisor-specific tests.
//!
//! Tests for SpawnSupervisor struct behavior (constructor, Debug impl, Send/Sync bounds).
//! Pure function tests are in `pure.rs`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ulid::Ulid;
use vo_types::InstanceId;

use super::actor::{SpawnSupervisor, SpawnSupervisorHandle};
use super::types::{SpawnSupervisorError, SpawnSupervisorState};
use super::{
    Counter, CycleResult, ExecutionSemaphore, ProcessHandle, ProcessManager, SpawnPhase,
    SpawnRecord, SpawnStorage, SpawnSupervisorMetrics, WorkQueue,
};
use crate::lifecycle::ShutdownPropagator;

fn test_instance_id() -> InstanceId {
    let ulid = Ulid::new();
    InstanceId::from_bytes(ulid.to_bytes())
}

// =============================================================================
// SpawnSupervisor Configuration Validation Tests
// =============================================================================

#[test]
fn spawn_supervisor_rejects_zero_health_check_interval() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let result = SpawnSupervisor::new(
        Duration::ZERO,
        3,
        Duration::from_millis(100),
        2.0,
        0.5,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        Arc::new(ShutdownPropagator::default_propagator()),
    );

    assert!(matches!(
        result,
        Err(SpawnSupervisorError::InvalidConfig(_))
    ));
}

#[test]
fn spawn_supervisor_rejects_zero_max_health_checks() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        0,
        Duration::from_millis(100),
        2.0,
        0.5,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        Arc::new(ShutdownPropagator::default_propagator()),
    );

    assert!(matches!(
        result,
        Err(SpawnSupervisorError::InvalidConfig(_))
    ));
}

#[test]
fn spawn_supervisor_rejects_zero_initial_backoff() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::ZERO,
        2.0,
        0.5,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        Arc::new(ShutdownPropagator::default_propagator()),
    );

    assert!(matches!(
        result,
        Err(SpawnSupervisorError::InvalidConfig(_))
    ));
}

#[test]
fn spawn_supervisor_rejects_backoff_multiplier_less_than_one() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(100),
        0.5,
        0.5,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        Arc::new(ShutdownPropagator::default_propagator()),
    );

    assert!(matches!(
        result,
        Err(SpawnSupervisorError::InvalidConfig(_))
    ));
}

#[test]
fn spawn_supervisor_rejects_zero_jitter_factor() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(100),
        2.0,
        0.0,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
    );

    assert!(matches!(
        result,
        Err(SpawnSupervisorError::InvalidConfig(msg)) if msg.contains("jitter_factor")
    ));
}

#[test]
fn spawn_supervisor_rejects_jitter_factor_greater_than_one() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(100),
        2.0,
        1.1,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
    );

    assert!(matches!(
        result,
        Err(SpawnSupervisorError::InvalidConfig(msg)) if msg.contains("jitter_factor")
    ));
}

// =============================================================================
// SpawnSupervisor Construction Tests
// =============================================================================

#[test]
fn spawn_supervisor_constructs_with_valid_config() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let supervisor = SpawnSupervisor::new(
        Duration::from_secs(10),
        3,
        Duration::from_millis(100),
        2.0,
        0.5,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        Arc::new(ShutdownPropagator::default_propagator()),
    );

    assert!(supervisor.is_ok());
    let supervisor = supervisor.unwrap();
    assert_eq!(supervisor.health_check_interval, Duration::from_secs(10));
    assert_eq!(supervisor.max_health_checks, 3);
    assert_eq!(supervisor.initial_backoff, Duration::from_millis(100));
    assert_eq!(supervisor.backoff_multiplier, 2.0);
    assert_eq!(supervisor.jitter_factor, 0.5);
    assert_eq!(supervisor.max_spawn_attempts, 5);
}

// =============================================================================
// SpawnSupervisor Debug Format Tests
// =============================================================================

#[test]
fn spawn_supervisor_debug_format() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;

    let supervisor = SpawnSupervisor::new(
        Duration::from_secs(10),
        3,
        Duration::from_millis(100),
        2.0,
        0.5,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        Arc::new(ShutdownPropagator::default_propagator()),
    )
    .unwrap();

    let debug_str = format!("{:?}", supervisor);
    assert!(debug_str.contains("SpawnSupervisor"));
    assert!(debug_str.contains("health_check_interval"));
    assert!(debug_str.contains("max_health_checks"));
    assert!(debug_str.contains("jitter_factor"));
}

// =============================================================================
// SpawnSupervisor Handle Debug Format Tests
// =============================================================================

#[test]
fn spawn_supervisor_handle_debug_format() {
    let handle = SpawnSupervisorHandle {
        state_sender: tokio::sync::watch::channel(SpawnSupervisorState::Stopped).0,
        shutdown_trigger: tokio::sync::broadcast::channel(1).0,
        task_handle: None,
        shutdown_propagator: Arc::new(ShutdownPropagator::default_propagator()),
    };

    let debug_str = format!("{:?}", handle);
    assert!(debug_str.contains("SpawnSupervisorHandle"));
}

// =============================================================================
// SpawnSupervisor Send/Sync Bounds Tests
// =============================================================================

#[test]
fn spawn_supervisor_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<SpawnSupervisor>();
}

// =============================================================================
// SpawnSupervisor Shutdown Propagator Registration Tests
// =============================================================================

#[test]
fn spawn_supervisor_registers_cleanup_with_propagator() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;
    let propagator = Arc::new(ShutdownPropagator::default_propagator());

    let _supervisor = SpawnSupervisor::new(
        Duration::from_secs(10),
        3,
        Duration::from_millis(100),
        2.0,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        propagator.clone(),
    )
    .unwrap();

    // Propagator should exist and be usable
    assert!(Arc::strong_count(&propagator) >= 2);
}

#[test]
fn spawn_supervisor_handle_propagator_shares_reference() {
    let storage = MockStorage;
    let pm = MockProcessManager;
    let wq = MockWorkQueue;
    let propagator = Arc::new(ShutdownPropagator::default_propagator());

    let supervisor = SpawnSupervisor::new(
        Duration::from_secs(10),
        3,
        Duration::from_millis(100),
        2.0,
        5,
        Arc::new(storage),
        Arc::new(pm),
        Arc::new(wq),
        Arc::new(ExecutionSemaphore::default()),
        propagator.clone(),
    )
    .unwrap();

    let handle = supervisor.spawn().unwrap();

    // Handle should share the same propagator reference
    assert_eq!(Arc::strong_count(&propagator), 2);
}

// =============================================================================
// Mock Implementations for SpawnSupervisor Tests
// =============================================================================

#[derive(Debug, Default)]
struct MockStorage;

#[async_trait::async_trait]
impl SpawnStorage for MockStorage {
    async fn get_spawn_record(&self, _instance_id: &InstanceId) -> Option<SpawnRecord> {
        None
    }
    async fn save_spawn_record(&self, _record: &SpawnRecord) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }
    async fn delete_spawn_record(
        &self,
        _instance_id: &InstanceId,
    ) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }
    async fn scan_spawns_by_phase(&self, _phase: SpawnPhase, _max: u32) -> Vec<SpawnRecord> {
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
struct MockProcessManager;

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
        Ok(true)
    }
    async fn is_zombie(&self, _pid: u32) -> Result<bool, SpawnSupervisorError> {
        Ok(false)
    }
    async fn terminate(&self, _pid: u32) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }
    async fn wait(&self, _pid: u32) -> Result<i32, SpawnSupervisorError> {
        Ok(0)
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
    async fn enqueue_resume(&self, _instance_id: InstanceId) -> Result<(), SpawnSupervisorError> {
        Ok(())
    }
}
