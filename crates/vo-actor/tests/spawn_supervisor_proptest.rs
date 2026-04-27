//! Proptest tests for spawn supervisor.
//!
//! Property-based tests verifying invariants for the async process supervisor.
//!
//! TDD Red Phase: These tests document expected behavior that is NOT
//! yet implemented correctly.

use std::path::PathBuf;

use proptest::prelude::*;
use vo_actor::spawn_supervisor::{
    calculate_backoff_delay, is_zombie_state, should_respawn, SpawnPhase, SpawnRecord,
    SpawnSupervisorError,
};
use vo_types::InstanceId;

fn test_instance_id() -> InstanceId {
    use ulid::Ulid;
    let ulid = Ulid::new();
    InstanceId::from_bytes(ulid.to_bytes())
}

#[derive(Debug, Clone)]
struct ArbitraryInstanceId(InstanceId);

impl Arbitrary for ArbitraryInstanceId {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<[u8; 16]>()
            .prop_map(|bytes| {
                let ulid = ulid::Ulid(u128::from_be_bytes(bytes));
                ArbitraryInstanceId(InstanceId::from_bytes(ulid.to_bytes()))
            })
            .boxed()
    }
}

#[derive(Debug, Clone)]
struct ArbitrarySpawnPhase(SpawnPhase);

impl Arbitrary for ArbitrarySpawnPhase {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(ArbitrarySpawnPhase(SpawnPhase::Spawn)),
            Just(ArbitrarySpawnPhase(SpawnPhase::HealthCheck)),
            Just(ArbitrarySpawnPhase(SpawnPhase::Running)),
            Just(ArbitrarySpawnPhase(SpawnPhase::Shutdown)),
            Just(ArbitrarySpawnPhase(SpawnPhase::Terminated)),
            Just(ArbitrarySpawnPhase(SpawnPhase::Failed)),
        ]
        .boxed()
    }
}

#[derive(Debug, Clone)]
struct ArbitrarySpawnSupervisorError(SpawnSupervisorError);

impl Arbitrary for ArbitrarySpawnSupervisorError {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            any::<String>()
                .prop_map(|s| ArbitrarySpawnSupervisorError(SpawnSupervisorError::StorageError(s))),
            any::<String>()
                .prop_map(|s| ArbitrarySpawnSupervisorError(SpawnSupervisorError::CorruptSpawn(s))),
            any::<String>().prop_map(|s| ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::AtomicityViolation(s)
            )),
            any::<ArbitraryInstanceId>().prop_map(|aid| ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::InstanceNotFound(aid.0)
            )),
            any::<ArbitraryInstanceId>().prop_map(|aid| ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::MailboxFull(aid.0)
            )),
            any::<String>().prop_map(|s| ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::InvalidConfig(s)
            )),
            Just(ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::AlreadyRunning
            )),
            any::<std::time::Duration>().prop_map(|d| ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::ShutdownTimeout(d)
            )),
            any::<String>().prop_map(|s| ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::DispatchError(s)
            )),
            (any::<String>(), any::<String>()).prop_map(|(command, error)| {
                ArbitrarySpawnSupervisorError(SpawnSupervisorError::SpawnFailed {
                    executable: command.into(),
                    error,
                })
            }),
            (any::<ArbitraryInstanceId>(), any::<u32>(), any::<String>()).prop_map(
                |(aid, check_number, error)| ArbitrarySpawnSupervisorError(
                    SpawnSupervisorError::HealthCheckFailed {
                        instance_id: aid.0,
                        check_number,
                        error
                    }
                )
            ),
            (any::<ArbitraryInstanceId>(), any::<u32>()).prop_map(|(aid, pid)| {
                ArbitrarySpawnSupervisorError(SpawnSupervisorError::ZombieDetected {
                    instance_id: aid.0,
                    pid,
                })
            }),
            (any::<ArbitraryInstanceId>(), any::<u32>(), any::<i32>()).prop_map(
                |(aid, pid, exit_code)| ArbitrarySpawnSupervisorError(
                    SpawnSupervisorError::ProcessExited {
                        instance_id: aid.0,
                        pid,
                        exit_code
                    }
                )
            ),
            Just(ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::NotRunning
            )),
            Just(ArbitrarySpawnSupervisorError(
                SpawnSupervisorError::AlreadyShutdown
            )),
        ]
        .boxed()
    }
}

// =============================================================================
// Proptest Invariants - Backoff Calculation
// =============================================================================

proptest! {
    #[test]
    fn backoff_delay_monotonic(initial in 1u64..=10000, multiplier in 1.0f64..=10.0, attempt_a in 1u32..=30, attempt_b in 1u32..=30) {
        // Filter to ensure attempt_a < attempt_b for valid monotonicity test
        let (attempt_a, attempt_b) = if attempt_a < attempt_b {
            (attempt_a, attempt_b)
        } else if attempt_a > attempt_b {
            (attempt_b, attempt_a)
        } else {
            // Equal attempts - test that result is same
            prop_assert_eq!(
                calculate_backoff_delay(initial, multiplier, attempt_a),
                calculate_backoff_delay(initial, multiplier, attempt_b)
            );
            return Ok(());
        };

        let delay_a = calculate_backoff_delay(initial, multiplier, attempt_a);
        let delay_b = calculate_backoff_delay(initial, multiplier, attempt_b);

        // With multiplier >= 1.0, later attempts should have >= delay
        prop_assert!(
            delay_b >= delay_a,
            "Later attempt {} should have >= delay than earlier attempt {} (initial={}, mult={})",
            attempt_b, attempt_a, initial, multiplier
        );
    }

    #[test]
    fn backoff_delay_lower_bound(initial in 1u64..=10000, multiplier in 1.0f64..=10.0, attempt in 1u32..=100) {
        let delay = calculate_backoff_delay(initial, multiplier, attempt);
        prop_assert!(
            delay >= initial,
            "Backoff delay {} should be >= initial {} for attempt {}",
            delay, initial, attempt
        );
    }

    #[test]
    fn backoff_delay_no_panic(initial: u64, multiplier: f64, attempt: u32) {
        // Filter to only valid multiplier values
        let multiplier = if multiplier.is_finite() && multiplier >= 1.0 {
            multiplier
        } else {
            1.0 // Default to valid value
        };

        // Should not panic
        let _ = calculate_backoff_delay(initial, multiplier, attempt);
    }

    #[test]
    fn backoff_delay_first_attempt_always_initial(initial in 1u64..=10000, multiplier in 1.0f64..=10.0) {
        let delay = calculate_backoff_delay(initial, multiplier, 1);
        prop_assert_eq!(
            delay, initial,
            "First attempt (1) should always return initial backoff"
        );
    }
}

// =============================================================================
// Proptest Invariants - SpawnRecord Transitions
// =============================================================================

proptest! {
    #[test]
    fn spawn_record_transition_preserves_fields(
        instance_id: ArbitraryInstanceId,
        command: String,
        spawn_id: Option<String>,
        phase in prop::sample::select(vec![
            SpawnPhase::Spawn,
            SpawnPhase::HealthCheck,
            SpawnPhase::Running,
            SpawnPhase::Shutdown,
            SpawnPhase::Terminated,
            SpawnPhase::Failed,
        ]),
        health_checks: u32,
        spawn_attempts: u32,
    ) {
        let instance_id = instance_id.0;
        let spawn_id = spawn_id.map(vo_types::SpawnId::new);
        let record = SpawnRecord {
            spawn_id: spawn_id.clone(),
            instance_id: instance_id.clone(),
            executable: PathBuf::from(&command),
            args: vec![],
            spawn_phase: phase,
            health_checks,
            spawn_attempts,
            last_error: None,
        };

        let transitioned = match phase {
            SpawnPhase::Spawn => record.transition_to_health_check(),
            SpawnPhase::HealthCheck => record.transition_to_running(),
            SpawnPhase::Running => record.transition_to_shutdown(),
            _ => return Ok(()),
        };

        prop_assert_eq!(transitioned.instance_id, record.instance_id);
        prop_assert_eq!(transitioned.executable, record.executable);
        prop_assert_eq!(transitioned.args, record.args);
        prop_assert_eq!(transitioned.spawn_id, record.spawn_id);
        prop_assert_eq!(transitioned.health_checks, record.health_checks);
        prop_assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
        prop_assert_eq!(transitioned.last_error, record.last_error);
    }

    #[test]
    fn spawn_record_respawn_phase_always_spawn(
        instance_id: ArbitraryInstanceId,
        command: String,
        spawn_attempts: u32,
        phase in prop::sample::select(vec![
            SpawnPhase::Spawn,
            SpawnPhase::HealthCheck,
            SpawnPhase::Running,
            SpawnPhase::Shutdown,
            SpawnPhase::Terminated,
            SpawnPhase::Failed,
        ]),
    ) {
        let instance_id = instance_id.0;
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: instance_id.clone(),
            executable: PathBuf::from(&command),
            args: vec![],
            spawn_phase: phase,
            health_checks: 0,
            spawn_attempts,
            last_error: None,
        };

        let respawned = record.respawn(None);

        prop_assert_eq!(
            respawned.spawn_phase,
            SpawnPhase::Spawn,
            "Respawn should always set phase to Spawn, regardless of original phase"
        );
    }

    #[test]
    fn spawn_record_respawn_attempts_non_decreasing(
        instance_id: ArbitraryInstanceId,
        command: String,
        spawn_attempts: u32,
    ) {
        let instance_id = instance_id.0;
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from(&command),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts,
            last_error: None,
        };

        let respawned = record.respawn(None);

        prop_assert!(
            respawned.spawn_attempts >= record.spawn_attempts,
            "Respawned attempts {} should be >= original {}",
            respawned.spawn_attempts, record.spawn_attempts
        );
    }

    #[test]
    fn spawn_record_respawn_resets_health_checks(
        instance_id: ArbitraryInstanceId,
        command: String,
        health_checks: u32,
    ) {
        let instance_id = instance_id.0;
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from(&command),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks,
            spawn_attempts: 1,
            last_error: None,
        };

        let respawned = record.respawn(None);

        prop_assert_eq!(
            respawned.health_checks, 0,
            "Respawn should reset health_checks to 0"
        );
    }

    #[test]
    fn spawn_record_respawn_clears_error(
        instance_id: ArbitraryInstanceId,
        command: String,
    ) {
        let instance_id = instance_id.0;
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from(&command),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: Some(SpawnSupervisorError::ProcessExited {
                instance_id: test_instance_id(),
                pid: 1234,
                exit_code: 1,
            }),
        };

        let respawned = record.respawn(None);

        prop_assert!(
            respawned.last_error.is_none(),
            "Respawn should clear last_error"
        );
    }
}

// =============================================================================
// Proptest Invariants - Error Classification
// =============================================================================

proptest! {
    #[test]
    fn error_not_both_transient_and_fatal(error in any::<ArbitrarySpawnSupervisorError>()) {
        let error = error.0;
        // An error cannot be both transient and fatal
        prop_assert!(
            !(error.is_transient() && error.is_fatal()),
            "Error {:?} cannot be both transient and fatal",
            error
        );
    }

    #[test]
    fn error_display_never_empty(error in any::<ArbitrarySpawnSupervisorError>()) {
        let error = error.0;
        let display = format!("{}", error);
        prop_assert!(
            !display.is_empty(),
            "Error display for {:?} should never be empty",
            error
        );
    }
}

// =============================================================================
// Proptest Invariants - is_zombie_state
// =============================================================================

proptest! {
    #[test]
    fn is_zombie_state_condition_exact(
        phase in prop::sample::select(vec![
            SpawnPhase::Spawn,
            SpawnPhase::HealthCheck,
            SpawnPhase::Running,
            SpawnPhase::Shutdown,
            SpawnPhase::Terminated,
            SpawnPhase::Failed,
        ]),
        spawn_attempts: u32,
    ) {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            executable: PathBuf::from("test"),
            args: vec![],
            spawn_phase: phase,
            health_checks: 0,
            spawn_attempts,
            last_error: None,
        };

        let is_zombie = is_zombie_state(&record);

        // is_zombie should only be true when phase is Failed AND spawn_attempts > 3
        let expected = matches!(phase, SpawnPhase::Failed) && spawn_attempts > 3;

        prop_assert_eq!(
            is_zombie, expected,
            "is_zombie_state({:?}, attempts={}) = {}, expected {}",
            phase, spawn_attempts, is_zombie, expected
        );
    }
}

// =============================================================================
// Proptest Invariants - should_respawn
// =============================================================================

proptest! {
    #[test]
    fn should_respawn_condition_exact(
        phase in prop::sample::select(vec![
            SpawnPhase::Spawn,
            SpawnPhase::HealthCheck,
            SpawnPhase::Running,
            SpawnPhase::Shutdown,
            SpawnPhase::Terminated,
            SpawnPhase::Failed,
        ]),
        spawn_attempts: u32,
        max_attempts: u32,
    ) {
        let record = SpawnRecord {
            spawn_id: None,
            instance_id: test_instance_id(),
            executable: PathBuf::from("test"),
            args: vec![],
            spawn_phase: phase,
            health_checks: 0,
            spawn_attempts,
            last_error: None,
        };

        let should = should_respawn(&record, max_attempts);

        // should_respawn should be true only when:
        // phase is Failed AND spawn_attempts < max_attempts
        let expected = matches!(phase, SpawnPhase::Failed) && spawn_attempts < max_attempts;

        prop_assert_eq!(
            should, expected,
            "should_respawn({:?}, attempts={}, max={}) = {}, expected {}",
            phase, spawn_attempts, max_attempts, should, expected
        );
    }
}

// =============================================================================
// Proptest Invariants - SpawnRecord Defaults
// =============================================================================

proptest! {
    #[test]
    fn spawn_record_new_defaults(instance_id: ArbitraryInstanceId, command: String, spawn_id: Option<String>) {
        let instance_id = instance_id.0;
        let spawn_id = spawn_id.map(vo_types::SpawnId::new);
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from(&command), vec![], spawn_id.clone());

        prop_assert_eq!(record.spawn_phase, SpawnPhase::Spawn);
        prop_assert_eq!(record.spawn_attempts, 1);
        prop_assert_eq!(record.health_checks, 0);
        prop_assert_eq!(record.last_error, None);
        prop_assert_eq!(record.instance_id, instance_id);
        prop_assert_eq!(record.executable, PathBuf::from(&command));
        prop_assert_eq!(record.args, Vec::<String>::new());
        prop_assert_eq!(record.spawn_id, spawn_id);
    }
}
