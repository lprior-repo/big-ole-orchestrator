//! Red Queen: Adversarial tests for structured logging pipeline
//!
//! Attack vectors targeting error classification, structured logging conventions,
//! metrics co-location, and error taxonomy across spawn supervisor and reanimator.

use std::path::PathBuf;
use std::time::Duration;

use vo_actor::reanimator::ReanimatorError;
use vo_actor::spawn_supervisor::SpawnSupervisorError;
use vo_types::InstanceId;

fn test_instance_id() -> InstanceId {
    use ulid::Ulid;
    let ulid = Ulid::new();
    InstanceId::from_bytes(ulid.to_bytes())
}

// =============================================================================
// ATTACK 1: Error Classification Completeness
// =============================================================================
// INV: Every error variant MUST be either transient or fatal.
// Per ER-007/ER-008, unclassified errors are treated as fatal.
// SpawnSupervisorError has variants that are NEITHER — this test exposes them.

#[test]
fn spawn_supervisor_error_no_unclassified_variants() {
    let instance_id = test_instance_id();

    let unclassified: Vec<&str> = vec![
        (
            "AtomicityViolation",
            SpawnSupervisorError::AtomicityViolation("test".into()),
        ),
        ("AlreadyRunning", SpawnSupervisorError::AlreadyRunning),
        (
            "ShutdownTimeout",
            SpawnSupervisorError::ShutdownTimeout(Duration::from_secs(5)),
        ),
        (
            "SpawnFailed",
            SpawnSupervisorError::SpawnFailed {
                executable: PathBuf::from("cmd"),
                error: "err".into(),
            },
        ),
        (
            "HealthCheckFailed",
            SpawnSupervisorError::HealthCheckFailed {
                instance_id: instance_id.clone(),
                check_number: 1,
                error: "timeout".into(),
            },
        ),
        (
            "ProcessExited",
            SpawnSupervisorError::ProcessExited {
                instance_id: instance_id.clone(),
                pid: 1234,
                exit_code: 1,
            },
        ),
        ("NotRunning", SpawnSupervisorError::NotRunning),
        ("AlreadyShutdown", SpawnSupervisorError::AlreadyShutdown),
    ]
    .into_iter()
    .filter(|(_, e)| !e.is_transient() && !e.is_fatal())
    .map(|(name, _)| name)
    .collect();

    assert!(
        unclassified.is_empty(),
        "SpawnSupervisorError has unclassified variants (neither transient nor fatal): {:?}\n\
         Per ER-007/ER-008, ALL errors must be classified. Unknown errors are treated as fatal.",
        unclassified
    );
}

#[test]
fn atomicity_violation_is_classified() {
    let error = SpawnSupervisorError::AtomicityViolation("partial delete".into());
    assert!(
        error.is_transient() || error.is_fatal(),
        "AtomicityViolation must be classified as either transient or fatal"
    );
}

#[test]
fn already_running_is_classified() {
    let error = SpawnSupervisorError::AlreadyRunning;
    assert!(
        error.is_transient() || error.is_fatal(),
        "AlreadyRunning must be classified as either transient or fatal"
    );
}

#[test]
fn shutdown_timeout_is_classified() {
    let error = SpawnSupervisorError::ShutdownTimeout(Duration::from_secs(5));
    assert!(
        error.is_transient() || error.is_fatal(),
        "ShutdownTimeout must be classified as either transient or fatal"
    );
}

#[test]
fn spawn_failed_is_classified() {
    let error = SpawnSupervisorError::SpawnFailed {
        executable: PathBuf::from("cmd"),
        error: "err".into(),
    };
    assert!(
        error.is_transient() || error.is_fatal(),
        "SpawnFailed must be classified as either transient or fatal"
    );
}

#[test]
fn health_check_failed_is_classified() {
    let error = SpawnSupervisorError::HealthCheckFailed {
        instance_id: test_instance_id(),
        check_number: 1,
        error: "timeout".into(),
    };
    assert!(
        error.is_transient() || error.is_fatal(),
        "HealthCheckFailed must be classified as either transient or fatal"
    );
}

#[test]
fn process_exited_is_classified() {
    let error = SpawnSupervisorError::ProcessExited {
        instance_id: test_instance_id(),
        pid: 1234,
        exit_code: 1,
    };
    assert!(
        error.is_transient() || error.is_fatal(),
        "ProcessExited must be classified as either transient or fatal"
    );
}

#[test]
fn not_running_is_classified() {
    let error = SpawnSupervisorError::NotRunning;
    assert!(
        error.is_transient() || error.is_fatal(),
        "NotRunning must be classified as either transient or fatal"
    );
}

#[test]
fn already_shutdown_is_classified() {
    let error = SpawnSupervisorError::AlreadyShutdown;
    assert!(
        error.is_transient() || error.is_fatal(),
        "AlreadyShutdown must be classified as either transient or fatal"
    );
}

#[test]
fn reanimator_storage_init_failed_is_classified() {
    let error = ReanimatorError::StorageInitFailed("db failed".into());
    assert!(
        error.is_transient() || error.is_fatal(),
        "StorageInitFailed must be classified as either transient or fatal"
    );
}

#[test]
fn reanimator_task_spawn_failed_is_classified() {
    let error = ReanimatorError::TaskSpawnFailed("spawn failed".into());
    assert!(
        error.is_transient() || error.is_fatal(),
        "TaskSpawnFailed must be classified as either transient or fatal"
    );
}

#[test]
fn reanimator_shutdown_timeout_is_classified() {
    let error = ReanimatorError::ShutdownTimeout(Duration::from_secs(5));
    assert!(
        error.is_transient() || error.is_fatal(),
        "ShutdownTimeout must be classified as either transient or fatal"
    );
}

#[test]
fn reanimator_error_transient_never_fatal_exhaustive() {
    let instance_id = test_instance_id();
    let all_variants = vec![
        ReanimatorError::StorageError("test".into()),
        ReanimatorError::CorruptKey("test".into()),
        ReanimatorError::AtomicityViolation("test".into()),
        ReanimatorError::InstanceNotFound(instance_id),
        ReanimatorError::BudgetExceeded("test".into()),
        ReanimatorError::EnqueueFailed("test".into()),
        ReanimatorError::AlreadyRunning,
        ReanimatorError::StorageInitFailed("test".into()),
        ReanimatorError::TaskSpawnFailed("test".into()),
        ReanimatorError::AlreadyShutdown,
        ReanimatorError::ShutdownTimeout(Duration::from_secs(5)),
    ];

    for error in &all_variants {
        assert!(
            !(error.is_transient() && error.is_fatal()),
            "INV VIOLATION: {:?} is both transient AND fatal — must be mutually exclusive",
            error
        );
    }
}

#[test]
fn counter_concurrent_increments_no_lost_updates() {
    use std::thread;
    use vo_actor::spawn_supervisor::Counter;

    let counter = std::sync::Arc::new(Counter::new());
    let num_threads = 8;
    let increments_per_thread = 10_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = counter.clone();
            thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    c.incr();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    let expected = num_threads * increments_per_thread;
    assert_eq!(
        counter.get(),
        expected as u64,
        "Counter lost updates under contention: expected {}, got {}",
        expected,
        counter.get()
    );
}

#[test]
fn metrics_counters_are_independent() {
    use vo_actor::spawn_supervisor::SpawnSupervisorMetrics;

    let metrics = SpawnSupervisorMetrics::default();

    metrics.spawns_failed.incr();
    metrics.spawns_failed.incr();
    metrics.spawns_failed.incr();

    assert_eq!(metrics.spawns_failed.get(), 3);
    assert_eq!(
        metrics.spawns_successful.get(),
        0,
        "spawns_successful affected by spawns_failed incr"
    );
    assert_eq!(metrics.health_checks_performed.get(), 0);
    assert_eq!(metrics.health_checks_failed.get(), 0);
    assert_eq!(metrics.zombies_detected.get(), 0);
    assert_eq!(metrics.respawns.get(), 0);
    assert_eq!(metrics.dispatch_errors.get(), 0);

    metrics.zombies_detected.incr();
    assert_eq!(metrics.zombies_detected.get(), 1);
    assert_eq!(
        metrics.spawns_failed.get(),
        3,
        "spawns_failed changed after zombies_detected incr"
    );
}

#[test]
fn spawn_supervisor_error_display_with_empty_strings() {
    let instance_id = test_instance_id();
    let empty_errors = vec![
        SpawnSupervisorError::StorageError(String::new()),
        SpawnSupervisorError::CorruptSpawn(String::new()),
        SpawnSupervisorError::AtomicityViolation(String::new()),
        SpawnSupervisorError::DispatchError(String::new()),
        SpawnSupervisorError::SpawnFailed {
            executable: PathBuf::new(),
            error: String::new(),
        },
        SpawnSupervisorError::HealthCheckFailed {
            instance_id,
            check_number: 0,
            error: String::new(),
        },
    ];

    for error in &empty_errors {
        let display = format!("{error}");
        assert!(
            !display.is_empty(),
            "Error with empty payload has empty Display: {:?}",
            error
        );
        assert!(
            display.trim().len() >= 3,
            "Error display too short with empty payload: {:?} → {:?}",
            error,
            display
        );
    }
}

#[test]
fn spawn_supervisor_error_clone_eq_symmetry() {
    let instance_id = test_instance_id();
    let errors = vec![
        SpawnSupervisorError::StorageError("test".into()),
        SpawnSupervisorError::CorruptSpawn("bad".into()),
        SpawnSupervisorError::InstanceNotFound(instance_id.clone()),
        SpawnSupervisorError::AlreadyRunning,
        SpawnSupervisorError::NotRunning,
        SpawnSupervisorError::ZombieDetected {
            instance_id,
            pid: 42,
        },
    ];

    for error in &errors {
        let cloned = error.clone();
        assert_eq!(
            error, &cloned,
            "Cloned error not equal to original: {:?}",
            error
        );
        assert_eq!(
            format!("{}", error),
            format!("{}", cloned),
            "Cloned error displays differently: {:?}",
            error
        );
        assert_eq!(error.is_transient(), cloned.is_transient());
        assert_eq!(error.is_fatal(), cloned.is_fatal());
    }
}

#[test]
fn spawn_record_transition_chain_preserves_all_fields() {
    use vo_actor::spawn_supervisor::{SpawnPhase, SpawnRecord};

    let instance_id = test_instance_id();
    let spawn_id = Some(vo_types::SpawnId::new("spawn-42".into()));

    let original = SpawnRecord {
        spawn_id: spawn_id.clone(),
        instance_id: instance_id.clone(),
        executable: PathBuf::from("./worker"),
        args: vec!["--port".to_string(), "8080".to_string()],
        spawn_phase: SpawnPhase::Spawn,
        health_checks: 0,
        spawn_attempts: 3,
        last_error: None,
    };

    let health_check = original.transition_to_health_check();
    assert_eq!(health_check.spawn_phase, SpawnPhase::HealthCheck);
    assert_eq!(health_check.instance_id, instance_id);
    assert_eq!(health_check.executable, PathBuf::from("./worker"));
    assert_eq!(health_check.args, vec!["--port".to_string(), "8080".to_string()]);
    assert_eq!(health_check.spawn_attempts, 3);
    assert_eq!(health_check.spawn_id, spawn_id);

    let running = health_check.transition_to_running();
    assert_eq!(running.spawn_phase, SpawnPhase::Running);
    assert_eq!(running.instance_id, instance_id);

    let shutdown = running.transition_to_shutdown();
    assert_eq!(shutdown.spawn_phase, SpawnPhase::Shutdown);
    assert_eq!(shutdown.instance_id, instance_id);
    assert_eq!(shutdown.spawn_attempts, 3);
}

#[test]
fn spawn_record_last_error_preserved_through_transition() {
    use vo_actor::spawn_supervisor::SpawnRecord;

    let instance_id = test_instance_id();
    let original_error = SpawnSupervisorError::StorageError("db down".into());

    let record = SpawnRecord {
        spawn_id: None,
        instance_id,
        executable: PathBuf::from("./worker"),
        args: vec![],
        spawn_phase: vo_actor::spawn_supervisor::SpawnPhase::Spawn,
        health_checks: 0,
        spawn_attempts: 1,
        last_error: Some(original_error.clone()),
    };

    let transitioned = record.transition_to_health_check();
    assert!(
        transitioned.last_error.is_some(),
        "last_error lost during transition"
    );
    assert_eq!(transitioned.last_error, Some(original_error));
}

#[test]
fn spawn_record_respawn_clears_error() {
    use vo_actor::spawn_supervisor::SpawnRecord;

    let instance_id = test_instance_id();
    let record = SpawnRecord {
        spawn_id: None,
        instance_id,
        executable: PathBuf::from("./worker"),
        args: vec![],
        spawn_phase: vo_actor::spawn_supervisor::SpawnPhase::Failed,
        health_checks: 5,
        spawn_attempts: 3,
        last_error: Some(SpawnSupervisorError::SpawnFailed {
            executable: PathBuf::from("./worker"),
            error: "segfault".into(),
        }),
    };

    let respawned = record.respawn(None);
    assert!(
        respawned.last_error.is_none(),
        "Respawn must clear last_error"
    );
    assert_eq!(
        respawned.health_checks, 0,
        "Respawn must reset health_checks"
    );
}
