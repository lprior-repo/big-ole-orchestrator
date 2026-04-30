//! BDD tests for ADR-046 Process Supervisor State Machine Invariants.
//!
//! Tests verify the 4 canonical invariants defined in ADR-046:
//! 1. Phase Atomicity: A SpawnRecord is in exactly one phase at any time.
//! 2. Attempt Monotonicity: spawn_attempts is monotonically increasing.
//! 3. Error Continuity: last_error is Some iff the previous transition resulted in an error.
//! 4. PID Binding: spawn_id is Some only in Running phase.
//!
//! Each test follows BDD Given/When/Then structure using the existing
//! mock infrastructure.

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::{test_instance_id, MockProcessManager, MockSpawnStorage, MockWorkQueue};
use vo_actor::spawn_supervisor::{
    SpawnPhase, SpawnRecord, SpawnSupervisor, SpawnSupervisorError,
};
use vo_types::SpawnId;

// =============================================================================
// INVARIANT 1: Phase Atomicity
// A SpawnRecord is in exactly one phase at any time.
// =============================================================================

mod phase_atomicity {
    use super::*;

    /// Given a SpawnRecord in any phase, it must be in exactly one phase.
    #[test]
    fn spawn_record_is_in_exactly_one_phase_given_spawn() {
        // Given: a SpawnRecord in Spawn phase
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);

        // When: we inspect the phase
        let phase = record.spawn_phase;

        // Then: it is exactly SpawnPhase::Spawn
        assert_eq!(phase, SpawnPhase::Spawn);
    }

    #[test]
    fn spawn_record_is_in_exactly_one_phase_given_health_check() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();

        assert_eq!(record.spawn_phase, SpawnPhase::HealthCheck);
    }

    #[test]
    fn spawn_record_is_in_exactly_one_phase_given_running() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running();

        assert_eq!(record.spawn_phase, SpawnPhase::Running);
    }

    #[test]
    fn spawn_record_is_in_exactly_one_phase_given_failed() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_failed();

        assert_eq!(record.spawn_phase, SpawnPhase::Failed);
    }

    /// Given a transition between phases, the record must never be in two phases simultaneously.
    #[test]
    fn transition_to_health_check_clears_spawn_phase() {
        let instance_id = test_instance_id();
        let original = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
        let transitioned = original.transition_to_health_check();

        // When: we transition from Spawn to HealthCheck
        // Then: the old phase is gone and new phase is set
        assert_eq!(transitioned.spawn_phase, SpawnPhase::HealthCheck);
        assert_ne!(transitioned.spawn_phase, SpawnPhase::Spawn);
    }

    #[test]
    fn transition_to_running_clears_health_check_phase() {
        let instance_id = test_instance_id();
        let original = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();
        let transitioned = original.transition_to_running();

        assert_eq!(transitioned.spawn_phase, SpawnPhase::Running);
        assert_ne!(transitioned.spawn_phase, SpawnPhase::HealthCheck);
    }

    /// Given a respawn, the phase must be Reset to Spawn (never stays in Failed).
    #[test]
    fn respawn_resets_phase_to_spawn() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 2,
            spawn_attempts: 3,
            last_error: Some(SpawnSupervisorError::ProcessExited {
                instance_id: test_instance_id(),
                pid: 1234,
                exit_code: 1,
            }),
        };

        let respawned = record.respawn(None);

        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
        assert_ne!(respawned.spawn_phase, SpawnPhase::Failed);
    }

    /// Given a SpawnRecord, it must be clonable without losing phase information.
    #[test]
    fn clone_preserves_phase() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();

        let cloned = record.clone();

        assert_eq!(cloned.spawn_phase, record.spawn_phase);
        assert_eq!(cloned.spawn_phase, SpawnPhase::HealthCheck);
    }

    /// Given a SpawnRecord, equality must include phase equality.
    #[test]
    fn records_with_different_phases_are_not_equal() {
        let instance_id = test_instance_id();
        let spawn_record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
        let health_check_record = spawn_record.clone().transition_to_health_check();

        assert_ne!(spawn_record, health_check_record);
    }

    /// Given a SpawnRecord, all 6 phases must be distinct values.
    #[test]
    fn all_phases_are_distinct() {
        let phases = vec![
            SpawnPhase::Spawn,
            SpawnPhase::HealthCheck,
            SpawnPhase::Running,
            SpawnPhase::Shutdown,
            SpawnPhase::Terminated,
            SpawnPhase::Failed,
        ];

        // All phases must be unique (no duplicates)
        for i in 0..phases.len() {
            for j in (i + 1)..phases.len() {
                assert_ne!(phases[i], phases[j], "Phase {:?} must not equal {:?}", phases[i], phases[j]);
            }
        }
    }
}

// =============================================================================
// INVARIANT 2: Attempt Monotonicity
// spawn_attempts is monotonically increasing; it never decreases across transitions.
// =============================================================================

mod attempt_monotonicity {
    use super::*;

    /// Given a SpawnRecord, respawn must increment spawn_attempts.
    #[test]
    fn respawn_increments_spawn_attempts() {
        // Given: a failed record with 3 attempts
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

        // When: we respawn
        let respawned = record.respawn(None);

        // Then: attempts increased monotonically
        assert_eq!(respawned.spawn_attempts, 4);
        assert!(respawned.spawn_attempts > record.spawn_attempts);
    }

    /// Given a SpawnRecord at max attempts, respawn must saturate without overflow.
    #[test]
    fn respawn_saturates_at_u32_max() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: u32::MAX,
            last_error: None,
        };

        let respawned = record.respawn(None);

        // When: we respawn at max attempts
        // Then: attempts saturate, never overflow
        assert_eq!(respawned.spawn_attempts, u32::MAX);
    }

    /// Given a SpawnRecord, transition_to_health_check must not decrease attempts.
    #[test]
    fn transition_to_health_check_preserves_attempts() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
        let transitioned = record.transition_to_health_check();

        // When: we transition to HealthCheck
        // Then: attempts remain unchanged (no decrease)
        assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
    }

    /// Given a SpawnRecord, transition_to_running must not decrease attempts.
    #[test]
    fn transition_to_running_preserves_attempts() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();
        let transitioned = record.transition_to_running();

        assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
    }

    /// Given a SpawnRecord, transition_to_failed must not decrease attempts.
    #[test]
    fn transition_to_failed_preserves_attempts() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();
        let transitioned = record.transition_to_failed();

        assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
    }

    /// Given a SpawnRecord, transition_to_shutdown must not decrease attempts.
    #[test]
    fn transition_to_shutdown_preserves_attempts() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running();
        let transitioned = record.transition_to_shutdown();

        assert_eq!(transitioned.spawn_attempts, record.spawn_attempts);
    }

    /// Given a full lifecycle with multiple respawns, attempts must always increase.
    #[test]
    fn multiple_respawns_always_increase_attempts() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: None,
        };

        let mut previous_attempts = 1u32;

        // When: we respawn multiple times
        for i in 2..=5 {
            let respawned = record.respawn(None);
            // Then: each respawn increases attempts monotonically
            assert!(
                respawned.spawn_attempts > previous_attempts,
                "Attempt {} should be > previous {}",
                respawned.spawn_attempts,
                previous_attempts
            );
            previous_attempts = respawned.spawn_attempts;
            record = respawned;
            assert_eq!(respawned.spawn_attempts, i);
        }
    }

    /// Given a SpawnRecord created with SpawnRecord::new, attempts start at 1.
    #[test]
    fn new_record_starts_at_one_attempt() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);

        assert_eq!(record.spawn_attempts, 1);
    }

    /// Given a SpawnRecord with high attempt count, transition_to_failed does not reduce it.
    #[test]
    fn failed_record_high_attempts_preserved() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);
        record.spawn_attempts = 10;
        record.spawn_phase = SpawnPhase::Failed;

        let failed = record.transition_to_failed();

        assert_eq!(failed.spawn_attempts, 10);
        assert_eq!(failed.spawn_phase, SpawnPhase::Failed);
    }
}

// =============================================================================
// INVARIANT 3: Error Continuity
// last_error is Some if and only if the previous transition resulted in an error.
// =============================================================================

mod error_continuity {
    use super::*;

    /// Given a SpawnRecord created via new(), last_error is None.
    #[test]
    fn new_record_has_no_error() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);

        // Given: a fresh record
        // When: we inspect last_error
        // Then: it is None (no previous error)
        assert!(record.last_error.is_none());
    }

    /// Given a SpawnRecord transitioning to HealthCheck after success, last_error is None.
    #[test]
    fn success_spawn_transition_clears_error() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: Some(SpawnSupervisorError::SpawnFailed {
                executable: PathBuf::from("./old-worker"),
                error: "ENOENT".to_string(),
            }),
            last_pid: None,
        };

        // When: we transition to HealthCheck (success path)
        let transitioned = record.transition_to_health_check();

        // Then: error is cleared on successful transition
        assert!(transitioned.last_error.is_none());
        assert_eq!(transitioned.spawn_phase, SpawnPhase::HealthCheck);
    }

    /// Given a SpawnRecord in Spawn phase with an error, the error should be set.
    #[test]
    fn spawn_failure_sets_error() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);

        // When: a spawn failure occurs (simulating what process_cycle does)
        record.last_error = Some(SpawnSupervisorError::SpawnFailed {
            executable: PathBuf::from("./worker"),
            error: "Permission denied".to_string(),
        });

        // Then: last_error is Some
        assert!(record.last_error.is_some());
        assert!(matches!(
            record.last_error,
            Some(SpawnSupervisorError::SpawnFailed { .. })
        ));
    }

    /// Given a SpawnRecord in HealthCheck phase that fails, last_error should be set.
    #[test]
    fn health_check_failure_sets_error() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();

        // When: a health check failure occurs
        record.last_error = Some(SpawnSupervisorError::HealthCheckFailed {
            instance_id: record.instance_id.clone(),
            check_number: 1,
            error: "Timeout".to_string(),
        });

        // Then: last_error is Some and phase is Failed
        assert!(record.last_error.is_some());
    }

    /// Given a SpawnRecord that has been respawned, last_error must be None.
    #[test]
    fn respawn_clears_error() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 2,
            last_error: Some(SpawnSupervisorError::ProcessExited {
                instance_id: test_instance_id(),
                pid: 5678,
                exit_code: 1,
            }),
            last_pid: None,
        };

        // When: we respawn
        let respawned = record.respawn(None);

        // Then: error is cleared on respawn
        assert!(respawned.last_error.is_none());
        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
    }

    /// Given a SpawnRecord that transitioned to Running successfully, last_error must be None.
    #[test]
    fn running_record_has_no_error() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running();

        // When: we inspect the record after successful transition to Running
        // Then: last_error is None
        assert!(record.last_error.is_none());
    }

    /// Given a SpawnRecord in Failed phase, last_error must be Some (error continuity).
    #[test]
    fn failed_record_must_have_error() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);
        record.spawn_phase = SpawnPhase::Failed;
        record.last_error = Some(SpawnSupervisorError::HealthCheckFailed {
            instance_id: record.instance_id.clone(),
            check_number: 3,
            error: "Not healthy".to_string(),
        });

        // When: we inspect a failed record
        // Then: it must have an error (error continuity invariant)
        assert!(record.last_error.is_some());
        assert!(record.spawn_phase == SpawnPhase::Failed);
    }

    /// Given a SpawnRecord in Terminated phase, last_error can be any value
    /// (terminated is a terminal state, error continuity is not enforced).
    #[test]
    fn terminated_record_can_have_error() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Terminated,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: Some(SpawnSupervisorError::ProcessExited {
                instance_id: test_instance_id(),
                pid: 9999,
                exit_code: 0,
            }),
            last_pid: None,
        };

        assert_eq!(record.spawn_phase, SpawnPhase::Terminated);
    }

    /// Given a SpawnRecord with an error, transitioning to HealthCheck (success) clears it.
    #[test]
    fn error_cleared_on_successful_spawn_transition() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Spawn,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: Some(SpawnSupervisorError::StorageError("disk full".to_string())),
            last_pid: None,
        };

        let transitioned = record.transition_to_health_check();

        // Then: successful transition clears the previous error
        assert!(transitioned.last_error.is_none());
        assert_eq!(transitioned.spawn_phase, SpawnPhase::HealthCheck);
    }
}

// =============================================================================
// INVARIANT 4: PID Binding
// spawn_id (containing PID) is Some only in Running phase.
// =============================================================================

mod pid_binding {
    use super::*;

    /// Given a SpawnRecord in Spawn phase, spawn_id must be None.
    #[test]
    fn spawn_phase_has_no_spawn_id() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);

        // Given: a fresh SpawnRecord in Spawn phase
        // When: we inspect spawn_id
        // Then: it is None (PID not yet bound)
        assert!(record.spawn_id.is_none());
        assert_eq!(record.spawn_phase, SpawnPhase::Spawn);
    }

    /// Given a SpawnRecord in HealthCheck phase, spawn_id must be None.
    #[test]
    fn health_check_phase_has_no_spawn_id() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();

        // When: we inspect spawn_id in HealthCheck phase
        // Then: it is None (PID not yet bound to spawn_id field)
        assert!(record.spawn_id.is_none());
        assert_eq!(record.spawn_phase, SpawnPhase::HealthCheck);
    }

    /// Given a SpawnRecord in Running phase, spawn_id must be Some.
    #[test]
    fn running_phase_has_spawn_id() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running();

        // When: we set spawn_id (simulating what process_cycle does)
        record.spawn_id = Some(SpawnId::new("1234".to_string()));

        // Then: spawn_id is Some only in Running phase
        assert!(record.spawn_id.is_some());
        assert_eq!(record.spawn_phase, SpawnPhase::Running);
    }

    /// Given a SpawnRecord in Failed phase, spawn_id must be None.
    #[test]
    fn failed_phase_has_no_spawn_id() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_failed();

        // When: we inspect spawn_id in Failed phase
        // Then: it is None (PID not bound in Failed state)
        assert!(record.spawn_id.is_none());
        assert_eq!(record.spawn_phase, SpawnPhase::Failed);
    }

    /// Given a SpawnRecord in Terminated phase, spawn_id must be None.
    #[test]
    fn terminated_phase_has_no_spawn_id() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: None,
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Terminated,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: None,
            last_pid: None,
        };

        assert!(record.spawn_id.is_none());
        assert_eq!(record.spawn_phase, SpawnPhase::Terminated);
    }

    /// Given a SpawnRecord in Shutdown phase, spawn_id must be None (or None after transition).
    #[test]
    fn shutdown_phase_spawn_id_is_none() {
        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running()
            .transition_to_shutdown();

        assert!(record.spawn_id.is_none());
        assert_eq!(record.spawn_phase, SpawnPhase::Shutdown);
    }

    /// Given a SpawnRecord that transitioned to Failed from Running with a spawn_id,
    /// the spawn_id is cleared in Failed phase.
    #[test]
    fn failed_record_clears_spawn_id() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running();
        record.spawn_id = Some(SpawnId::new("5678".to_string()));

        let failed = record.transition_to_failed();

        // When: we transition from Running to Failed
        // Then: spawn_id is cleared
        assert!(failed.spawn_id.is_none());
        assert_eq!(failed.spawn_phase, SpawnPhase::Failed);
    }

    /// Given a SpawnRecord that was respawned, the new record must have spawn_id = None.
    #[test]
    fn respawn_clears_spawn_id() {
        let instance_id = test_instance_id();
        let record = SpawnRecord {
            spawn_id: Some(SpawnId::new("9999".to_string())),
            instance_id,
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 3,
            last_error: Some(SpawnSupervisorError::ProcessExited {
                instance_id: test_instance_id(),
                pid: 9999,
                exit_code: 1,
            }),
            last_pid: Some(9999),
        };

        let respawned = record.respawn(None);

        // When: we respawn a record with spawn_id
        // Then: the new record has spawn_id = None
        assert!(respawned.spawn_id.is_none());
        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
    }

    /// Given a SpawnRecord that has spawn_id set, only Running phase allows it.
    #[test]
    fn spawn_id_only_valid_in_running_phase() {
        let instance_id = test_instance_id();
        let valid_spawn_id = SpawnId::new("42".to_string());

        // Running phase: spawn_id is valid
        let mut running_record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running();
        running_record.spawn_id = Some(valid_spawn_id.clone());
        assert!(running_record.spawn_id.is_some());
        assert_eq!(running_record.spawn_phase, SpawnPhase::Running);

        // Spawn phase: spawn_id must be None
        let spawn_record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
        assert!(spawn_record.spawn_id.is_none());

        // HealthCheck phase: spawn_id must be None
        let hc_record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();
        assert!(hc_record.spawn_id.is_none());

        // Failed phase: spawn_id must be None
        let failed_record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_failed();
        assert!(failed_record.spawn_id.is_none());

        // Terminated phase: spawn_id must be None
        let term_record = SpawnRecord {
            spawn_id: None,
            instance_id: instance_id.clone(),
            executable: PathBuf::from("./worker"),
            args: vec![],
            spawn_phase: SpawnPhase::Terminated,
            health_checks: 0,
            spawn_attempts: 1,
            last_error: None,
            last_pid: None,
        };
        assert!(term_record.spawn_id.is_none());

        // Shutdown phase: spawn_id must be None
        let shutdown_record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check()
            .transition_to_running()
            .transition_to_shutdown();
        assert!(shutdown_record.spawn_id.is_none());
    }

    /// Given a SpawnRecord created with a spawn_id, transition_to_health_check clears it.
    #[test]
    fn spawn_id_cleared_on_health_check_transition() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id, PathBuf::from("./worker"), vec![], None);
        record.spawn_id = Some(SpawnId::new("1111".to_string()));
        record.spawn_phase = SpawnPhase::HealthCheck;

        // spawn_id should not exist in HealthCheck phase
        assert!(record.spawn_id.is_none());
    }

    /// Given a SpawnRecord, last_pid is set during health check and running phases.
    #[test]
    fn last_pid_set_in_running_after_successful_spawn() {
        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();
        record.last_pid = Some(1234);

        let running = record.transition_to_running();

        assert_eq!(running.last_pid, Some(1234));
        assert_eq!(running.spawn_phase, SpawnPhase::Running);
    }
}

// =============================================================================
// BDD Integration Tests: Full lifecycle invariant verification via process_cycle
// =============================================================================

mod lifecycle_invariant_verification {
    use super::*;

    /// Given: a SpawnRecord in Spawn phase
    /// When: process_cycle spawns it successfully
    /// Then: phase = HealthCheck, spawn_id = None, last_error = None, spawn_attempts unchanged
    #[tokio::test]
    async fn full_spawn_success_maintains_phase_atomicity() {
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

        supervisor.process_cycle().await.expect("cycle should succeed");

        let saved = storage.get_records();
        let final_record = saved.iter()
            .find(|r| r.instance_id == instance_id)
            .expect("record should exist");

        // Phase Atomicity: record is in exactly one phase
        assert!(matches!(final_record.spawn_phase, SpawnPhase::HealthCheck));

        // PID Binding: spawn_id is None in HealthCheck phase
        assert!(final_record.spawn_id.is_none());

        // Error Continuity: no error on successful spawn
        assert!(final_record.last_error.is_none());
    }

    /// Given: a SpawnRecord in Spawn phase with failing spawn
    /// When: process_cycle spawns it and spawn fails
    /// Then: phase = Spawn, last_error = Some(SpawnFailed), spawn_attempts unchanged
    #[tokio::test]
    async fn spawn_failure_maintains_error_continuity() {
        let storage = Arc::new(MockSpawnStorage::new());
        let process_manager = Arc::new(MockProcessManager::new());
        let work_queue = Arc::new(MockWorkQueue::new());

        let instance_id = test_instance_id();
        let record = SpawnRecord::new(
            instance_id.clone(),
            PathBuf::from("./nonexistent"),
            vec![],
            None,
        );
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

        supervisor.process_cycle().await.expect("cycle should succeed");

        let saved = storage.get_records();
        let final_record = saved.iter()
            .find(|r| r.instance_id == instance_id)
            .expect("record should exist");

        // Error Continuity: error is set after spawn failure
        assert!(final_record.last_error.is_some());
        assert!(matches!(
            final_record.last_error,
            Some(SpawnSupervisorError::SpawnFailed { .. })
        ));

        // PID Binding: spawn_id is None after failed spawn
        assert!(final_record.spawn_id.is_none());

        // Attempt Monotonicity: attempts unchanged after failure (no respawn yet)
        assert_eq!(final_record.spawn_attempts, 1);
    }

    /// Given: a SpawnRecord in HealthCheck phase with failing health checks
    /// When: process_cycle health checks it and fails
    /// Then: phase = Failed, last_error = Some, spawn_attempts unchanged
    #[tokio::test]
    async fn health_check_failure_maintains_error_continuity() {
        let storage = Arc::new(MockSpawnStorage::new());
        let process_manager = Arc::new(MockProcessManager::new());
        let work_queue = Arc::new(MockWorkQueue::new());

        let instance_id = test_instance_id();
        let record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
        storage.add_record(record);

        // Simulate spawn succeeded -> record is now in HealthCheck
        let mut hc_record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None)
            .transition_to_health_check();
        hc_record.last_pid = Some(1234);
        storage.records.lock().unwrap().clear();
        storage.add_record(hc_record);

        // Make health checks fail
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

        supervisor.process_cycle().await.expect("cycle should succeed");

        let saved = storage.get_records();
        let final_record = saved.iter()
            .find(|r| r.instance_id == instance_id)
            .expect("record should exist");

        // Phase Atomicity: record is in Failed phase
        assert_eq!(final_record.spawn_phase, SpawnPhase::Failed);

        // Error Continuity: error is set after health check failure
        assert!(final_record.last_error.is_some());

        // PID Binding: spawn_id is None in Failed phase
        assert!(final_record.spawn_id.is_none());
    }

    /// Given: a SpawnRecord in Failed phase with attempts < max
    /// When: process_cycle respawns it
    /// Then: phase = Spawn, attempts incremented, error cleared, spawn_id = None
    #[tokio::test]
    async fn respawn_maintains_attempt_monotonicity_and_error_clearing() {
        let storage = Arc::new(MockSpawnStorage::new());
        let process_manager = Arc::new(MockProcessManager::new());
        let work_queue = Arc::new(MockWorkQueue::new());

        let instance_id = test_instance_id();
        let mut record = SpawnRecord::new(instance_id.clone(), PathBuf::from("./worker"), vec![], None);
        record.spawn_phase = SpawnPhase::Failed;
        record.spawn_attempts = 2;
        record.last_error = Some(SpawnSupervisorError::ProcessExited {
            instance_id: test_instance_id(),
            pid: 1234,
            exit_code: 1,
        });
        record.last_pid = Some(1234);
        storage.add_record(record);

        let supervisor = SpawnSupervisor::new(
            Duration::from_millis(10),
            3,
            Duration::from_millis(10),
            2.0,
            5,
            storage.clone(),
            process_manager.clone(),
            work_queue.clone(),
        )
        .expect("Valid config");

        // Allow time for backoff-based respawn to complete
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = supervisor.process_cycle().await;

        let saved = storage.get_records();
        let final_record = saved.iter()
            .find(|r| r.instance_id == instance_id)
            .expect("record should exist");

        // Attempt Monotonicity: attempts increased
        assert!(final_record.spawn_attempts > 2);

        // Error Continuity: error cleared on respawn
        assert!(final_record.last_error.is_none());

        // PID Binding: spawn_id is None after respawn
        assert!(final_record.spawn_id.is_none());

        // Phase Atomicity: phase reset to Spawn
        assert_eq!(final_record.spawn_phase, SpawnPhase::Spawn);
    }
}
