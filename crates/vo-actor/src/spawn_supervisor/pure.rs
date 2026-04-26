//! Pure calculation functions for spawn supervisor.
//!
//! These functions have no side effects and are fully unit-testable.
//! They implement the Data -> Calc layer of the architecture.

use super::types::{SpawnPhase, SpawnRecord};

// =============================================================================
// Pure Calculation Functions (Data -> Calc -> Actions)
// =============================================================================

/// `calculate_backoff_delay` - Calculate exponential backoff delay
///
/// Formula: `initial_backoff * backoff_multiplier^(attempt - 1)`
///
/// This function is a pure calculation with no side effects.
///
/// # Arguments
/// * `initial_backoff_ms` - Initial backoff duration in milliseconds
/// * `backoff_multiplier` - Multiplier for exponential backoff
/// * `attempt` - Current attempt number (1-indexed)
///
/// # Returns
/// Backoff delay in milliseconds
#[inline]
#[must_use]
pub fn calculate_backoff_delay(
    initial_backoff_ms: u64,
    backoff_multiplier: f64,
    attempt: u32,
) -> u64 {
    let exponent = attempt.saturating_sub(1) as f64;
    let multiplier_pow = backoff_multiplier.powf(exponent);
    #[allow(clippy::cast_possible_truncation)]
    let result = (initial_backoff_ms as f64 * multiplier_pow) as u64;
    result
}

/// `is_zombie_state` - Check if spawn record indicates zombie state
///
/// Returns true if spawn is in failed phase with high attempt count.
///
/// # Arguments
/// * `record` - The spawn record to check
///
/// # Returns
/// `true` if the spawn appears to be a zombie
#[inline]
#[must_use]
pub fn is_zombie_state(record: &SpawnRecord) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts > 3
}

/// `should_respawn` - Check if spawn should be respawned
///
/// Returns true if spawn is in failed phase and attempts are within limit.
///
/// # Arguments
/// * `record` - The spawn record to check
/// * `max_attempts` - Maximum allowed attempts
///
/// # Returns
/// `true` if the spawn should be respawned
#[inline]
#[must_use]
pub fn should_respawn(record: &SpawnRecord, max_attempts: u32) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts < max_attempts
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;
    use ulid::Ulid;
    use vo_types::InstanceId;

    fn test_instance_id() -> InstanceId {
        let ulid = Ulid::new();
        InstanceId::from_bytes(ulid.to_bytes())
    }

    #[test]
    fn calculate_backoff_delay_returns_initial_for_first_attempt() {
        assert_eq!(calculate_backoff_delay(1000, 2.0, 1), 1000);
    }

    #[test]
    fn calculate_backoff_delay_applies_multiplier() {
        assert_eq!(calculate_backoff_delay(1000, 2.0, 2), 2000);
        assert_eq!(calculate_backoff_delay(1000, 2.0, 3), 4000);
    }

    #[test]
    fn calculate_backoff_delay_with_multiplier_1_0() {
        assert_eq!(calculate_backoff_delay(1000, 1.0, 1), 1000);
        assert_eq!(calculate_backoff_delay(1000, 1.0, 2), 1000);
        assert_eq!(calculate_backoff_delay(1000, 1.0, 10), 1000);
    }

    #[test]
    fn is_zombie_state_returns_true_for_failed_high_attempts() {
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
    fn is_zombie_state_returns_false_for_low_attempts() {
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

        assert!(!is_zombie_state(&record));
    }

    #[test]
    fn should_respawn_returns_true_within_limit() {
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
    fn should_respawn_returns_false_at_limit() {
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
    fn spawn_record_transitions_correctly() {
        let record = SpawnRecord::new(
            test_instance_id(),
            PathBuf::from("test"),
            vec![],
            None,
        );

        assert_eq!(record.spawn_phase, SpawnPhase::Spawn);

        let health_check_record = record.transition_to_health_check();
        assert_eq!(health_check_record.spawn_phase, SpawnPhase::HealthCheck);

        let running_record = health_check_record.transition_to_running();
        assert_eq!(running_record.spawn_phase, SpawnPhase::Running);

        let shutdown_record = running_record.transition_to_shutdown();
        assert_eq!(shutdown_record.spawn_phase, SpawnPhase::Shutdown);
    }

    #[test]
    fn spawn_record_respawn_increments_attempts() {
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

        let respawned = record.respawn(Some(vo_types::SpawnId::new("new-123".to_string())));
        assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
        assert_eq!(respawned.spawn_attempts, 4);
        assert_eq!(respawned.health_checks, 0);
    }

    #[test]
    fn spawn_supervisor_error_is_transient() {
        use super::super::types::SpawnSupervisorError;

        assert!(SpawnSupervisorError::StorageError("test".to_string()).is_transient());
        assert!(SpawnSupervisorError::InstanceNotFound(test_instance_id()).is_transient());
        assert!(SpawnSupervisorError::MailboxFull(test_instance_id()).is_transient());
        assert!(SpawnSupervisorError::DispatchError("test".to_string()).is_transient());
        assert!(!SpawnSupervisorError::InvalidConfig("test".to_string()).is_transient());
    }

    #[test]
    fn spawn_supervisor_error_is_resumable() {
        use super::super::types::SpawnSupervisorError;

        assert!(SpawnSupervisorError::HealthCheckFailed {
            instance_id: test_instance_id(),
            check_number: 1,
            error: "test".to_string()
        }
        .is_resumable());
        assert!(SpawnSupervisorError::ProcessExited {
            instance_id: test_instance_id(),
            pid: 123,
            exit_code: 1
        }
        .is_resumable());
        assert!(SpawnSupervisorError::SpawnFailed {
            executable: PathBuf::from("test"),
            error: "test".to_string()
        }
        .is_resumable());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_resumable());
    }

    #[test]
    fn spawn_supervisor_error_is_fatal() {
        use super::super::types::SpawnSupervisorError;

        assert!(SpawnSupervisorError::CorruptSpawn("test".to_string()).is_fatal());
        assert!(SpawnSupervisorError::InvalidConfig("test".to_string()).is_fatal());
        assert!(SpawnSupervisorError::ZombieDetected {
            instance_id: test_instance_id(),
            pid: 123
        }
        .is_fatal());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_fatal());
    }

    #[test]
    fn spawn_supervisor_error_is_operational() {
        use super::super::types::SpawnSupervisorError;

        assert!(SpawnSupervisorError::AlreadyRunning.is_operational());
        assert!(SpawnSupervisorError::AlreadyShutdown.is_operational());
        assert!(SpawnSupervisorError::NotRunning.is_operational());
        assert!(SpawnSupervisorError::ShutdownTimeout(std::time::Duration::from_secs(30)).is_operational());
        assert!(SpawnSupervisorError::AtomicityViolation("test".to_string()).is_operational());
        assert!(!SpawnSupervisorError::StorageError("test".to_string()).is_operational());
    }
}
