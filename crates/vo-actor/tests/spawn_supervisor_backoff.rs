mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::{test_instance_id, MockProcessManager, MockSpawnStorage, MockWorkQueue};
use vo_actor::lifecycle::ShutdownPropagator;
use vo_actor::spawn_supervisor::{ExecutionSemaphore, SpawnPhase, SpawnRecord, SpawnSupervisor};

#[tokio::test]
async fn respawn_after_health_check_failure_delays_by_backoff() {
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
        Duration::from_millis(200),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let start = std::time::Instant::now();
    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(400),
        "Expected at least ~500ms elapsed (3 health checks + backoff), got {:?}",
        elapsed
    );

    assert_eq!(supervisor.metrics.respawns.get(), 1);
}

#[tokio::test]
async fn respawn_failed_phase_delays_by_backoff() {
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
        Duration::from_millis(300),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let start = std::time::Instant::now();
    supervisor
        .process_cycle()
        .await
        .expect("Process cycle should succeed");
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(500),
        "Expected at least ~600ms elapsed for attempt-2 backoff, got {:?}",
        elapsed
    );

    assert_eq!(supervisor.metrics.respawns.get(), 1);

    let enqueued = work_queue.get_enqueued_spawns();
    assert_eq!(enqueued.len(), 1, "Should have enqueued one spawn");
}

#[tokio::test]
async fn respawn_exponential_backoff_increases_with_attempts() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let initial_backoff = Duration::from_millis(50);

    let instance_id_1 = test_instance_id();
    let mut record_1 = SpawnRecord::new(instance_id_1.clone(), "./worker".to_string(), None);
    record_1.spawn_phase = SpawnPhase::Failed;
    record_1.spawn_attempts = 1;
    storage.add_record(record_1);

    let supervisor = SpawnSupervisor::new(
        Duration::from_millis(10),
        1,
        initial_backoff,
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    )
    .expect("Valid config");

    let start = std::time::Instant::now();
    supervisor.process_cycle().await.expect("ok");
    let elapsed_1 = start.elapsed();

    storage.records.lock().unwrap().clear();
    let instance_id_2 = test_instance_id();
    let mut record_2 = SpawnRecord::new(instance_id_2.clone(), "./worker".to_string(), None);
    record_2.spawn_phase = SpawnPhase::Failed;
    record_2.spawn_attempts = 2;
    storage.add_record(record_2);

    let start = std::time::Instant::now();
    supervisor.process_cycle().await.expect("ok");
    let elapsed_2 = start.elapsed();

    assert!(
        elapsed_2 > elapsed_1,
        "Attempt 2 backoff ({:?}) should exceed attempt 1 ({:?})",
        elapsed_2,
        elapsed_1
    );
}
