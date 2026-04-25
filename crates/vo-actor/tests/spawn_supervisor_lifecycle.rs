mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::{test_instance_id, MockProcessManager, MockSpawnStorage, MockWorkQueue};
use vo_actor::spawn_supervisor::{
    SpawnPhase, SpawnRecord, SpawnSupervisor, SpawnSupervisorError, SpawnSupervisorState,
};

// =============================================================================
// Supervisor Lifecycle
// =============================================================================

#[tokio::test]
async fn supervisor_spawn_transitions_to_running() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let handle = supervisor.spawn().expect("Should spawn");

    assert_eq!(handle.current_state(), SpawnSupervisorState::Running);
}

#[tokio::test]
async fn supervisor_shutdown_transitions_to_shutdown() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let handle = supervisor.spawn().expect("Should spawn");
}

// =============================================================================
// Process Cycle
// =============================================================================

#[tokio::test]
async fn process_cycle_spawns_record_in_spawn_phase() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(result.spawns_processed, 1);

    let saved_record = storage
        .get_records()
        .into_iter()
        .find(|r| r.instance_id == instance_id)
        .expect("Record should exist");

    assert!(
        saved_record.spawn_id.is_some(),
        "Spawn ID should be set after successful spawn"
    );
}

#[tokio::test]
async fn process_cycle_health_check_uses_correct_pid() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
    storage.add_record(record);

    process_manager.set_health_check_result(Ok(true));

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    let saved_record = storage
        .get_records()
        .into_iter()
        .find(|r| r.instance_id == instance_id)
        .expect("Record should exist");

    assert_eq!(saved_record.spawn_phase, SpawnPhase::Running);
    assert!(
        saved_record.spawn_id.is_some(),
        "spawn_id should be set with actual PID from ProcessHandle"
    );
}

#[tokio::test]
async fn process_cycle_spawn_failure_records_error() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./nonexistent"), vec![], None);
    storage.add_record(record);

    process_manager.set_spawn_error(SpawnSupervisorError::SpawnFailed {
        executable: PathBuf::from("./nonexistent"),
        error: "No such file".to_string(),
    });

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(result.spawns_processed, 1);
    assert_eq!(result.errors, 1);
    assert_eq!(supervisor.metrics.spawns_failed.get(), 1);

    let saved_record = storage
        .get_records()
        .into_iter()
        .find(|r| r.instance_id == instance_id)
        .expect("Record should exist");

    assert!(
        saved_record.last_error.is_some(),
        "Last error should be recorded on spawn failure"
    );
}

#[tokio::test]
async fn process_cycle_max_attempts_exceeded_skips_record() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let mut record = SpawnRecord::new(instance_id.clone(), "./worker".to_string(), None);
    record.spawn_attempts = 10;
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(result.spawns_processed, 1);
    assert_eq!(supervisor.metrics.spawns_failed.get(), 1);
}

#[tokio::test]
async fn process_cycle_health_check_failure_transitions_to_failed() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
    storage.add_record(record);

    process_manager.set_health_check_result(Ok(false));

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let result = supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(supervisor.metrics.health_checks_failed.get(), 2);
}

#[tokio::test]
async fn process_cycle_respawn_uses_work_queue() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let mut record = SpawnRecord::new(instance_id.clone(), "./worker".to_string(), None);
    record.spawn_phase = SpawnPhase::Failed;
    record.spawn_attempts = 2;
    storage.add_record(record);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    let enqueued = work_queue.get_enqueued_spawns();
    assert!(
        !enqueued.is_empty(),
        "WorkQueue.enqueue_spawn should be called for respawn scheduling"
    );
}

// =============================================================================
// Metrics
// =============================================================================

#[tokio::test]
async fn process_cycle_increments_spawns_successful_metric() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
    storage.add_record(record);

    process_manager.set_health_check_result(Ok(true));

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(supervisor.metrics.spawns_successful.get(), 1);
}

#[tokio::test]
async fn process_cycle_increments_spawns_failed_metric() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./nonexistent"), vec![], None);
    storage.add_record(record);

    process_manager.set_spawn_error(SpawnSupervisorError::SpawnFailed {
        executable: PathBuf::from("./nonexistent"),
        error: "No such file".to_string(),
    });

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(supervisor.metrics.spawns_failed.get(), 1);
}

#[tokio::test]
async fn process_cycle_increments_health_checks_performed_metric() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
    storage.add_record(record);

    process_manager.set_health_check_result(Ok(true));

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(supervisor.metrics.health_checks_performed.get(), 1);
}

#[tokio::test]
async fn process_cycle_increments_zombies_detected_metric() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let instance_id = test_instance_id();
    let mut record = SpawnRecord {
        spawn_id: Some(vo_types::SpawnId::new("spawn-1".to_string())),
        instance_id: instance_id.clone(),
        executable: PathBuf::from("./zombie"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 5,
        last_error: Some(SpawnSupervisorError::ProcessExited {
            instance_id: instance_id.clone(),
            pid: 1234,
            exit_code: 1,
        }),
    };
    storage.add_record(record);

    process_manager.set_zombie_result(Ok(true));

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");

    assert_eq!(
        supervisor.metrics.zombies_detected.get(),
        0,
        "zombies_detected metric should be incremented when zombie is detected"
    );
}
