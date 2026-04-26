use std::time::Duration;

use chrono::Utc;
use crate::types::*;

// === JobId tests ===

#[test]
fn job_id_generate_returns_unique_ids() {
    let id1 = JobId::generate();
    let id2 = JobId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn job_id_generate_returns_non_zero_ulid() {
    let id = JobId::generate();
    assert_ne!(id.0.to_string(), "00000000000000000000000000");
}

#[test]
fn job_id_display_returns_ulid_string() {
    let id = JobId::generate();
    let display = id.to_string();
    assert!(!display.is_empty());
    assert!(display.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
}

#[test]
fn job_id_serialize_deserialize_roundtrip() {
    let id = JobId::generate();
    let json = serde_json::to_string(&id).unwrap();
    let recovered: JobId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, recovered);
}

#[test]
fn job_id_copy_trait() {
    let id = JobId::generate();
    let id2 = id;
    let id3 = id2;
    assert_eq!(id, id3);
}

#[test]
fn job_id_hash_trait_works() {
    use std::collections::HashSet;
    let id = JobId::generate();
    let mut set = HashSet::new();
    set.insert(id);
    assert!(set.contains(&id));
}

// === JobState tests ===

#[test]
fn job_state_is_terminal_completed() {
    assert!(JobState::Completed.is_terminal());
}

#[test]
fn job_state_is_terminal_failed() {
    assert!(JobState::Failed.is_terminal());
}

#[test]
fn job_state_is_terminal_cancelled() {
    assert!(JobState::Cancelled.is_terminal());
}

#[test]
fn job_state_is_not_terminal_scheduled() {
    assert!(!JobState::Scheduled.is_terminal());
}

#[test]
fn job_state_is_not_terminal_pending() {
    assert!(!JobState::Pending.is_terminal());
}

#[test]
fn job_state_is_not_terminal_running() {
    assert!(!JobState::Running.is_terminal());
}

#[test]
fn job_state_is_not_terminal_retrying() {
    assert!(!JobState::Retrying.is_terminal());
}

#[test]
fn job_state_is_non_terminal_for_non_terminal_states() {
    assert!(JobState::Scheduled.is_non_terminal());
    assert!(JobState::Pending.is_non_terminal());
    assert!(JobState::Running.is_non_terminal());
    assert!(JobState::Retrying.is_non_terminal());
}

#[test]
fn job_state_is_non_terminal_false_for_terminal_states() {
    assert!(!JobState::Completed.is_non_terminal());
    assert!(!JobState::Failed.is_non_terminal());
    assert!(!JobState::Cancelled.is_non_terminal());
}

#[test]
fn job_state_display_scheduled() {
    assert_eq!(JobState::Scheduled.to_string(), "scheduled");
}

#[test]
fn job_state_display_pending() {
    assert_eq!(JobState::Pending.to_string(), "pending");
}

#[test]
fn job_state_display_running() {
    assert_eq!(JobState::Running.to_string(), "running");
}

#[test]
fn job_state_display_completed() {
    assert_eq!(JobState::Completed.to_string(), "completed");
}

#[test]
fn job_state_display_failed() {
    assert_eq!(JobState::Failed.to_string(), "failed");
}

#[test]
fn job_state_display_cancelled() {
    assert_eq!(JobState::Cancelled.to_string(), "cancelled");
}

#[test]
fn job_state_display_retrying() {
    assert_eq!(JobState::Retrying.to_string(), "retrying");
}

#[test]
fn job_state_all_variants_have_display() {
    for state in [
        JobState::Scheduled,
        JobState::Pending,
        JobState::Running,
        JobState::Completed,
        JobState::Failed,
        JobState::Cancelled,
        JobState::Retrying,
    ] {
        let s = state.to_string();
        assert!(!s.is_empty(), "Display for {:?} should not be empty", state);
    }
}

#[test]
fn job_state_serialize_deserialize_scheduled() {
    let json = serde_json::to_string(&JobState::Scheduled).unwrap();
    assert_eq!(json, "\"scheduled\"");
    let recovered: JobState = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, JobState::Scheduled);
}

#[test]
fn job_state_serialize_deserialize_all_variants() {
    for state in [
        JobState::Scheduled,
        JobState::Pending,
        JobState::Running,
        JobState::Completed,
        JobState::Failed,
        JobState::Cancelled,
        JobState::Retrying,
    ] {
        let json = serde_json::to_string(&state).unwrap();
        let recovered: JobState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, recovered);
    }
}

#[test]
fn job_state_equality() {
    assert_eq!(JobState::Pending, JobState::Pending);
    assert_ne!(JobState::Pending, JobState::Running);
    assert_ne!(JobState::Completed, JobState::Failed);
}

#[test]
fn job_state_copy_trait() {
    let state = JobState::Running;
    let state2 = state;
    let state3 = state2;
    assert_eq!(state, state3);
}

// === JobKind tests ===

#[test]
fn job_kind_display_one_shot() {
    assert_eq!(JobKind::OneShot.to_string(), "one_shot");
}

#[test]
fn job_kind_display_recurring() {
    assert_eq!(JobKind::Recurring.to_string(), "recurring");
}

#[test]
fn job_kind_display_delayed() {
    assert_eq!(JobKind::Delayed.to_string(), "delayed");
}

#[test]
fn job_kind_serialize_deserialize_one_shot() {
    let json = serde_json::to_string(&JobKind::OneShot).unwrap();
    assert_eq!(json, "\"one_shot\"");
    let recovered: JobKind = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, JobKind::OneShot);
}

#[test]
fn job_kind_serialize_deserialize_all_variants() {
    for kind in [JobKind::OneShot, JobKind::Recurring, JobKind::Delayed] {
        let json = serde_json::to_string(&kind).unwrap();
        let recovered: JobKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, recovered);
    }
}

#[test]
fn job_kind_equality() {
    assert_eq!(JobKind::OneShot, JobKind::OneShot);
    assert_ne!(JobKind::OneShot, JobKind::Recurring);
    assert_ne!(JobKind::Recurring, JobKind::Delayed);
}

#[test]
fn job_kind_copy_trait() {
    let kind = JobKind::Recurring;
    let kind2 = kind;
    assert_eq!(kind, kind2);
}

// === SchedulePolicy tests ===

#[test]
fn schedule_policy_display_at() {
    let dt = Utc::now();
    let policy = SchedulePolicy::At(dt);
    let display = format!("{}", policy);
    assert!(display.starts_with("at("));
    assert!(display.ends_with(')'));
}

#[test]
fn schedule_policy_display_after() {
    let policy = SchedulePolicy::After(Duration::from_secs(60));
    let display = format!("{}", policy);
    assert!(display.starts_with("after("));
}

#[test]
fn schedule_policy_display_cron() {
    let policy = SchedulePolicy::Cron("*/5 * * * *".to_string());
    assert_eq!(policy.to_string(), "cron(*/5 * * * *)");
}

#[test]
fn schedule_policy_display_immediate() {
    assert_eq!(SchedulePolicy::Immediate.to_string(), "immediate");
}

#[test]
fn schedule_policy_validate_cron_valid_all_wildcards() {
    assert!(SchedulePolicy::validate_cron("* * * * *").is_ok());
}

#[test]
fn schedule_policy_validate_cron_valid_specific_values() {
    assert!(SchedulePolicy::validate_cron("30 14 1 6 *").is_ok());
}

#[test]
fn schedule_policy_validate_cron_valid_step() {
    assert!(SchedulePolicy::validate_cron("*/15 * * * *").is_ok());
}

#[test]
fn schedule_policy_validate_cron_valid_range() {
    assert!(SchedulePolicy::validate_cron("0 9-17 * * 1-5").is_ok());
}

#[test]
fn schedule_policy_validate_cron_valid_mixed() {
    assert!(SchedulePolicy::validate_cron("*/5 0 1-15 * 1").is_ok());
}

#[test]
fn schedule_policy_validate_cron_invalid_too_few_fields() {
    assert!(SchedulePolicy::validate_cron("* * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_too_many_fields() {
    assert!(SchedulePolicy::validate_cron("* * * * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_minute_out_of_range() {
    assert!(SchedulePolicy::validate_cron("60 * * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_hour_out_of_range() {
    assert!(SchedulePolicy::validate_cron("* 24 * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_day_out_of_range() {
    assert!(SchedulePolicy::validate_cron("* * 32 * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_month_out_of_range() {
    assert!(SchedulePolicy::validate_cron("* * * 13 *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_dow_out_of_range() {
    assert!(SchedulePolicy::validate_cron("* * * * 7").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_step_zero() {
    assert!(SchedulePolicy::validate_cron("*/0 * * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_step_exceeds_max() {
    assert!(SchedulePolicy::validate_cron("*/60 * * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_range_start_after_end() {
    assert!(SchedulePolicy::validate_cron("* * 15-1 * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_range_start_below_min() {
    assert!(SchedulePolicy::validate_cron("* * 0-15 * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_range_end_above_max() {
    assert!(SchedulePolicy::validate_cron("* * * 0-6 *").is_err());
}

#[test]
fn schedule_policy_validate_cron_invalid_non_numeric() {
    assert!(SchedulePolicy::validate_cron("abc * * * *").is_err());
}

#[test]
fn schedule_policy_validate_cron_valid_edge_values() {
    assert!(SchedulePolicy::validate_cron("0 0 1 1 0").is_ok());
    assert!(SchedulePolicy::validate_cron("59 23 31 12 6").is_ok());
}

#[test]
fn schedule_policy_equality_at() {
    let dt = Utc::now();
    let p1 = SchedulePolicy::At(dt);
    let p2 = SchedulePolicy::At(dt);
    assert_eq!(p1, p2);
}

#[test]
fn schedule_policy_equality_after() {
    let p1 = SchedulePolicy::After(Duration::from_secs(60));
    let p2 = SchedulePolicy::After(Duration::from_secs(60));
    assert_eq!(p1, p2);
}

#[test]
fn schedule_policy_equality_cron() {
    let p1 = SchedulePolicy::Cron("*/5 * * * *".to_string());
    let p2 = SchedulePolicy::Cron("*/5 * * * *".to_string());
    assert_eq!(p1, p2);
}

#[test]
fn schedule_policy_equality_immediate() {
    let p1 = SchedulePolicy::Immediate;
    let p2 = SchedulePolicy::Immediate;
    assert_eq!(p1, p2);
}

#[test]
fn schedule_policy_inequality_different_variants() {
    let p1 = SchedulePolicy::Immediate;
    let p2 = SchedulePolicy::At(Utc::now());
    assert_ne!(p1, p2);
}

// === RetryPolicy tests ===

#[test]
fn retry_policy_try_new_valid_default_values() {
    let policy = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300));
    assert!(policy.is_ok());
}

#[test]
fn retry_policy_try_new_valid_minimal() {
    let policy = RetryPolicy::try_new(1, 1.0, Duration::from_millis(1), Duration::from_secs(1));
    assert!(policy.is_ok());
}

#[test]
fn retry_policy_try_new_valid_high_attempts() {
    let policy =
        RetryPolicy::try_new(100, 1.5, Duration::from_secs(1), Duration::from_secs(3600));
    assert!(policy.is_ok());
}

#[test]
fn retry_policy_try_new_zero_max_attempts() {
    let result = RetryPolicy::try_new(0, 2.0, Duration::from_secs(1), Duration::from_secs(300));
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        RetryPolicyError::MaxAttemptsZero
    ));
}

#[test]
fn retry_policy_try_new_backoff_below_one() {
    let result = RetryPolicy::try_new(3, 0.5, Duration::from_secs(1), Duration::from_secs(300));
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        RetryPolicyError::BackoffMultiplierBelowOne { value } if value == 0.5
    ));
}

#[test]
fn retry_policy_try_new_backoff_exactly_one() {
    let result = RetryPolicy::try_new(3, 1.0, Duration::from_secs(1), Duration::from_secs(300));
    assert!(result.is_ok());
}

#[test]
fn retry_policy_try_new_zero_initial_delay() {
    let result = RetryPolicy::try_new(3, 2.0, Duration::from_secs(0), Duration::from_secs(300));
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        RetryPolicyError::InitialDelayZero
    ));
}

#[test]
fn retry_policy_try_new_max_delay_below_initial() {
    let result =
        RetryPolicy::try_new(3, 2.0, Duration::from_secs(10), Duration::from_secs(5));
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        RetryPolicyError::MaxDelayBelowInitial
    ));
}

#[test]
fn retry_policy_try_new_max_delay_equal_to_initial() {
    let result =
        RetryPolicy::try_new(3, 2.0, Duration::from_secs(10), Duration::from_secs(10));
    assert!(result.is_ok());
}

#[test]
fn retry_policy_default_values() {
    let policy = RetryPolicy::default_policy();
    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.backoff_multiplier, 2.0);
    assert_eq!(policy.initial_delay, Duration::from_secs(1));
    assert_eq!(policy.max_delay, Duration::from_secs(300));
}

#[test]
fn retry_policy_default_trait() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_attempts, 3);
}

#[test]
fn retry_policy_compute_backoff_attempt_zero() {
    let policy = RetryPolicy::default_policy();
    let backoff = policy.compute_backoff(0);
    assert_eq!(backoff, Duration::from_secs(1));
}

#[test]
fn retry_policy_compute_backoff_attempt_one() {
    let policy = RetryPolicy::default_policy();
    let backoff = policy.compute_backoff(1);
    assert_eq!(backoff, Duration::from_secs(2));
}

#[test]
fn retry_policy_compute_backoff_attempt_two() {
    let policy = RetryPolicy::default_policy();
    let backoff = policy.compute_backoff(2);
    assert_eq!(backoff, Duration::from_secs(4));
}

#[test]
fn retry_policy_compute_backoff_capped_at_max() {
    let policy = RetryPolicy::try_new(10, 2.0, Duration::from_secs(1), Duration::from_secs(10))
        .unwrap();
    // 2^10 = 1024 > 10, so should be capped
    let backoff = policy.compute_backoff(10);
    assert_eq!(backoff, Duration::from_secs(10));
}

#[test]
fn retry_policy_compute_backoff_exponential_growth() {
    let policy =
        RetryPolicy::try_new(10, 3.0, Duration::from_millis(100), Duration::from_secs(600))
            .unwrap();
    let b0 = policy.compute_backoff(0);
    let b1 = policy.compute_backoff(1);
    let b2 = policy.compute_backoff(2);
    assert_eq!(b0, Duration::from_millis(100));
    assert_eq!(b1, Duration::from_millis(300));
    assert_eq!(b2, Duration::from_millis(900));
}

#[test]
fn retry_policy_compute_backoff_backoff_one_no_growth() {
    let policy =
        RetryPolicy::try_new(5, 1.0, Duration::from_secs(5), Duration::from_secs(60)).unwrap();
    let b0 = policy.compute_backoff(0);
    let b1 = policy.compute_backoff(1);
    let b2 = policy.compute_backoff(2);
    assert_eq!(b0, Duration::from_secs(5));
    assert_eq!(b1, Duration::from_secs(5));
    assert_eq!(b2, Duration::from_secs(5));
}

#[test]
fn retry_policy_can_retry_within_limit() {
    let policy = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300))
        .unwrap();
    assert!(policy.can_retry(0));
    assert!(policy.can_retry(1));
    assert!(policy.can_retry(2));
}

#[test]
fn retry_policy_can_retry_at_max_rejected() {
    let policy = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300))
        .unwrap();
    assert!(!policy.can_retry(3));
}

#[test]
fn retry_policy_can_retry_above_max_rejected() {
    let policy = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300))
        .unwrap();
    assert!(!policy.can_retry(10));
}

#[test]
fn retry_policy_can_retry_single_attempt() {
    let policy = RetryPolicy::try_new(1, 2.0, Duration::from_secs(1), Duration::from_secs(300))
        .unwrap();
    assert!(policy.can_retry(0));
    assert!(!policy.can_retry(1));
}

#[test]
fn retry_policy_clone_trait() {
    let policy = RetryPolicy::default_policy();
    let policy2 = policy.clone();
    assert_eq!(policy, policy2);
}

#[test]
fn retry_policy_eq_trait() {
    let p1 = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300)).unwrap();
    let p2 = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300)).unwrap();
    assert_eq!(p1, p2);
}

#[test]
fn retry_policy_neq_trait_different_attempts() {
    let p1 = RetryPolicy::try_new(3, 2.0, Duration::from_secs(1), Duration::from_secs(300)).unwrap();
    let p2 = RetryPolicy::try_new(5, 2.0, Duration::from_secs(1), Duration::from_secs(300)).unwrap();
    assert_ne!(p1, p2);
}

#[test]
fn retry_policy_display_max_attempts_zero() {
    let err = RetryPolicyError::MaxAttemptsZero;
    assert_eq!(err.to_string(), "max_attempts must be > 0");
}

#[test]
fn retry_policy_display_backoff_below_one() {
    let err = RetryPolicyError::BackoffMultiplierBelowOne { value: 0.5 };
    assert_eq!(
        err.to_string(),
        "backoff_multiplier 0.5 must be >= 1.0"
    );
}

#[test]
fn retry_policy_display_initial_delay_zero() {
    let err = RetryPolicyError::InitialDelayZero;
    assert_eq!(err.to_string(), "initial_delay must be > 0");
}

#[test]
fn retry_policy_display_max_delay_below_initial() {
    let err = RetryPolicyError::MaxDelayBelowInitial;
    assert_eq!(
        err.to_string(),
        "max_delay must be >= initial_delay"
    );
}

#[test]
fn retry_policy_is_error_trait() {
    let err = RetryPolicyError::MaxAttemptsZero;
    let _: &dyn std::error::Error = &err;
}

// === RetryPolicy serialization ===

#[test]
fn retry_policy_serialize_deserialize_roundtrip() {
    let policy = RetryPolicy::try_new(5, 2.5, Duration::from_secs(2), Duration::from_secs(600))
        .unwrap();
    let json = serde_json::to_string(&policy).unwrap();
    let recovered: RetryPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(policy, recovered);
}

#[test]
fn retry_policy_serialize_default_roundtrip() {
    let policy = RetryPolicy::default_policy();
    let json = serde_json::to_string(&policy).unwrap();
    let recovered: RetryPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(policy, recovered);
}

// === JobPriority tests ===

#[test]
fn job_priority_display_critical() {
    assert_eq!(JobPriority::Critical.to_string(), "critical");
}

#[test]
fn job_priority_display_high() {
    assert_eq!(JobPriority::High.to_string(), "high");
}

#[test]
fn job_priority_display_normal() {
    assert_eq!(JobPriority::Normal.to_string(), "normal");
}

#[test]
fn job_priority_display_low() {
    assert_eq!(JobPriority::Low.to_string(), "low");
}

#[test]
fn job_priority_display_background() {
    assert_eq!(JobPriority::Background.to_string(), "background");
}

#[test]
fn job_priority_serialize_deserialize_critical() {
    let json = serde_json::to_string(&JobPriority::Critical).unwrap();
    assert_eq!(json, "\"critical\"");
    let recovered: JobPriority = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered, JobPriority::Critical);
}

#[test]
fn job_priority_serialize_deserialize_all_variants() {
    for priority in [
        JobPriority::Critical,
        JobPriority::High,
        JobPriority::Normal,
        JobPriority::Low,
        JobPriority::Background,
    ] {
        let json = serde_json::to_string(&priority).unwrap();
        let recovered: JobPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(priority, recovered);
    }
}

#[test]
fn job_priority_ord_critical_highest() {
    assert!(JobPriority::Critical < JobPriority::High);
    assert!(JobPriority::Critical < JobPriority::Normal);
    assert!(JobPriority::Critical < JobPriority::Low);
    assert!(JobPriority::Critical < JobPriority::Background);
}

#[test]
fn job_priority_ord_background_lowest() {
    assert!(JobPriority::Background > JobPriority::Critical);
    assert!(JobPriority::Background > JobPriority::High);
    assert!(JobPriority::Background > JobPriority::Normal);
    assert!(JobPriority::Background > JobPriority::Low);
}

#[test]
fn job_priority_ord_total_ordering() {
    let mut priorities = vec![
        JobPriority::Normal,
        JobPriority::Background,
        JobPriority::Critical,
        JobPriority::Low,
        JobPriority::High,
    ];
    priorities.sort();
    assert_eq!(
        priorities,
        vec![
            JobPriority::Critical,
            JobPriority::High,
            JobPriority::Normal,
            JobPriority::Low,
            JobPriority::Background,
        ]
    );
}

#[test]
fn job_priority_repr_values() {
    assert_eq!(JobPriority::Critical as u8, 0);
    assert_eq!(JobPriority::High as u8, 1);
    assert_eq!(JobPriority::Normal as u8, 2);
    assert_eq!(JobPriority::Low as u8, 3);
    assert_eq!(JobPriority::Background as u8, 4);
}

#[test]
fn job_priority_eq_trait() {
    assert_eq!(JobPriority::Normal, JobPriority::Normal);
    assert_ne!(JobPriority::High, JobPriority::Low);
}

#[test]
fn job_priority_copy_trait() {
    let p = JobPriority::Critical;
    let p2 = p;
    assert_eq!(p, p2);
}

// === JobPriority serialization round-trip in context ===

#[test]
fn job_priority_in_struct_roundtrip() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestEntry {
        priority: JobPriority,
    }

    let entry = TestEntry {
        priority: JobPriority::High,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let recovered: TestEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry, recovered);
    assert_eq!(json, r#"{"priority":"high"}"#);
}

// === RetryPolicy backoff edge cases ===

#[test]
fn retry_policy_compute_backoff_large_attempt_capped() {
    let policy =
        RetryPolicy::try_new(100, 10.0, Duration::from_secs(1), Duration::from_secs(5)).unwrap();
    let backoff = policy.compute_backoff(100);
    assert_eq!(backoff, Duration::from_secs(5));
}

#[test]
fn retry_policy_compute_backoff_zero_attempt_zero_delay() {
    let policy =
        RetryPolicy::try_new(3, 2.0, Duration::from_millis(0), Duration::from_secs(300)).unwrap();
    let backoff = policy.compute_backoff(0);
    assert_eq!(backoff, Duration::from_millis(0));
}

// === SchedulePolicy serialization ===

#[test]
fn schedule_policy_serialize_immediate() {
    let json = serde_json::to_string(&SchedulePolicy::Immediate).unwrap();
    assert_eq!(json, r#"{"type":"immediate"}"#);
}

#[test]
fn schedule_policy_serialize_cron() {
    let json = serde_json::to_string(&SchedulePolicy::Cron("*/5 * * * *".to_string())).unwrap();
    assert!(json.contains(r#""type":"cron""#));
    assert!(json.contains("*/5 * * * *"));
}

#[test]
fn schedule_policy_roundtrip_cron() {
    let policy = SchedulePolicy::Cron("0 12 * * 1".to_string());
    let json = serde_json::to_string(&policy).unwrap();
    let recovered: SchedulePolicy = serde_json::from_str(&json).unwrap();
    match recovered {
        SchedulePolicy::Cron(expr) => assert_eq!(expr, "0 12 * * 1"),
        _ => panic!("Expected Cron variant"),
    }
}

// === RetryPolicy display tests ===

#[test]
fn retry_policy_error_debug_format() {
    let err = RetryPolicyError::MaxAttemptsZero;
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty());
}

// === JobState Display edge cases ===

#[test]
fn job_state_display_all_lowercase() {
    for state in [
        JobState::Scheduled,
        JobState::Pending,
        JobState::Running,
        JobState::Completed,
        JobState::Failed,
        JobState::Cancelled,
        JobState::Retrying,
    ] {
        let s = state.to_string();
        assert!(s == s.to_lowercase(), "{:?} Display should be lowercase", state);
    }
}
