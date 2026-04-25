use crate::record::{SpawnRecord, SpawnPhase};

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

#[inline]
#[must_use]
pub fn is_zombie_state(record: &SpawnRecord) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts > 3
}

#[inline]
#[must_use]
pub fn should_respawn(record: &SpawnRecord, max_attempts: u32) -> bool {
    matches!(record.spawn_phase, SpawnPhase::Failed) && record.spawn_attempts < max_attempts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::SpawnRecord;
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
            command: "test".to_string(),
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
            command: "test".to_string(),
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
            command: "test".to_string(),
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
            command: "test".to_string(),
            spawn_phase: SpawnPhase::Failed,
            health_checks: 0,
            spawn_attempts: 5,
            last_error: None,
        };

        assert!(!should_respawn(&record, 5));
    }
}