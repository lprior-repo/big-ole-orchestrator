mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{MockProcessManager, MockSpawnStorage, MockWorkQueue};
use vo_actor::spawn_supervisor::{SpawnSupervisor, SpawnSupervisorError};

#[test]
fn supervisor_rejects_zero_health_check_interval() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let result = SpawnSupervisor::new(
        Duration::ZERO,
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SpawnSupervisorError::InvalidConfig(_)));
}

#[test]
fn supervisor_rejects_zero_max_health_checks() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        0,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SpawnSupervisorError::InvalidConfig(_)));
}

#[test]
fn supervisor_rejects_zero_initial_backoff() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::ZERO,
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SpawnSupervisorError::InvalidConfig(_)));
}

#[test]
fn supervisor_rejects_backoff_multiplier_less_than_one() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        0.5,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, SpawnSupervisorError::InvalidConfig(_)));
}

#[test]
fn supervisor_accepts_valid_config() {
    let storage = Arc::new(MockSpawnStorage::new());
    let process_manager = Arc::new(MockProcessManager::new());
    let work_queue = Arc::new(MockWorkQueue::new());

    let result = SpawnSupervisor::new(
        Duration::from_millis(100),
        3,
        Duration::from_millis(1000),
        2.0,
        5,
        storage.clone(),
        process_manager.clone(),
        work_queue.clone(),
    );

    assert!(result.is_ok());
}
