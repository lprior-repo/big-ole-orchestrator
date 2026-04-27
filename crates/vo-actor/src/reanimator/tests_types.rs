//! Unit tests for Reanimator data types, config, state, and errors.

use std::time::Duration;
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::{
    types::{FairnessBudget, ReanimatorConfig, ReanimatorState, TimerRecord, TimerScanResult},
    ReanimatorError,
};

// Helper function to create TimestampMs from u64 without unwrap in test code
fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

// =============================================================================
// Error Type Tests
// =============================================================================

mod reanimator_error_tests {
    use super::*;

    #[test]
    fn storage_error_is_transient() {
        let err = ReanimatorError::StorageError("disk full".to_string());
        assert!(err.is_transient());
        assert!(!err.is_fatal());
    }

    #[test]
    fn corrupt_key_is_fatal() {
        let err = ReanimatorError::CorruptKey("invalid format".to_string());
        assert!(!err.is_transient());
        assert!(err.is_fatal());
    }

    #[test]
    fn atomicity_violation_is_transient() {
        let err = ReanimatorError::AtomicityViolation("partial update".to_string());
        assert!(err.is_transient());
        assert!(!err.is_fatal());
    }

    #[test]
    fn budget_exceeded_is_transient() {
        let err = ReanimatorError::BudgetExceeded("limit reached".to_string());
        assert!(err.is_transient());
        assert!(!err.is_fatal());
    }

    #[test]
    fn already_running_is_fatal() {
        let err = ReanimatorError::AlreadyRunning;
        assert!(!err.is_transient());
        assert!(err.is_fatal());
    }

    #[test]
    fn already_shutdown_is_fatal() {
        let err = ReanimatorError::AlreadyShutdown;
        assert!(!err.is_transient());
        assert!(err.is_fatal());
    }

    #[test]
    fn error_display_format() {
        let err = ReanimatorError::StorageError("test error".to_string());
        assert_eq!(format!("{}", err), "Storage error: test error");

        let err = ReanimatorError::CorruptKey("bad key".to_string());
        assert_eq!(format!("{}", err), "Corrupt key format: bad key");
    }
}

// =============================================================================
// TimerRecord Tests
// =============================================================================

mod timer_record_tests {
    use super::*;

    #[test]
    fn timer_record_constructor() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let fire_at = ts_ms(1000);
        let scheduled = ts_ms(500);
        let timer_id = vo_types::TimerId::parse("timer-1").unwrap();

        let record = TimerRecord::new(instance_id.clone(), fire_at, Some(timer_id), scheduled);

        assert_eq!(record.instance_id, instance_id);
        assert_eq!(record.fire_at_ms, fire_at);
        assert_eq!(record.scheduled_at_ms, scheduled);
    }

    #[test]
    fn timer_record_without_timer_id() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let fire_at = ts_ms(1000);
        let scheduled = ts_ms(500);

        let record = TimerRecord::new(instance_id.clone(), fire_at, None, scheduled);

        assert_eq!(record.instance_id, instance_id);
        assert!(record.timer_id.is_none());
    }
}

// =============================================================================
// TimerScanResult Tests
// =============================================================================

mod timer_scan_result_tests {
    use super::*;

    #[test]
    fn scan_result_empty() {
        let result = TimerScanResult::new(Vec::new(), ts_ms(1000), 0);
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn scan_result_with_timers() {
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let timers = vec![
            TimerRecord::new(instance_id.clone(), ts_ms(1000), None, ts_ms(500)),
            TimerRecord::new(instance_id, ts_ms(2000), None, ts_ms(1500)),
        ];
        let result = TimerScanResult::new(timers, ts_ms(3000), 5);

        assert!(!result.is_empty());
        assert_eq!(result.len(), 2);
        assert_eq!(result.skipped_count, 5);
    }
}

// =============================================================================
// FairnessBudget Tests
// =============================================================================

mod fairness_budget_tests {
    use super::*;

    #[test]
    fn budget_default_limits() {
        let budget = FairnessBudget::default();
        assert_eq!(budget.max_per_instance, 5);
        assert!(budget.instance_counts.is_empty());
    }

    #[test]
    fn budget_custom_limits() {
        let budget = FairnessBudget::with_limits(10, 100);
        assert_eq!(budget.max_per_instance, 10);
        assert_eq!(budget.max_per_workflow, 100);
    }

    #[test]
    fn can_resume_allows_first_resume() {
        let budget = FairnessBudget::default();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        assert!(budget.can_resume(&instance_id));
    }

    #[test]
    fn can_resume_blocks_after_limit() {
        let mut budget = FairnessBudget::with_limits(2, 100);
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        assert!(budget.record_resume(instance_id.clone()));
        assert!(budget.record_resume(instance_id.clone()));
        assert!(!budget.can_resume(&instance_id));
    }

    #[test]
    fn reset_clears_counts() {
        let mut budget = FairnessBudget::with_limits(1, 100);
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        assert!(budget.record_resume(instance_id.clone()));
        assert!(!budget.can_resume(&instance_id));

        budget.reset();
        assert!(budget.can_resume(&instance_id));
    }

    #[test]
    fn different_instances_have_separate_counts() {
        let mut budget = FairnessBudget::with_limits(1, 100);
        let instance1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        assert!(budget.record_resume(instance1.clone()));
        assert!(!budget.can_resume(&instance1));
        assert!(budget.can_resume(&instance2));
    }
}

// =============================================================================
// ReanimatorState Tests
// =============================================================================

mod reanimator_state_tests {
    use super::*;

    #[test]
    fn stopped_is_not_active() {
        let state = ReanimatorState::Stopped;
        assert!(!state.is_active());
    }

    #[test]
    fn running_is_active() {
        let state = ReanimatorState::Running;
        assert!(state.is_active());
    }

    #[test]
    fn shutting_down_is_active() {
        let state = ReanimatorState::ShuttingDown;
        assert!(state.is_active());
    }

    #[test]
    fn shut_down_is_not_active() {
        let state = ReanimatorState::ShutDown;
        assert!(!state.is_active());
    }
}

// =============================================================================
// ReanimatorConfig Tests
// =============================================================================

mod reanimator_config_tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ReanimatorConfig::default();
        assert_eq!(config.scan_interval, Duration::from_secs(1));
        assert_eq!(config.max_timers_per_cycle, 100);
        assert_eq!(config.max_concurrent_resumes, 10);
        assert_eq!(config.shutdown_timeout, Duration::from_secs(30));
    }

    #[test]
    fn custom_config() {
        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(500),
            max_timers_per_cycle: 50,
            max_concurrent_resumes: 5,
            shutdown_timeout: Duration::from_secs(60),
        };
        assert_eq!(config.scan_interval, Duration::from_millis(500));
        assert_eq!(config.max_timers_per_cycle, 50);
        assert_eq!(config.max_concurrent_resumes, 5);
        assert_eq!(config.shutdown_timeout, Duration::from_secs(60));
    }
}
