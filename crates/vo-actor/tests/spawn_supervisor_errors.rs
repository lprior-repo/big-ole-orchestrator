mod common;

use std::path::PathBuf;

use common::test_instance_id;
use vo_actor::spawn_supervisor::SpawnSupervisorError;

#[test]
fn spawn_supervisor_error_is_transient_for_storage_error() {
    let error = SpawnSupervisorError::StorageError("db connection failed".to_string());
    assert!(error.is_transient(), "StorageError should be transient");
}

#[test]
fn spawn_supervisor_error_is_transient_for_instance_not_found() {
    let instance_id = test_instance_id();
    let error = SpawnSupervisorError::InstanceNotFound(instance_id);
    assert!(error.is_transient(), "InstanceNotFound should be transient");
}

#[test]
fn spawn_supervisor_error_is_transient_for_mailbox_full() {
    let instance_id = test_instance_id();
    let error = SpawnSupervisorError::MailboxFull(instance_id);
    assert!(error.is_transient(), "MailboxFull should be transient");
}

#[test]
fn spawn_supervisor_error_is_transient_for_dispatch_error() {
    let error = SpawnSupervisorError::DispatchError("Channel closed".to_string());
    assert!(error.is_transient(), "DispatchError should be transient");
}

#[test]
fn spawn_supervisor_error_is_fatal_for_corrupt_spawn() {
    let error = SpawnSupervisorError::CorruptSpawn("Invalid spawn key".to_string());
    assert!(error.is_fatal(), "CorruptSpawn should be fatal");
}

#[test]
fn spawn_supervisor_error_is_fatal_for_invalid_config() {
    let error = SpawnSupervisorError::InvalidConfig("Missing field".to_string());
    assert!(error.is_fatal(), "InvalidConfig should be fatal");
}

#[test]
fn spawn_supervisor_error_is_fatal_for_zombie_detected() {
    let instance_id = test_instance_id();
    let error = SpawnSupervisorError::ZombieDetected {
        instance_id,
        pid: 1234,
    };
    assert!(error.is_fatal(), "ZombieDetected should be fatal");
}

#[test]
fn spawn_supervisor_error_is_not_transient_for_fatal_errors() {
    let instance_id = test_instance_id();

    let corrupt = SpawnSupervisorError::CorruptSpawn("bad".to_string());
    assert!(
        !corrupt.is_transient(),
        "CorruptSpawn should not be transient"
    );

    let invalid = SpawnSupervisorError::InvalidConfig("bad".to_string());
    assert!(
        !invalid.is_transient(),
        "InvalidConfig should not be transient"
    );

    let zombie = SpawnSupervisorError::ZombieDetected {
        instance_id,
        pid: 1234,
    };
    assert!(
        !zombie.is_transient(),
        "ZombieDetected should not be transient"
    );
}

#[test]
fn spawn_failed_error_display() {
    let error = SpawnSupervisorError::SpawnFailed {
        executable: PathBuf::from("./worker"),
        error: "ENOENT".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Spawn failed"));
    assert!(display.contains("./worker"));
    assert!(display.contains("ENOENT"));
}

#[test]
fn health_check_failed_error_display() {
    let instance_id = test_instance_id();
    let error = SpawnSupervisorError::HealthCheckFailed {
        instance_id,
        check_number: 3,
        error: "timeout".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Health check"));
    assert!(display.contains("3"));
    assert!(display.contains("timeout"));
}

#[test]
fn zombie_detected_error_display() {
    let instance_id = test_instance_id();
    let error = SpawnSupervisorError::ZombieDetected {
        instance_id,
        pid: 1234,
    };
    let display = format!("{}", error);
    assert!(display.contains("Zombie"));
    assert!(display.contains("1234"));
}

#[test]
fn process_exited_error_display() {
    let instance_id = test_instance_id();
    let error = SpawnSupervisorError::ProcessExited {
        instance_id,
        pid: 5678,
        exit_code: 1,
    };
    let display = format!("{}", error);
    assert!(display.contains("Process exited"));
    assert!(display.contains("5678"));
    assert!(display.contains("1"));
}
