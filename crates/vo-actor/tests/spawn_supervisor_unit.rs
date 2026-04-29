mod common;

use std::path::PathBuf;

use common::test_instance_id;
use vo_actor::spawn_supervisor::{
    calculate_backoff_delay, is_zombie_state, should_respawn, Counter, CycleResult, ProcessHandle,
    SpawnPhase, SpawnRecord, SpawnSupervisorError,
};

// =============================================================================
// Error Classification
// =============================================================================

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

// =============================================================================
// SpawnPhase Display
// =============================================================================

#[test]
fn spawn_phase_display_formats_correctly() {
    assert_eq!(SpawnPhase::Spawn.to_string(), "spawn");
    assert_eq!(SpawnPhase::HealthCheck.to_string(), "health-check");
    assert_eq!(SpawnPhase::Running.to_string(), "running");
    assert_eq!(SpawnPhase::Shutdown.to_string(), "shutdown");
    assert_eq!(SpawnPhase::Terminated.to_string(), "terminated");
    assert_eq!(SpawnPhase::Failed.to_string(), "failed");
}

// =============================================================================
// Counter
// =============================================================================

#[test]
fn counter_starts_at_zero() {
    let counter = Counter::new();
    assert_eq!(counter.get(), 0);
}

#[test]
fn counter_increments() {
    let counter = Counter::new();
    counter.incr();
    counter.incr();
    counter.incr();
    assert_eq!(counter.get(), 3);
}

// =============================================================================
// ProcessHandle
// =============================================================================

#[test]
fn process_handle_contains_pid_and_command() {
    let handle = ProcessHandle::new(1234, PathBuf::from("./worker"), vec![]);
    assert_eq!(handle.pid, 1234);
    assert_eq!(handle.executable, PathBuf::from("./worker"));
}

// =============================================================================
// CycleResult
// =============================================================================

#[test]
fn cycle_result_has_correct_fields() {
    let result = CycleResult {
        spawns_processed: 3,
        health_checks: 1,
        errors: 1,
        respawns: 1,
    };
    assert_eq!(result.spawns_processed, 3);
    assert_eq!(result.health_checks, 1);
    assert_eq!(result.errors, 1);
    assert_eq!(result.respawns, 1);
}

// =============================================================================
// Error Display
// =============================================================================

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

// =============================================================================
// Pure Functions: calculate_backoff_delay
// =============================================================================

#[test]
fn calculate_backoff_delay_first_attempt_returns_initial() {
    let delay = calculate_backoff_delay(1000, 2.0, 1);
    assert_eq!(delay, 1000);
}

#[test]
fn calculate_backoff_delay_exponential_growth() {
    assert_eq!(calculate_backoff_delay(1000, 2.0, 1), 1000);
    assert_eq!(calculate_backoff_delay(1000, 2.0, 2), 2000);
    assert_eq!(calculate_backoff_delay(1000, 2.0, 3), 4000);
}

#[test]
fn calculate_backoff_delay_constant_multiplier() {
    assert_eq!(calculate_backoff_delay(1000, 1.0, 1), 1000);
    assert_eq!(calculate_backoff_delay(1000, 1.0, 10), 1000);
    assert_eq!(calculate_backoff_delay(1000, 1.0, 100), 1000);
}

// =============================================================================
// Pure Functions: is_zombie_state
// =============================================================================

#[test]
fn is_zombie_state_true_for_failed_high_attempts() {
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: test_instance_id(),
        executable: PathBuf::from("test"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 5,
        last_error: None,
    };
    assert!(is_zombie_state(&record));
}

#[test]
fn is_zombie_state_false_for_low_attempts() {
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: test_instance_id(),
        executable: PathBuf::from("test"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 3,
        last_error: None,
    };
    assert!(!is_zombie_state(&record));
}

#[test]
fn is_zombie_state_false_for_non_failed_phase() {
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: test_instance_id(),
        executable: PathBuf::from("test"),
        args: vec![],
        spawn_phase: SpawnPhase::Running,
        health_checks: 0,
        spawn_attempts: 10,
        last_error: None,
    };
    assert!(!is_zombie_state(&record));
}

// =============================================================================
// Pure Functions: should_respawn
// =============================================================================

#[test]
fn should_respawn_true_for_failed_within_limit() {
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: test_instance_id(),
        executable: PathBuf::from("test"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 2,
        last_error: None,
    };
    assert!(should_respawn(&record, 5));
}

#[test]
fn should_respawn_false_at_limit() {
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: test_instance_id(),
        executable: PathBuf::from("test"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 5,
        last_error: None,
    };
    assert!(!should_respawn(&record, 5));
}

#[test]
fn should_respawn_false_for_non_failed_phase() {
    let record = SpawnRecord {
        spawn_id: None,
        instance_id: test_instance_id(),
        executable: PathBuf::from("test"),
        args: vec![],
        spawn_phase: SpawnPhase::Running,
        health_checks: 0,
        spawn_attempts: 2,
        last_error: None,
    };
    assert!(!should_respawn(&record, 5));
}

// =============================================================================
// SpawnRecord Transitions
// =============================================================================

#[test]
fn spawn_record_new_sets_correct_defaults() {
    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);

    assert_eq!(record.instance_id, instance_id);
    assert_eq!(record.executable, PathBuf::from("./worker"));
    assert_eq!(record.args, Vec::<String>::new());
    assert_eq!(record.spawn_phase, SpawnPhase::Spawn);
    assert_eq!(record.spawn_attempts, 1);
    assert_eq!(record.health_checks, 0);
    assert!(record.last_error.is_none());
}

#[test]
fn spawn_record_transition_to_health_check() {
    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);
    let transitioned = record.transition_to_health_check();

    assert_eq!(transitioned.spawn_phase, SpawnPhase::HealthCheck);
    assert_eq!(transitioned.instance_id, record.instance_id);
    assert_eq!(transitioned.executable, record.executable);
    assert_eq!(transitioned.args, record.args);
    assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
}

#[test]
fn spawn_record_transition_to_running() {
    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
        .transition_to_health_check();
    let transitioned = record.transition_to_running();

    assert_eq!(transitioned.spawn_phase, SpawnPhase::Running);
}

#[test]
fn spawn_record_transition_to_shutdown() {
    let instance_id = test_instance_id();
    let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
        .transition_to_health_check()
        .transition_to_running();
    let transitioned = record.transition_to_shutdown();

    assert_eq!(transitioned.spawn_phase, SpawnPhase::Shutdown);
}

#[test]
fn spawn_record_respawn_increments_attempts() {
    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id,
        executable: PathBuf::from("./worker"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 3,
        last_error: Some(SpawnSupervisorError::ProcessExited {
            instance_id: test_instance_id(),
            pid: 1234,
            exit_code: 1,
        }),
    };

    let respawned = record.respawn(Some(vo_types::SpawnId::new("new-spawn".to_string())));

    assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
    assert_eq!(respawned.spawn_attempts, 4);
    assert_eq!(respawned.health_checks, 0);
    assert!(respawned.last_error.is_none());
    assert_eq!(
        respawned.spawn_id,
        Some(vo_types::SpawnId::new("new-spawn".to_string()))
    );
}

#[test]
fn spawn_record_respawn_saturating_at_u32_max() {
    let instance_id = test_instance_id();
    let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);
    record.spawn_phase = SpawnPhase::Failed;
    record.spawn_attempts = u32::MAX;

    let respawned = record.respawn(None);

    assert_eq!(respawned.spawn_attempts, u32::MAX);
}

#[test]
fn spawn_record_respawn_with_none_spawn_id() {
    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: Some(vo_types::SpawnId::new("old-spawn".to_string())),
        instance_id,
        executable: PathBuf::from("./worker"),
        args: vec![],
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 3,
        last_error: None,
    };

    let respawned = record.respawn(None);

    assert_eq!(respawned.spawn_id, None);
    assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
    assert_eq!(respawned.spawn_attempts, 4);
}
