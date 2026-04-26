use std::time::Duration;
use vo_executor::{Job, JobId, JobPriority, JobResult, Schedule};

// =========================================================================
// JobPriority Enum Tests
// =========================================================================

#[test]
fn job_priority_default_is_normal() {
    let priority = JobPriority::default();
    assert_eq!(priority, JobPriority::Normal);
}

#[test]
fn job_priority_all_variants_present() {
    let variants = [
        JobPriority::Critical,
        JobPriority::High,
        JobPriority::Normal,
        JobPriority::Low,
    ];
    assert_eq!(variants.len(), 4);
}

#[test]
fn job_priority_debug_format() {
    let priority = JobPriority::High;
    let debug = format!("{:?}", priority);
    assert!(debug.contains("High"));
}

#[test]
fn job_priority_extremes() {
    assert!(JobPriority::Critical < JobPriority::Low);
}

// =========================================================================
// Schedule Enum Tests
// =========================================================================

#[test]
fn schedule_cron_creation() {
    let schedule = Schedule::cron("*/5 * * * *");
    match schedule {
        Schedule::Cron(expr) => assert_eq!(expr, "*/5 * * * *"),
        _ => panic!("Expected Cron schedule"),
    }
}

#[test]
fn schedule_cron_next_fire_returns_none() {
    let schedule = Schedule::cron("*/5 * * * *");
    let next = schedule.next_fire_time(0);
    assert!(
        next.is_none(),
        "Cron next_fire_time should return None (not implemented)"
    );
}

#[test]
fn schedule_one_shot_creation() {
    let delay = Duration::from_secs(60);
    let schedule = Schedule::one_shot(delay);
    match schedule {
        Schedule::OneShot { fire_at_ms } => {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            assert!(fire_at_ms > now_ms);
        }
        _ => panic!("Expected OneShot schedule"),
    }
}

#[test]
fn schedule_one_shot_next_fire_first_call() {
    let schedule = Schedule::one_shot(Duration::from_secs(60));
    let next = schedule.next_fire_time(0);
    assert!(
        next.is_some(),
        "First call with last_fire_ms=0 should return Some"
    );
}

#[test]
fn schedule_one_shot_next_fire_second_call() {
    let schedule = Schedule::one_shot(Duration::from_secs(60));
    let first = schedule.next_fire_time(0).unwrap();
    let second = schedule.next_fire_time(first);
    assert!(
        second.is_none(),
        "Second call with last_fire_ms!=0 should return None"
    );
}

#[test]
fn schedule_interval_creation() {
    let interval = Duration::from_secs(30);
    let schedule = Schedule::interval(interval);
    match schedule {
        Schedule::Interval { interval_ms } => assert_eq!(interval_ms, 30_000),
        _ => panic!("Expected Interval schedule"),
    }
}

#[test]
fn schedule_interval_next_fire_first() {
    let schedule = Schedule::interval(Duration::from_secs(30));
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let next = schedule.next_fire_time(0);
    assert!(next.is_some());
    assert!(next.unwrap() > now_ms);
}

#[test]
fn schedule_interval_next_fire_subsequent() {
    let schedule = Schedule::interval(Duration::from_secs(30));
    let first = schedule.next_fire_time(0).unwrap();
    let second = schedule.next_fire_time(first).unwrap();
    assert_eq!(second - first, 30_000);
}

#[test]
fn schedule_interval_no_overflow() {
    let schedule = Schedule::interval(Duration::from_secs(1));
    let max_u64 = u64::MAX;
    let next = schedule.next_fire_time(max_u64);
    assert!(
        next.is_some(),
        "saturating_add should prevent overflow at u64::MAX"
    );
}

#[test]
fn schedule_one_shot_zero_delay() {
    let schedule = Schedule::one_shot(Duration::ZERO);
    if let Schedule::OneShot { fire_at_ms } = schedule {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        assert!(fire_at_ms >= now_ms);
    } else {
        panic!("Expected OneShot");
    }
}

#[test]
fn schedule_interval_zero_interval() {
    let schedule = Schedule::interval(Duration::ZERO);
    if let Schedule::Interval { interval_ms } = schedule {
        assert_eq!(interval_ms, 0);
    } else {
        panic!("Expected Interval");
    }
}

// =========================================================================
// Job Type Tests
// =========================================================================

#[test]
fn job_new_sets_all_fields() {
    let job = Job::new(
        JobId::new(1),
        "test payload".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert_eq!(job.id, JobId::new(1));
    assert_eq!(job.payload, "test payload");
}

#[test]
fn job_default_priority_is_normal() {
    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert_eq!(job.priority, JobPriority::Normal);
}

#[test]
fn job_default_retries_is_3() {
    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert_eq!(job.max_retries, 3);
}

#[test]
fn job_default_backoff_is_1000ms() {
    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert_eq!(job.backoff_ms, 1000);
}

#[test]
fn job_with_priority() {
    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    )
    .with_priority(JobPriority::Critical);
    assert_eq!(job.priority, JobPriority::Critical);
}

#[test]
fn job_with_retries() {
    let job = Job::new(
        JobId::new(1),
        "test".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    )
    .with_retries(5, 500);
    assert_eq!(job.max_retries, 5);
    assert_eq!(job.backoff_ms, 500);
}

#[test]
fn job_payload_is_string() {
    let job = Job::new(
        JobId::new(1),
        "test payload".to_string(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert!(matches!(job.payload, String));
}

#[test]
fn job_empty_payload() {
    let job = Job::new(
        JobId::new(1),
        String::new(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert_eq!(job.payload, "");
}

#[test]
fn job_large_payload() {
    let large_payload = "x".repeat(1_000_000);
    let job = Job::new(
        JobId::new(1),
        large_payload.clone(),
        Schedule::one_shot(Duration::from_secs(10)),
    );
    assert_eq!(job.payload, large_payload);
}

// =========================================================================
// JobId Type Tests
// =========================================================================

#[test]
fn job_id_new_constructs() {
    let job_id = JobId::new(42);
    assert_eq!(job_id.0, 42);
}

#[test]
fn job_id_equality() {
    let id1 = JobId::new(100);
    let id2 = JobId::new(100);
    let id3 = JobId::new(200);
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn job_id_hash() {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    let id = JobId::new(1);
    map.insert(id, "test");
    assert_eq!(map.get(&JobId::new(1)), Some(&"test"));
}

#[test]
fn job_id_display() {
    let job_id = JobId::new(42);
    let display = format!("{}", job_id);
    assert_eq!(display, "job-42");
}

#[test]
fn job_id_debug() {
    let job_id = JobId::new(42);
    let debug = format!("{:?}", job_id);
    assert!(debug.contains("42"));
}

// =========================================================================
// JobResult Type Tests
// =========================================================================

#[test]
fn job_result_has_all_fields() {
    let result = JobResult {
        job_id: JobId::new(1),
        success: true,
        output: Some("output".to_string()),
        error: None,
        attempt: 1,
    };
    assert_eq!(result.job_id, JobId::new(1));
    assert!(result.success);
    assert_eq!(result.attempt, 1);
}

#[test]
fn job_result_success_true() {
    let result = JobResult {
        job_id: JobId::new(1),
        success: true,
        output: Some("done".to_string()),
        error: None,
        attempt: 1,
    };
    assert!(result.success);
    assert!(result.error.is_none());
}

#[test]
fn job_result_failure_false() {
    let result = JobResult {
        job_id: JobId::new(1),
        success: false,
        output: None,
        error: Some("failed".to_string()),
        attempt: 3,
    };
    assert!(!result.success);
    assert!(result.error.is_some());
}
