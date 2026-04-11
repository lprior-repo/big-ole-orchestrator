//! Unit tests for Reanimator calculation layer and handle.

use std::time::Duration;
use tokio::sync::{broadcast, watch};
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    loop_core::ReanimatorHandle,
    types::{
        calculate_batch_size, check_resume_budget, filter_timers_by_fairness,
        validate_timer_record, FairnessBudget, ReanimatorState, TimerRecord,
    },
    ReanimatorError,
};

// Helper function to create TimestampMs from u64 without unwrap in test code
fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

// =============================================================================
// Calculation Layer Tests
// =============================================================================

mod calculation_tests {
    use super::*;

    #[test]
    fn filter_timers_by_fairness_allows_within_budget() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timers = vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )];

        let budget = FairnessBudget::default();
        let (allowed, rejected) = filter_timers_by_fairness(timers.clone(), &budget);

        assert_eq!(allowed.len(), 1);
        assert_eq!(rejected.len(), 0);
    }

    #[test]
    fn filter_timers_by_fairness_rejects_over_budget() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let mut budget = FairnessBudget::with_limits(1, 100);

        // Exhaust budget
        assert!(budget.record_resume(instance_id.clone()));

        let timers = vec![TimerRecord::new(
            instance_id.clone(),
            ts_ms(1000),
            None,
            ts_ms(500),
        )];

        let (allowed, rejected) = filter_timers_by_fairness(timers, &budget);

        assert_eq!(allowed.len(), 0);
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn calculate_batch_size_respects_budget() {
        assert_eq!(calculate_batch_size(50, 100, 0), 50);
        assert_eq!(calculate_batch_size(50, 100, 30), 50);
        assert_eq!(calculate_batch_size(50, 100, 70), 30);
        assert_eq!(calculate_batch_size(50, 100, 100), 0);
        assert_eq!(calculate_batch_size(50, 100, 101), 0);
    }

    #[test]
    fn calculate_batch_size_respects_remaining() {
        assert_eq!(calculate_batch_size(10, 100, 0), 10);
        assert_eq!(calculate_batch_size(10, 100, 95), 5);
    }

    #[test]
    fn validate_timer_record_accepts_valid_record() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(1000),
            None,
            ts_ms(500),
        );
        assert_eq!(validate_timer_record(&record), Ok(()));
    }

    #[test]
    fn validate_timer_record_rejects_zero_fire_at() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(0),
            None,
            ts_ms(500),
        );
        let err = validate_timer_record(&record).unwrap_err();
        assert!(matches!(err, ReanimatorError::CorruptKey(_)));
    }

    #[test]
    fn validate_timer_record_rejects_zero_scheduled_at() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(1000),
            None,
            ts_ms(0),
        );
        let err = validate_timer_record(&record).unwrap_err();
        assert!(matches!(err, ReanimatorError::CorruptKey(_)));
    }

    #[test]
    fn validate_timer_record_rejects_fire_before_scheduled() {
        let record = TimerRecord::new(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            ts_ms(500),
            None,
            ts_ms(1000),
        );
        let err = validate_timer_record(&record).unwrap_err();
        assert!(matches!(err, ReanimatorError::CorruptKey(_)));
    }

    #[test]
    fn check_resume_budget_success() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let budget = FairnessBudget::default();

        let result = check_resume_budget(&instance_id, &budget);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn check_resume_budget_fails_over_budget() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let mut budget = FairnessBudget::with_limits(1, 100);

        // Exhaust budget
        assert!(budget.record_resume(instance_id.clone()));

        let result = check_resume_budget(&instance_id, &budget);
        assert_eq!(
            result,
            Err(ReanimatorError::BudgetExceeded(format!(
                "Instance {} has exceeded resume budget",
                instance_id
            )))
        );
    }
}

// =============================================================================
// ReanimatorHandle Tests
// =============================================================================

mod reanimator_handle_tests {
    use super::*;

    #[test]
    fn handle_initial_state() {
        let (state_sender, _) = watch::channel(ReanimatorState::Stopped);
        let (shutdown_trigger, _) = broadcast::channel(1);

        let handle = ReanimatorHandle {
            state_sender,
            shutdown_trigger,
            task_handle: None,
        };

        assert_eq!(handle.current_state(), ReanimatorState::Stopped);
    }
}
