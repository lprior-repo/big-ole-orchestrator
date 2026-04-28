//! Hibernation round-trip tests for Reanimator types.
//!
//! Tests serialization/deserialization of timer records, fairness budgets,
//! config, state, and errors to verify state preservation across
//! sleep/wake cycles and disk format stability.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use vo_types::{InstanceId, TimestampMs};

use crate::reanimator::types::{
    FairnessBudget, ReanimatorConfig, ReanimatorState, TimerRecord, TimerScanResult,
};
use crate::reanimator::{ReanimatorError, filter_timers_by_fairness};

fn ts_ms(value: u64) -> TimestampMs {
    TimestampMs::try_from(value).expect("valid timestamp")
}

// =============================================================================
// Serialization Round-Trip Tests (Hibernation: sleep -> wake)
// =============================================================================

mod hibernation_roundtrip {
    use super::*;

    fn sample_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    fn sample_timer_record() -> TimerRecord {
        TimerRecord::new(
            sample_instance_id(),
            ts_ms(1000),
            Some(vo_types::TimerId::parse("timer-1").unwrap()),
            ts_ms(500),
        )
    }

    #[test]
    fn timer_record_json_roundtrip() {
        let record = sample_timer_record();
        let json = serde_json::to_string(&record).expect("serialize TimerRecord");
        let deserialized: TimerRecord = serde_json::from_str(&json).expect("deserialize TimerRecord");
        assert_eq!(record, deserialized);
    }

    #[test]
    fn timer_record_bincode_roundtrip() {
        let record = sample_timer_record();
        let bytes = bincode::serialize(&record).expect("serialize TimerRecord");
        let deserialized: TimerRecord = bincode::deserialize(&bytes).expect("deserialize TimerRecord");
        assert_eq!(record, deserialized);
    }

    #[test]
    fn timer_record_preserves_all_fields() {
        let record = sample_timer_record();
        let json = serde_json::to_string(&record).expect("serialize");
        let deserialized: TimerRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.instance_id, record.instance_id);
        assert_eq!(deserialized.fire_at_ms, record.fire_at_ms);
        assert_eq!(deserialized.scheduled_at_ms, record.scheduled_at_ms);
        assert_eq!(deserialized.timer_id, record.timer_id);
    }

    #[test]
    fn timer_record_without_timer_id_roundtrip() {
        let record = TimerRecord::new(
            sample_instance_id(),
            ts_ms(2000),
            None,
            ts_ms(1500),
        );
        let json = serde_json::to_string(&record).expect("serialize");
        let deserialized: TimerRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.instance_id, record.instance_id);
        assert_eq!(deserialized.fire_at_ms, record.fire_at_ms);
        assert!(deserialized.timer_id.is_none());
        assert_eq!(deserialized.scheduled_at_ms, record.scheduled_at_ms);
    }

    #[test]
    fn timer_record_multiple_instances_roundtrip() {
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let id3 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap();

        let records = vec![
            TimerRecord::new(id1.clone(), ts_ms(1000), None, ts_ms(500)),
            TimerRecord::new(id2.clone(), ts_ms(2000), Some(vo_types::TimerId::parse("t-2").unwrap()), ts_ms(1500)),
            TimerRecord::new(id3.clone(), ts_ms(3000), None, ts_ms(2500)),
        ];

        let json = serde_json::to_string(&records).expect("serialize vec");
        let deserialized: Vec<TimerRecord> = serde_json::from_str(&json).expect("deserialize vec");
        assert_eq!(records.len(), deserialized.len());
        for (orig, deser) in records.iter().zip(deserialized.iter()) {
            assert_eq!(orig, deser);
        }
    }

    #[test]
    fn timer_scan_result_json_roundtrip() {
        let instance_id = sample_instance_id();
        let timers = vec![
            TimerRecord::new(instance_id.clone(), ts_ms(1000), None, ts_ms(500)),
            TimerRecord::new(instance_id, ts_ms(2000), None, ts_ms(1500)),
        ];
        let result = TimerScanResult::new(timers.clone(), ts_ms(3000), 5);

        let json = serde_json::to_string(&result).expect("serialize TimerScanResult");
        let deserialized: TimerScanResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(result, deserialized);
        assert_eq!(deserialized.timers.len(), 2);
        assert_eq!(deserialized.skipped_count, 5);
        assert_eq!(deserialized.scanned_at_ms, ts_ms(3000));
    }

    #[test]
    fn timer_scan_result_empty_roundtrip() {
        let result = TimerScanResult::new(Vec::new(), ts_ms(0), 0);
        let json = serde_json::to_string(&result).expect("serialize");
        let deserialized: TimerScanResult = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.is_empty());
        assert_eq!(deserialized.len(), 0);
    }

    #[test]
    fn fairness_budget_json_roundtrip() {
        let mut budget = FairnessBudget::with_limits(10, 100);
        let id1 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let id2 = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        budget.record_resume(id1.clone());
        budget.record_resume(id1.clone());
        budget.record_resume(id2.clone());

        let json = serde_json::to_string(&budget).expect("serialize FairnessBudget");
        let deserialized: FairnessBudget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(budget.max_per_instance, deserialized.max_per_instance);
        assert_eq!(budget.max_per_workflow, deserialized.max_per_workflow);
        assert_eq!(budget.instance_counts.len(), deserialized.instance_counts.len());
        assert_eq!(
            *budget.instance_counts.get(&id1).unwrap(),
            *deserialized.instance_counts.get(&id1).unwrap()
        );
        assert!(deserialized.can_resume(&id1));
    }

    #[test]
    fn fairness_budget_default_json_roundtrip() {
        let budget = FairnessBudget::default();
        let json = serde_json::to_string(&budget).expect("serialize");
        let deserialized: FairnessBudget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(budget.max_per_instance, deserialized.max_per_instance);
        assert_eq!(budget.max_per_workflow, deserialized.max_per_workflow);
        assert!(deserialized.instance_counts.is_empty());
    }

    #[test]
    fn reanimator_config_json_roundtrip() {
        let config = ReanimatorConfig {
            scan_interval: Duration::from_millis(500),
            max_timers_per_cycle: 50,
            max_concurrent_resumes: 5,
            shutdown_timeout: Duration::from_secs(60),
        };
        let json = serde_json::to_string(&config).expect("serialize ReanimatorConfig");
        let deserialized: ReanimatorConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn reanimator_config_default_json_roundtrip() {
        let config = ReanimatorConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: ReanimatorConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn reanimator_state_json_roundtrip() {
        for state in [
            ReanimatorState::Stopped,
            ReanimatorState::Running,
            ReanimatorState::ShuttingDown,
            ReanimatorState::ShutDown,
        ] {
            let json = serde_json::to_string(&state).expect("serialize state");
            let deserialized: ReanimatorState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(state, deserialized, "state roundtrip failed for {:?}", state);
        }
    }

    #[test]
    fn reanimator_error_json_roundtrip() {
        let errors = vec![
            ReanimatorError::StorageError("disk full".to_string()),
            ReanimatorError::CorruptKey("bad key".to_string()),
            ReanimatorError::AtomicityViolation("partial update".to_string()),
            ReanimatorError::InstanceNotFound(sample_instance_id()),
            ReanimatorError::BudgetExceeded("limit reached".to_string()),
            ReanimatorError::EnqueueFailed("queue full".to_string()),
            ReanimatorError::AlreadyRunning,
            ReanimatorError::StorageInitFailed("init failed".to_string()),
            ReanimatorError::TaskSpawnFailed("spawn failed".to_string()),
            ReanimatorError::AlreadyShutdown,
            ReanimatorError::ShutdownTimeout(Duration::from_secs(30)),
        ];

        for err in &errors {
            let json = serde_json::to_string(err).expect("serialize error");
            let deserialized: ReanimatorError = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(err, &deserialized, "error roundtrip failed for {:?}", err);
        }
    }

    #[test]
    fn hibernation_full_state_preservation() {
        let instance_id = sample_instance_id();
        let record = TimerRecord::new(
            instance_id.clone(),
            ts_ms(10000),
            Some(vo_types::TimerId::parse("hibernate-1").unwrap()),
            ts_ms(5000),
        );

        let mut budget = FairnessBudget::with_limits(3, 50);
        budget.record_resume(instance_id.clone());

        let scan_result = TimerScanResult::new(
            vec![record.clone()],
            ts_ms(10000),
            0,
        );

        let config = ReanimatorConfig {
            scan_interval: Duration::from_secs(2),
            max_timers_per_cycle: 200,
            max_concurrent_resumes: 20,
            shutdown_timeout: Duration::from_secs(120),
        };

        // Serialize all state as if persisting to disk before shutdown
        let state = serde_json::json!({
            "config": config,
            "state": ReanimatorState::ShuttingDown,
            "timers": vec![record],
            "budget": budget,
            "scan_result": scan_result,
        });

        let json = serde_json::to_string(&state).expect("serialize full state");

        // Deserialize after "boot" (wake from hibernation)
        let deserialized: serde_json::Value = serde_json::from_str(&json).expect("deserialize");

        let deser_config: ReanimatorConfig =
            serde_json::from_value(deserialized["config"].clone()).expect("deserialize config");
        let deser_state: ReanimatorState =
            serde_json::from_value(deserialized["state"].clone()).expect("deserialize state");
        let deser_timers: Vec<TimerRecord> =
            serde_json::from_value(deserialized["timers"].clone()).expect("deserialize timers");
        let deser_budget: FairnessBudget =
            serde_json::from_value(deserialized["budget"].clone()).expect("deserialize budget");
        let deser_scan: TimerScanResult =
            serde_json::from_value(deserialized["scan_result"].clone()).expect("deserialize scan");

        assert_eq!(config, deser_config);
        assert_eq!(ReanimatorState::ShuttingDown, deser_state);
        assert_eq!(1, deser_timers.len());
        assert_eq!(record, deser_timers[0]);
        assert_eq!(budget.max_per_instance, deser_budget.max_per_instance);
        assert_eq!(scan_result.timers.len(), deser_scan.timers.len());
    }

    // =============================================================================
    // Disk Format Stability Tests
    // =============================================================================

    mod disk_format_stability {
        use super::*;

        #[test]
        fn timer_record_minimal_serialization() {
            let record = TimerRecord::new(
                sample_instance_id(),
                ts_ms(1),
                None,
                ts_ms(1),
            );
            let json = serde_json::to_string(&record).expect("serialize minimal");
            let deser: TimerRecord = serde_json::from_str(&json).expect("deserialize minimal");
            assert_eq!(record.fire_at_ms.as_u64(), 1);
            assert_eq!(deser.fire_at_ms.as_u64(), 1);
        }

        #[test]
        fn timer_record_max_timestamp_serialization() {
            let max_ts = TimestampMs::try_from(u64::MAX).unwrap_or_else(|_| ts_ms(u64::MAX - 1000));
            let record = TimerRecord::new(
                sample_instance_id(),
                max_ts,
                None,
                max_ts - TimestampMs::try_from(1000).unwrap(),
            );
            let json = serde_json::to_string(&record).expect("serialize max ts");
            let deser: TimerRecord = serde_json::from_str(&json).expect("deserialize max ts");
            assert_eq!(deser.fire_at_ms.as_u64(), max_ts.as_u64());
        }

        #[test]
        fn fairness_budget_many_instances_roundtrip() {
            let mut budget = FairnessBudget::with_limits(100, 1000);
            let mut ids = Vec::new();
            for i in 0..50u8 {
                let id = InstanceId::parse(&format!("01H5JYV4XHGSR2F8KZ9BWNRFM{}", i)).unwrap();
                ids.push(id);
                for _ in 0..5 {
                    budget.record_resume(ids[i as usize].clone());
                }
            }

            let json = serde_json::to_string(&budget).expect("serialize many instances");
            let deser: FairnessBudget = serde_json::from_str(&json).expect("deserialize many");

            for id in &ids {
                assert_eq!(
                    *budget.instance_counts.get(id).unwrap(),
                    *deser.instance_counts.get(id).unwrap(),
                    "count mismatch for instance {}", id
                );
            }
        }

        #[test]
        fn config_edge_values_roundtrip() {
            let config = ReanimatorConfig {
                scan_interval: Duration::from_millis(1),
                max_timers_per_cycle: 1,
                max_concurrent_resumes: 1,
                shutdown_timeout: Duration::from_millis(1),
            };
            let json = serde_json::to_string(&config).expect("serialize minimal config");
            let deser: ReanimatorConfig = serde_json::from_str(&json).expect("deserialize minimal");
            assert_eq!(deser.scan_interval, Duration::from_millis(1));
            assert_eq!(deser.max_timers_per_cycle, 1);
        }

        #[test]
        fn state_machine_survives_hibernation() {
            let states = [
                ReanimatorState::Stopped,
                ReanimatorState::Running,
                ReanimatorState::ShuttingDown,
                ReanimatorState::ShutDown,
            ];

            for state in states {
                let json = serde_json::to_string(&state).expect("serialize");
                let deser: ReanimatorState = serde_json::from_str(&json).expect("deserialize");
                assert_eq!(state, deser, "state {} should survive hibernation", state);
            }
        }
    }

    // =============================================================================
    // Deserialization Robustness Tests
    // =============================================================================

    mod deserialization_robustness {
        use super::*;

        #[test]
        fn timer_record_missing_fields_deserialize_fails_gracefully() {
            let invalid_json = r#"{"instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA"}"#;
            let result: Result<TimerRecord, _> = serde_json::from_str(invalid_json);
            assert!(result.is_err(), "missing fields should fail to deserialize");
        }

        #[test]
        fn config_invalid_value_deserialize_fails() {
            let invalid_json = r#"{"scan_interval": -1, "max_timers_per_cycle": 100, "max_concurrent_resumes": 10, "shutdown_timeout": 30}"#;
            let result: Result<ReanimatorConfig, _> = serde_json::from_str(invalid_json);
            assert!(result.is_err(), "negative duration should fail to deserialize");
        }

        #[test]
        fn unknown_state_deserialize_fails() {
            let invalid_json = r#""UnknownState""#;
            let result: Result<ReanimatorState, _> = serde_json::from_str(invalid_json);
            assert!(result.is_err(), "unknown state should fail to deserialize");
        }
    }
}

// =============================================================================
// Filter Timers by Fairness - Integration with Serialization
// =============================================================================

mod filter_serialization_integration {
    use super::*;

    #[test]
    fn serialized_budget_respects_filtered_timers() {
        let instance_id = sample_instance_id();
        let mut budget = FairnessBudget::with_limits(1, 100);
        budget.record_resume(instance_id.clone());

        let timers = vec![
            TimerRecord::new(instance_id.clone(), ts_ms(1000), None, ts_ms(500)),
            TimerRecord::new(instance_id, ts_ms(2000), None, ts_ms(1500)),
        ];

        let (allowed, rejected) = filter_timers_by_fairness(timers.clone(), &budget);
        assert_eq!(allowed.len(), 0);
        assert_eq!(rejected.len(), 2);

        // Serialize the budget after filtering
        let json = serde_json::to_string(&budget).expect("serialize budget after filter");
        let deser: FairnessBudget = serde_json::from_str(&json).expect("deserialize");

        // Deserialize should preserve the exhausted budget
        let (allowed2, rejected2) = filter_timers_by_fairness(timers, &deser);
        assert_eq!(allowed2.len(), 0);
        assert_eq!(rejected2.len(), 2);
    }
}

// =============================================================================
// Backoff and Timing Edge Cases
// =============================================================================

mod timing_edge_cases {
    use super::*;

    #[test]
    fn timer_record_zero_duration_between_scheduled_and_fired() {
        let ts = ts_ms(1000);
        let record = TimerRecord::new(
            sample_instance_id(),
            ts,
            None,
            ts,
        );
        let json = serde_json::to_string(&record).expect("serialize zero duration");
        let deser: TimerRecord = serde_json::from_str(&json).expect("deserialize zero duration");
        assert_eq!(deser.fire_at_ms, deser.scheduled_at_ms);
    }

    #[test]
    fn timer_record_large_gap_between_scheduled_and_fired() {
        let record = TimerRecord::new(
            sample_instance_id(),
            ts_ms(u64::MAX / 2),
            None,
            ts_ms(0),
        );
        let json = serde_json::to_string(&record).expect("serialize large gap");
        let deser: TimerRecord = serde_json::from_str(&json).expect("deserialize large gap");
        assert_eq!(deser.fire_at_ms.as_u64(), u64::MAX / 2);
    }

    #[test]
    fn scan_result_preserves_skipped_count_across_roundtrip() {
        let result = TimerScanResult::new(
            vec![TimerRecord::new(
                sample_instance_id(),
                ts_ms(1000),
                None,
                ts_ms(500),
            )],
            ts_ms(2000),
            42,
        );
        let json = serde_json::to_string(&result).expect("serialize");
        let deser: TimerScanResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.skipped_count, 42);
    }
}
