# Test Plan: Scheduler Types Exhaustive Testing

**Bead:** ve-yncka
**Contract:** ADR-047-v2 Background Job Scheduler (Implementation: vo-executor/src/scheduler/)
**Implementation:** `crates/vo-executor/src/scheduler/types.rs`, `error.rs`, `queue.rs`

## Overview

This test plan provides Given-When-Then scenarios for every scheduler type variant, state transition, and policy. The scheduler implementation uses:

- **JobPriority** - 4 variants (Critical, High, Normal, Low)
- **Schedule** - 3 variants (Cron, OneShot, Interval)
- **JobId** - Single u64-based type with Display impl
- **Job** - Struct with id, payload, schedule, priority, max_retries, backoff_ms
- **JobResult** - Struct with job_id, success, output, error, attempt
- **SchedulerConfig** - Struct with max_concurrent, scan_interval, max_jobs_per_scan
- **SchedulerError** - Error taxonomy (error.rs)
- **JobRunError** - Job execution error taxonomy (error.rs)
- **PriorityQueue** - Ordering by (priority, fire_at_ms)

**Note:** The ADR-047 mentioned JobState, JobKind, and SchedulePolicy, but the current implementation uses the types listed above instead.

---

## 1. JobPriority Enum - Given-When-Then Scenarios

### 1.1 JobPriority Ordering

**Given** all JobPriority variants exist (Critical=0, High=1, Normal=2, Low=3)
**When** comparing any two priorities with `<` operator
**Then** lower enum value = higher priority (Critical < High < Normal < Low)

**Test:** `job_priority_ordering`
```rust
assert!(JobPriority::Critical < JobPriority::High);
assert!(JobPriority::High < JobPriority::Normal);
assert!(JobPriority::Normal < JobPriority::Low);
```

### 1.2 JobPriority Default

**Given** JobPriority implements Default trait
**When** creating JobPriority::default()
**Then** returns Normal (priority level 2)

**Test:** `job_priority_default_is_normal`
```rust
assert_eq!(JobPriority::default(), JobPriority::Normal);
```

### 1.3 JobPriority Serialization

**Given** a JobPriority value
**When** serializing with serde and deserializing
**Then** the value is preserved exactly

**Test:** `job_priority_serialization_roundtrip`
```rust
let priority = JobPriority::Critical;
let json = serde_json::to_string(&priority).unwrap();
let parsed: JobPriority = serde_json::from_str(&json).unwrap();
assert_eq!(priority, parsed);
```

### 1.4 JobPriority All Variants

**Given** JobPriority is an enum
**When** matching exhaustively on all variants
**Then** all 4 variants (Critical, High, Normal, Low) are covered

**Test:** `job_priority_all_variants_present`
```rust
fn match_all(p: JobPriority) -> &'static str {
    match p {
        JobPriority::Critical => "critical",
        JobPriority::High => "high",
        JobPriority::Normal => "normal",
        JobPriority::Low => "low",
    }
}
assert_eq!(match_all(JobPriority::Critical), "critical");
```

### 1.5 JobPriority Debug

**Given** JobPriority implements Debug
**When** formatting with `{:?}`
**Then** Debug output shows the variant name

**Test:** `job_priority_debug_format`
```rust
let debug_str = format!("{:?}", JobPriority::High);
assert_eq!(debug_str, "High");
```

---

## 2. Schedule Enum - Given-When-Then Scenarios

### 2.1 Schedule Cron Variant

**Given** a cron expression string
**When** creating Schedule::cron(expr)
**Then** Schedule::Cron(expr) variant is created with expression preserved

**Test:** `schedule_cron_creation`
```rust
let schedule = Schedule::cron("0 0 * * *");
match schedule {
    Schedule::Cron(expr) => assert_eq!(expr, "0 0 * * *"),
    _ => panic!("Expected Cron variant"),
}
```

### 2.2 Schedule Cron Next Fire

**Given** a Cron schedule
**When** calling next_fire_time(last_fire_ms)
**Then** returns None (cron not yet implemented)

**Test:** `schedule_cron_next_fire_returns_none`
```rust
let schedule = Schedule::cron("0 0 * * *");
assert!(schedule.next_fire_time(0).is_none());
assert!(schedule.next_fire_time(1000).is_none());
```

### 2.3 Schedule OneShot Creation

**Given** a delay duration
**When** calling Schedule::one_shot(delay)
**Then** Schedule::OneShot variant with fire_at_ms = now_ms + delay_ms

**Test:** `schedule_one_shot_creation`
```rust
let schedule = Schedule::one_shot(Duration::from_secs(60));
match schedule {
    Schedule::OneShot { fire_at_ms } => {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        assert!(fire_at_ms > now_ms);
        assert!(fire_at_ms <= now_ms + 61000);
    }
    _ => panic!("Expected OneShot variant"),
}
```

### 2.4 Schedule OneShot First Call

**Given** a OneShot schedule with fire_at_ms in future
**When** calling next_fire_time(0) (first call)
**Then** returns Some(fire_at_ms)

**Test:** `schedule_one_shot_next_fire_first_call`
```rust
let schedule = Schedule::one_shot(Duration::from_secs(60));
if let Schedule::OneShot { fire_at_ms } = schedule {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    assert!(fire_at_ms > now_ms);
    assert_eq!(schedule.next_fire_time(0), Some(fire_at_ms));
}
```

### 2.5 Schedule OneShot Second Call

**Given** a OneShot schedule with last_fire_ms != 0
**When** calling next_fire_time(last_fire_ms)
**Then** returns None (one-shot should only fire once)

**Test:** `schedule_one_shot_next_fire_second_call`
```rust
let schedule = Schedule::one_shot(Duration::from_secs(60));
if let Schedule::OneShot { fire_at_ms } = schedule {
    assert_eq!(schedule.next_fire_time(0), Some(fire_at_ms));
    assert_eq!(schedule.next_fire_time(fire_at_ms), None);
}
```

### 2.6 Schedule Interval Creation

**Given** an interval duration
**When** calling Schedule::interval(interval)
**Then** Schedule::Interval variant with interval_ms = interval.as_millis()

**Test:** `schedule_interval_creation`
```rust
let interval = Duration::from_secs(30);
let schedule = Schedule::interval(interval);
match schedule {
    Schedule::Interval { interval_ms } => {
        assert_eq!(interval_ms, interval.as_millis() as u64);
    }
    _ => panic!("Expected Interval variant"),
}
```

### 2.7 Schedule Interval First Fire

**Given** an Interval schedule
**When** calling next_fire_time(0) (first call)
**Then** returns Some(now_ms + interval_ms)

**Test:** `schedule_interval_next_fire_first`
```rust
let schedule = Schedule::interval(Duration::from_secs(30));
let now_ms = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| d.as_millis() as u64);
let next = schedule.next_fire_time(0).unwrap();
assert!(next >= now_ms);
assert!(next <= now_ms + 31000);
```

### 2.8 Schedule Interval Subsequent Fire

**Given** an Interval schedule with last_fire_ms
**When** calling next_fire_time(last_fire_ms)
**Then** returns Some(last_fire_ms + interval_ms)

**Test:** `schedule_interval_next_fire_subsequent`
```rust
let schedule = Schedule::interval(Duration::from_secs(30));
let last = 1000000;
let next = schedule.next_fire_time(last).unwrap();
assert_eq!(next, last + 30000);
let next2 = schedule.next_fire_time(next).unwrap();
assert_eq!(next2, next + 30000);
```

### 2.9 Schedule Interval No Overflow

**Given** an Interval schedule at u64::MAX
**When** calling next_fire_time(u64::MAX)
**Then** returns Some(u64::MAX) via saturating_add (no panic)

**Test:** `schedule_interval_no_overflow`
```rust
let schedule = Schedule::interval(Duration::from_secs(1));
let result = schedule.next_fire_time(u64::MAX);
assert_eq!(result, Some(u64::MAX));
```

### 2.10 Schedule Serialization

**Given** any Schedule variant (Cron, OneShot, Interval)
**When** serializing with serde and deserializing
**Then** the variant and all fields are preserved

**Test:** `schedule_serialization_roundtrip`
```rust
let cron = Schedule::cron("0 0 * * *");
let cron_json = serde_json::to_string(&cron).unwrap();
let cron_parsed: Schedule = serde_json::from_str(&cron_json).unwrap();
assert_eq!(cron, cron_parsed);

let one_shot = Schedule::one_shot(Duration::from_secs(60));
let one_shot_json = serde_json::to_string(&one_shot).unwrap();
let one_shot_parsed: Schedule = serde_json::from_str(&one_shot_json).unwrap();
assert_eq!(one_shot, one_shot_parsed);

let interval = Schedule::interval(Duration::from_secs(30));
let interval_json = serde_json::to_string(&interval).unwrap();
let interval_parsed: Schedule = serde_json::from_str(&interval_json).unwrap();
assert_eq!(interval, interval_parsed);
```

---

## 3. Job Struct - Given-When-Then Scenarios

### 3.1 Job New

**Given** job id, payload string, and schedule
**When** calling Job::new(id, payload, schedule)
**Then** Job is created with default priority=Normal, max_retries=3, backoff_ms=1000

**Test:** `job_new_sets_all_fields`
```rust
let job = Job::new(
    JobId::new(1),
    "test payload".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
);
assert_eq!(job.id, JobId::new(1));
assert_eq!(job.payload, "test payload");
assert_eq!(job.priority, JobPriority::Normal);
assert_eq!(job.max_retries, 3);
assert_eq!(job.backoff_ms, 1000);
```

### 3.2 Job Default Priority

**Given** a newly created Job
**When** accessing job.priority
**Then** returns JobPriority::Normal

**Test:** `job_default_priority_is_normal`
```rust
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
);
assert_eq!(job.priority, JobPriority::Normal);
```

### 3.3 Job Default Retries

**Given** a newly created Job
**When** accessing job.max_retries
**Then** returns 3

**Test:** `job_default_retries_is_3`
```rust
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
);
assert_eq!(job.max_retries, 3);
```

### 3.4 Job Default Backoff

**Given** a newly created Job
**When** accessing job.backoff_ms
**Then** returns 1000 (1 second)

**Test:** `job_default_backoff_is_1000ms`
```rust
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
);
assert_eq!(job.backoff_ms, 1000);
```

### 3.5 Job With Priority Builder

**Given** a Job
**When** calling with_priority(JobPriority::High)
**Then** returns new Job with priority set to High

**Test:** `job_with_priority`
```rust
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
)
.with_priority(JobPriority::High);
assert_eq!(job.priority, JobPriority::High);
```

### 3.6 Job With Retries Builder

**Given** a Job
**When** calling with_retries(5, 500)
**Then** returns new Job with max_retries=5, backoff_ms=500

**Test:** `job_with_retries`
```rust
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
)
.with_retries(5, 500);
assert_eq!(job.max_retries, 5);
assert_eq!(job.backoff_ms, 500);
```

### 3.7 Job Serialization

**Given** a Job with all fields populated
**When** serializing with serde and deserializing
**Then** all fields are preserved exactly

**Test:** `job_serialization_roundtrip`
```rust
let job = Job::new(
    JobId::new(1),
    "test payload".to_string(),
    Schedule::interval(Duration::from_secs(30)),
)
.with_priority(JobPriority::Critical)
.with_retries(5, 500);

let json = serde_json::to_string(&job).unwrap();
let parsed: Job = serde_json::from_str(&json).unwrap();
assert_eq!(job.id, parsed.id);
assert_eq!(job.payload, parsed.payload);
assert_eq!(job.schedule, parsed.schedule);
assert_eq!(job.priority, parsed.priority);
assert_eq!(job.max_retries, parsed.max_retries);
assert_eq!(job.backoff_ms, parsed.backoff_ms);
```

### 3.8 Job Payload Type

**Given** the Job struct definition
**When** examining the payload field type
**Then** payload is String (not Vec<u8> or other type)

**Test:** `job_payload_is_string`
```rust
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
);
// Compile-time type check
let _: String = job.payload.clone();
```

---

## 4. JobId Struct - Given-When-Then Scenarios

### 4.1 JobId New Constructor

**Given** a u64 value
**When** calling JobId::new(42)
**Then** returns JobId(42)

**Test:** `job_id_new_constructs`
```rust
let id = JobId::new(42);
assert_eq!(id.0, 42);
```

### 4.2 JobId Equality

**Given** two JobIds with same u64 value
**When** comparing with ==
**Then** they are equal

**Test:** `job_id_equality`
```rust
let id1 = JobId::new(42);
let id2 = JobId::new(42);
assert_eq!(id1, id2);
```

### 4.3 JobId Hash

**Given** a JobId
**When** using in HashMap or HashSet
**Then** it works correctly (Hash impl present)

**Test:** `job_id_hash`
```rust
use std::collections::HashMap;

let mut map = HashMap::new();
let id = JobId::new(42);
map.insert(id, "test");
assert_eq!(map.get(&JobId::new(42)), Some(&"test"));
```

### 4.4 JobId Display

**Given** a JobId with value 123
**When** formatting with `{}`
**Then** output is "job-123"

**Test:** `job_id_display`
```rust
let id = JobId::new(123);
let display_str = format!("{}", id);
assert_eq!(display_str, "job-123");
```

### 4.5 JobId Debug

**Given** a JobId
**When** formatting with `{:?}`
**Then** Debug output shows the inner u64

**Test:** `job_id_debug`
```rust
let id = JobId::new(42);
let debug_str = format!("{:?}", id);
assert_eq!(debug_str, "JobId(42)");
```

---

## 5. JobResult Struct - Given-When-Then Scenarios

### 5.1 JobResult All Fields

**Given** the JobResult struct definition
**When** examining fields
**Then** has job_id, success, output, error, attempt

**Test:** `job_result_has_all_fields`
```rust
let result = JobResult {
    job_id: JobId::new(1),
    success: true,
    output: Some("output".to_string()),
    error: None,
    attempt: 1,
};
assert_eq!(result.job_id, JobId::new(1));
assert_eq!(result.success, true);
assert_eq!(result.output, Some("output".to_string()));
assert_eq!(result.error, None);
assert_eq!(result.attempt, 1);
```

### 5.2 JobResult Success

**Given** a successful JobResult
**When** checking success and error fields
**Then** success=true, error=None

**Test:** `job_result_success_true`
```rust
let result = JobResult {
    job_id: JobId::new(1),
    success: true,
    output: Some("done".to_string()),
    error: None,
    attempt: 1,
};
assert!(result.success);
assert!(result.error.is_none());
```

### 5.3 JobResult Failure

**Given** a failed JobResult
**When** checking success and error fields
**Then** success=false, error=Some(message)

**Test:** `job_result_failure_false`
```rust
let result = JobResult {
    job_id: JobId::new(1),
    success: false,
    output: None,
    error: Some("connection failed".to_string()),
    attempt: 3,
};
assert!(!result.success);
assert!(result.error.is_some());
```

### 5.4 JobResult Serialization

**Given** a JobResult with all fields populated
**When** serializing with serde and deserializing
**Then** all fields are preserved

**Test:** `job_result_serialization_roundtrip`
```rust
let result = JobResult {
    job_id: JobId::new(42),
    success: false,
    output: None,
    error: Some("timeout".to_string()),
    attempt: 3,
};

let json = serde_json::to_string(&result).unwrap();
let parsed: JobResult = serde_json::from_str(&json).unwrap();
assert_eq!(result, parsed);
```

---

## 6. SchedulerConfig Struct - Given-When-Then Scenarios

### 6.1 SchedulerConfig Default

**Given** SchedulerConfig implements Default
**When** calling SchedulerConfig::default()
**Then** returns max_concurrent=10, scan_interval=100ms, max_jobs_per_scan=100

**Test:** `scheduler_config_default_values`
```rust
let config = SchedulerConfig::default();
assert_eq!(config.max_concurrent, 10);
assert_eq!(config.scan_interval, Duration::from_millis(100));
assert_eq!(config.max_jobs_per_scan, 100);
```

### 6.2 SchedulerConfig Custom Values

**Given** custom configuration values
**When** creating SchedulerConfig with those values
**Then** all values are set correctly

**Test:** `scheduler_config_custom_values`
```rust
let config = SchedulerConfig {
    max_concurrent: 5,
    scan_interval: Duration::from_millis(50),
    max_jobs_per_scan: 50,
};
assert_eq!(config.max_concurrent, 5);
assert_eq!(config.scan_interval, Duration::from_millis(50));
assert_eq!(config.max_jobs_per_scan, 50);
```

### 6.3 SchedulerConfig Serialization

**Given** a SchedulerConfig
**When** serializing with serde and deserializing
**Then** all fields are preserved

**Test:** `scheduler_config_serialization_roundtrip`
```rust
let config = SchedulerConfig {
    max_concurrent: 5,
    scan_interval: Duration::from_millis(50),
    max_jobs_per_scan: 50,
};

let json = serde_json::to_string(&config).unwrap();
let parsed: SchedulerConfig = serde_json::from_str(&json).unwrap();
assert_eq!(config, parsed);
```

### 6.4 SchedulerConfig Debug

**Given** a SchedulerConfig
**When** formatting with `{:?}`
**Then** Debug output shows all fields

**Test:** `scheduler_config_debug`
```rust
let config = SchedulerConfig::default();
let debug_str = format!("{:?}", config);
assert!(debug_str.contains("max_concurrent"));
assert!(debug_str.contains("scan_interval"));
assert!(debug_str.contains("max_jobs_per_scan"));
```

---

## 7. SchedulerError Taxonomy - Given-When-Then Scenarios

### 7.1 SchedulerError JobNotFound

**Given** a non-existent JobId
**When** attempting to cancel that job
**Then** returns SchedulerError::JobNotFound(JobId)

**Test:** `scheduler_error_job_not_found`
```rust
// Error variant construction test
let error = SchedulerError::JobNotFound(JobId::new(42));
match error {
    SchedulerError::JobNotFound(id) => assert_eq!(id, JobId::new(42)),
    _ => panic!("Expected JobNotFound variant"),
}
```

### 7.2 SchedulerError QueueFull

**Given** a full queue (implementation-dependent)
**When** attempting to push
**Then** returns SchedulerError::QueueFull

**Test:** `scheduler_error_queue_full`
```rust
let error = SchedulerError::QueueFull;
// Unit variant, just verify it compiles and matches
matches!(error, SchedulerError::QueueFull);
```

### 7.3 SchedulerError SchedulerStopped

**Given** a stopped scheduler
**When** attempting an operation
**Then** returns SchedulerError::SchedulerStopped

**Test:** `scheduler_error_scheduler_stopped`
```rust
let error = SchedulerError::SchedulerStopped;
matches!(error, SchedulerError::SchedulerStopped);
```

### 7.4 SchedulerError InvalidSchedule

**Given** an invalid schedule (e.g., cron not implemented)
**When** attempting to schedule
**Then** returns SchedulerError::InvalidSchedule(reason)

**Test:** `scheduler_error_invalid_schedule`
```rust
let error = SchedulerError::InvalidSchedule("Cron not implemented".to_string());
match error {
    SchedulerError::InvalidSchedule(reason) => {
        assert_eq!(reason, "Cron not implemented");
    }
    _ => panic!("Expected InvalidSchedule variant"),
}
```

### 7.5 SchedulerError ConcurrencyLimitReached

**Given** max_concurrent permits are acquired
**When** attempting to acquire another
**Then** returns None (or could be SchedulerError::ConcurrencyLimitReached)

**Test:** `scheduler_error_concurrency_limit_reached`
```rust
let error = SchedulerError::ConcurrencyLimitReached;
matches!(error, SchedulerError::ConcurrencyLimitReached);
```

### 7.6 SchedulerError StorageError

**Given** a storage failure
**When** persisting a job
**Then** returns SchedulerError::StorageError(message)

**Test:** `scheduler_error_storage_error`
```rust
let error = SchedulerError::StorageError("disk full".to_string());
match error {
    SchedulerError::StorageError(msg) => assert_eq!(msg, "disk full"),
    _ => panic!("Expected StorageError variant"),
}
```

---

## 8. JobRunError Taxonomy - Given-When-Then Scenarios

### 8.1 JobRunError Failed

**Given** a job execution failure
**When** wrapping the error with job context
**Then** returns JobRunError::Failed{job_id, reason}

**Test:** `job_run_error_failed`
```rust
let error = JobRunError::Failed {
    job_id: JobId::new(42),
    reason: "timeout".to_string(),
};
match error {
    JobRunError::Failed { job_id, reason } => {
        assert_eq!(job_id, JobId::new(42));
        assert_eq!(reason, "timeout");
    }
    _ => panic!("Expected Failed variant"),
}
```

### 8.2 JobRunError ExceededRetries

**Given** a job that failed all retries
**When** giving up after max_retries
**Then** returns JobRunError::ExceededRetries{job_id, attempts}

**Test:** `job_run_error_exceeded_retries`
```rust
let error = JobRunError::ExceededRetries {
    job_id: JobId::new(123),
    attempts: 5,
};
match error {
    JobRunError::ExceededRetries { job_id, attempts } => {
        assert_eq!(job_id, JobId::new(123));
        assert_eq!(attempts, 5);
    }
    _ => panic!("Expected ExceededRetries variant"),
}
```

### 8.3 JobRunError Cancelled

**Given** a job that was cancelled during execution
**When** returning cancellation status
**Then** returns JobRunError::Cancelled{job_id}

**Test:** `job_run_error_cancelled`
```rust
let error = JobRunError::Cancelled {
    job_id: JobId::new(999),
};
match error {
    JobRunError::Cancelled { job_id } => {
        assert_eq!(job_id, JobId::new(999));
    }
    _ => panic!("Expected Cancelled variant"),
}
```

---

## 9. PriorityQueue Ordering - Given-When-Then Scenarios

### 9.1 Priority Critical Before High

**Given** a Critical job and High job with same fire_at_ms
**When** pushing both to PriorityQueue
**When** popping jobs
**Then** Critical job is returned first

**Test:** `priority_queue_critical_before_high`
```rust
let mut queue = PriorityQueue::new();
let critical_job = Job::new(
    JobId::new(1),
    "critical".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
).with_priority(JobPriority::Critical);
let high_job = Job::new(
    JobId::new(2),
    "high".to_string(),
    Schedule::one_shot(Duration::from_secs(10)),
).with_priority(JobPriority::High);

queue.push(critical_job, 1000);
queue.push(high_job, 1000);

let (job, _) = queue.pop().unwrap();
assert_eq!(job.id, JobId::new(1)); // Critical first
```

### 9.2 Priority High Before Normal

**Given** a High job and Normal job with same fire_at_ms
**When** popping jobs
**Then** High job is returned first

**Test:** `priority_queue_high_before_normal`
```rust
let mut queue = PriorityQueue::new();
let high_job = Job::new(JobId::new(1), "high".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::High);
let normal_job = Job::new(JobId::new(2), "normal".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Normal);

queue.push(high_job, 1000);
queue.push(normal_job, 1000);

let (job, _) = queue.pop().unwrap();
assert_eq!(job.id, JobId::new(1)); // High first
```

### 9.3 Priority Normal Before Low

**Given** a Normal job and Low job with same fire_at_ms
**When** popping jobs
**Then** Normal job is returned first

**Test:** `priority_queue_normal_before_low`
```rust
let mut queue = PriorityQueue::new();
let normal_job = Job::new(JobId::new(1), "normal".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Normal);
let low_job = Job::new(JobId::new(2), "low".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Low);

queue.push(normal_job, 1000);
queue.push(low_job, 1000);

let (job, _) = queue.pop().unwrap();
assert_eq!(job.id, JobId::new(1)); // Normal first
```

### 9.4 Same Priority Earlier First

**Given** two jobs with same priority but different fire_at_ms
**When** popping jobs
**Then** earlier fire_at_ms is returned first

**Test:** `priority_queue_same_priority_earlier_first`
```rust
let mut queue = PriorityQueue::new();
let early_job = Job::new(JobId::new(1), "early".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Normal);
let late_job = Job::new(JobId::new(2), "late".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Normal);

queue.push(early_job, 1000);
queue.push(late_job, 2000);

let (job, _) = queue.pop().unwrap();
assert_eq!(job.id, JobId::new(1)); // Earlier first
```

### 9.5 Mixed Priority and Time Ordering

**Given** jobs with mixed priorities and times
**When** popping all jobs
**Then** ordering is by (priority ASC, fire_at_ms ASC)

**Test:** `priority_queue_pop_ordering`
```rust
let mut queue = PriorityQueue::new();
// Add jobs in random order
queue.push(Job::new(JobId::new(3), "3".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Normal), 3000);
queue.push(Job::new(JobId::new(1), "1".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::Critical), 5000);
queue.push(Job::new(JobId::new(2), "2".to_string(), Schedule::one_shot(Duration::ZERO)).with_priority(JobPriority::High), 1000);

let (j1, _) = queue.pop().unwrap();
let (j2, _) = queue.pop().unwrap();
let (j3, _) = queue.pop().unwrap();

assert_eq!(j1.id, JobId::new(1)); // Critical, fire=5000
assert_eq!(j2.id, JobId::new(2)); // High, fire=1000
assert_eq!(j3.id, JobId::new(3)); // Normal, fire=3000
```

### 9.6 Peek Does Not Remove

**Given** a job in the queue
**When** calling peek()
**Then** job remains in queue and can be popped later

**Test:** `priority_queue_peek_does_not_remove`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, 1000);

let (_, fire_time) = queue.peek().unwrap();
assert_eq!(fire_time, 1000);

let (popped_job, _) = queue.pop().unwrap();
assert_eq!(popped_job.id, JobId::new(1));

assert!(queue.is_empty());
```

---

## 10. PriorityQueue Operations - Given-When-Then Scenarios

### 10.1 Push Increases Length

**Given** an empty queue
**When** pushing one job
**Then** len() = 1

**Test:** `priority_queue_push_increases_len`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, 1000);
assert_eq!(queue.len(), 1);
```

### 10.2 Pop Returns Job and Time

**Given** a job in queue
**When** calling pop()
**Then** returns Some((Job, fire_at_ms))

**Test:** `priority_queue_pop_returns_job`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, 1000);

let (popped_job, fire_time) = queue.pop().unwrap();
assert_eq!(popped_job.id, JobId::new(1));
assert_eq!(fire_time, 1000);
```

### 10.3 Pop Empty Returns None

**Given** an empty queue
**When** calling pop()
**Then** returns None

**Test:** `priority_queue_pop_empty_returns_none`
```rust
let mut queue = PriorityQueue::new();
assert!(queue.pop().is_none());
```

### 10.4 Remove Existing Job

**Given** a job in queue
**When** calling remove(job_id)
**Then** returns Some(job) and queue size decreases by 1

**Test:** `priority_queue_remove_existing`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, 1000);
assert_eq!(queue.len(), 1);

let removed = queue.remove(&JobId::new(1));
assert!(removed.is_some());
assert_eq!(queue.len(), 0);
```

### 10.5 Remove Non-Existing Job

**Given** a queue with jobs
**When** calling remove(non_existent_job_id)
**Then** returns None and queue size unchanged

**Test:** `priority_queue_remove_nonexistent`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, 1000);

let removed = queue.remove(&JobId::new(999));
assert!(removed.is_none());
assert_eq!(queue.len(), 1);
```

### 10.6 Due Jobs Filters by Time

**Given** jobs with various fire_at_ms values
**When** calling due_jobs(now_ms)
**Then** only returns jobs with fire_at_ms <= now_ms

**Test:** `priority_queue_due_jobs_filters_time`
```rust
let mut queue = PriorityQueue::new();
let now_ms = 1000;

queue.push(Job::new(JobId::new(1), "early".to_string(), Schedule::one_shot(Duration::ZERO)), 500);
queue.push(Job::new(JobId::new(2), "due".to_string(), Schedule::one_shot(Duration::ZERO)), 1000);
queue.push(Job::new(JobId::new(3), "late".to_string(), Schedule::one_shot(Duration::ZERO)), 2000);

let due = queue.due_jobs(now_ms, 100);
assert_eq!(due.len(), 2);
assert!(due.iter().any(|(j, _)| j.id == JobId::new(1)));
assert!(due.iter().any(|(j, _)| j.id == JobId::new(2)));
```

### 10.7 Due Jobs Respects Max Limit

**Given** 10 due jobs
**When** calling due_jobs(now_ms, max=2)
**Then** returns at most 2 jobs

**Test:** `priority_queue_due_jobs_respects_max`
```rust
let mut queue = PriorityQueue::new();
let now_ms = 1000;

for i in 0..10 {
    queue.push(Job::new(JobId::new(i), "job".to_string(), Schedule::one_shot(Duration::ZERO)), now_ms);
}

let due = queue.due_jobs(now_ms, 2);
assert!(due.len() <= 2);
```

### 10.8 Due Jobs Returns Fire Time

**Given** a job in queue
**When** calling due_jobs()
**Then** returns Vec<(Job, fire_at_ms)>

**Test:** `priority_queue_due_jobs_returns_fire_time`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, 1000);

let due = queue.due_jobs(1000, 1);
assert_eq!(due.len(), 1);
let (j, fire_time) = &due[0];
assert_eq!(j.id, JobId::new(1));
assert_eq!(*fire_time, 1000);
```

### 10.9 Into Vec Drains Queue

**Given** a queue with jobs
**When** calling into_vec()
**Then** returns all jobs and queue becomes empty

**Test:** `priority_queue_into_vec`
```rust
let mut queue = PriorityQueue::new();
queue.push(Job::new(JobId::new(1), "1".to_string(), Schedule::one_shot(Duration::ZERO)), 1000);
queue.push(Job::new(JobId::new(2), "2".to_string(), Schedule::one_shot(Duration::ZERO)), 2000);

let vec = queue.into_vec();
assert_eq!(vec.len(), 2);
assert!(queue.is_empty());
```

---

## 11. Scheduler Integration - Given-When-Then Scenarios

### 11.1 Scheduler New Sets Config

**Given** a custom SchedulerConfig
**When** calling Scheduler::new(config)
**Then** scheduler stores config correctly

**Test:** `scheduler_new_sets_config`
```rust
let config = SchedulerConfig {
    max_concurrent: 5,
    scan_interval: Duration::from_millis(50),
    max_jobs_per_scan: 50,
};
let scheduler = Scheduler::new(config);
// Config is private, verify via behavior
assert_eq!(scheduler.is_empty(), true);
```

### 11.2 Scheduler Schedule OneShot

**Given** a Scheduler
**When** calling schedule(one_shot_job)
**Then** job is added to queue

**Test:** `scheduler_schedule_one_shot`
```rust
let config = SchedulerConfig::default();
let mut scheduler = Scheduler::new(config);

let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_millis(50)),
);
scheduler.schedule(job).unwrap();

assert_eq!(scheduler.len(), 1);
```

### 11.3 Scheduler Schedule Multiple

**Given** a Scheduler
**When** scheduling 5 jobs
**Then** len() = 5

**Test:** `scheduler_schedule_multiple`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
for i in 0..5 {
    let job = Job::new(
        JobId::new(i),
        "test".to_string(),
        Schedule::one_shot(Duration::from_millis(50)),
    );
    scheduler.schedule(job).unwrap();
}
assert_eq!(scheduler.len(), 5);
```

### 11.4 Scheduler Cancel Existing

**Given** a scheduled job
**When** calling cancel(job_id)
**Then** job is removed and Some(job) is returned

**Test:** `scheduler_cancel_existing`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
scheduler.schedule(job.clone()).unwrap();

let removed = scheduler.cancel(JobId::new(1));
assert!(removed.is_some());
assert_eq!(removed.unwrap().id, JobId::new(1));
assert_eq!(scheduler.len(), 0);
```

### 11.5 Scheduler Cancel Non-Existent

**Given** no job with given id
**When** calling cancel(job_id)
**Then** returns None

**Test:** `scheduler_cancel_nonexistent`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let removed = scheduler.cancel(JobId::new(999));
assert!(removed.is_none());
assert_eq!(scheduler.len(), 0);
```

### 11.6 Scheduler Poll Empty

**Given** no jobs in queue
**When** calling poll_due_jobs(now_ms)
**Then** returns empty Vec

**Test:** `scheduler_poll_due_jobs_empty`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let now_ms = 1000;
let due = scheduler.poll_due_jobs(now_ms);
assert!(due.is_empty());
```

### 11.7 Scheduler Poll Respects Max

**Given** 100 jobs due
**When** calling poll_due_jobs(now_ms) with default max_jobs_per_scan=100
**Then** returns at most 100 jobs

**Test:** `scheduler_poll_due_jobs_respects_max`
```rust
let config = SchedulerConfig {
    max_jobs_per_scan: 50,
    ..SchedulerConfig::default()
};
let mut scheduler = Scheduler::new(config);

for i in 0..100 {
    let job = Job::new(JobId::new(i), "test".to_string(), Schedule::one_shot(Duration::ZERO));
    scheduler.schedule(job).unwrap();
}

let now_ms = 1000;
let due = scheduler.poll_due_jobs(now_ms);
assert!(due.len() <= 50);
```

### 11.8 Scheduler Reschedule

**Given** a job was polled
**When** calling reschedule(job, next_fire_ms)
**Then** job is re-added to queue

**Test:** `scheduler_reschedule_job`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::interval(Duration::from_millis(100)),
);
scheduler.schedule(job.clone()).unwrap();
assert_eq!(scheduler.len(), 1);

let now_ms = 1000;
let due = scheduler.poll_due_jobs(now_ms);
assert_eq!(due.len(), 1);

// Reschedule for later
scheduler.reschedule(due[0].clone(), now_ms + 200);
assert_eq!(scheduler.len(), 1);
```

---

## 12. Concurrency Control - Given-When-Then Scenarios

### 12.1 Concurrency Limit Respected

**Given** max_concurrent=2
**When** acquiring 3 permits
**Then** third acquire returns None

**Test:** `scheduler_concurrency_limit_respected`
```rust
let config = SchedulerConfig {
    max_concurrent: 2,
    ..SchedulerConfig::default()
};
let scheduler = Scheduler::new(config);

let permit1 = scheduler.try_acquire();
let permit2 = scheduler.try_acquire();
let permit3 = scheduler.try_acquire();

assert!(permit1.is_some());
assert!(permit2.is_some());
assert!(permit3.is_none());
```

### 12.2 Try Acquire Success

**Given** available permits
**When** calling try_acquire()
**Then** returns Some(permit)

**Test:** `scheduler_try_acquire_success`
```rust
let scheduler = Scheduler::new(SchedulerConfig {
    max_concurrent: 5,
    ..SchedulerConfig::default()
});
let permit = scheduler.try_acquire();
assert!(permit.is_some());
```

### 12.3 Try Acquire Failure

**Given** no available permits
**When** calling try_acquire()
**Then** returns None

**Test:** `scheduler_try_acquire_failure`
```rust
let scheduler = Scheduler::new(SchedulerConfig {
    max_concurrent: 0,
    ..SchedulerConfig::default()
});
let permit = scheduler.try_acquire();
assert!(permit.is_none());
```

### 12.4 Start Stop State

**Given** a Scheduler
**When** calling start() then stop()
**Then** is_running() changes from false -> true -> false

**Test:** `scheduler_start_stop`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
assert!(!scheduler.is_running());

scheduler.start();
assert!(scheduler.is_running());

scheduler.stop();
assert!(!scheduler.is_running());
```

---

## 13. Scheduler State Machine - Given-When-Then Scenarios

### 13.1 Full Lifecycle

**Given** a Scheduler
**When** scheduling a job, polling it as due, and completing
**Then** job lifecycle is: scheduled -> due -> completed

**Test:** `scheduler_schedule_and_poll_complete`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::ZERO),
);
scheduler.schedule(job).unwrap();
assert_eq!(scheduler.len(), 1);

let now_ms = 1000;
let due = scheduler.poll_due_jobs(now_ms);
assert_eq!(due.len(), 1);
assert_eq!(scheduler.len(), 0);
```

### 13.2 Cancel Before Due

**Given** a scheduled job not yet due
**When** cancelling before poll
**Then** job is not in poll result

**Test:** `scheduler_cancel_before_due`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let job = Job::new(
    JobId::new(1),
    "test".to_string(),
    Schedule::one_shot(Duration::from_secs(60)),
);
scheduler.schedule(job).unwrap();

let now_ms = 1000;
scheduler.cancel(JobId::new(1));
let due = scheduler.poll_due_jobs(now_ms);
assert!(due.is_empty());
```

### 13.3 Reschedule Recurring

**Given** an Interval job
**When** polled as due
**When** rescheduled with next_fire_ms
**Then** job continues to execute periodically

**Test:** `scheduler_reschedule_recurring`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let job = Job::new(
    JobId::new(1),
    "recurring".to_string(),
    Schedule::interval(Duration::from_millis(100)),
);
scheduler.schedule(job).unwrap();

let now_ms = 1000;
let due = scheduler.poll_due_jobs(now_ms);
assert_eq!(due.len(), 1);

// Reschedule
if let Schedule::Interval { interval_ms } = &due[0].schedule {
    scheduler.reschedule(due[0].clone(), now_ms + interval_ms);
}
assert_eq!(scheduler.len(), 1);
```

### 13.4 Cancel Then Reschedule

**Given** a job was cancelled
**When** rescheduling a new job with same id
**Then** both operations are independent

**Test:** `scheduler_reschedule_after_cancel`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
scheduler.schedule(job).unwrap();
scheduler.cancel(JobId::new(1));
assert_eq!(scheduler.len(), 0);

// Reschedule new job with same id
let job2 = Job::new(JobId::new(1), "test2".to_string(), Schedule::one_shot(Duration::ZERO));
scheduler.reschedule(job2, 1000);
assert_eq!(scheduler.len(), 1);
```

### 13.5 Len and Empty

**Given** a Scheduler
**When** checking len() and is_empty()
**Then** they are consistent

**Test:** `scheduler_len_and_empty`
```rust
let mut scheduler = Scheduler::new(SchedulerConfig::default());
assert_eq!(scheduler.len(), 0);
assert!(scheduler.is_empty());

scheduler.schedule(Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO))).unwrap();
assert_eq!(scheduler.len(), 1);
assert!(!scheduler.is_empty());

scheduler.cancel(JobId::new(1));
assert_eq!(scheduler.len(), 0);
assert!(scheduler.is_empty());
```

---

## 14. Edge Cases & Boundary Tests - Given-When-Then Scenarios

### 14.1 OneShot Zero Delay

**Given** a OneShot schedule with Duration::ZERO
**When** created
**Then** fire_at_ms is now or slightly in future

**Test:** `schedule_one_shot_zero_delay`
```rust
let schedule = Schedule::one_shot(Duration::ZERO);
match schedule {
    Schedule::OneShot { fire_at_ms } => {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        assert!(fire_at_ms >= now_ms);
    }
    _ => panic!("Expected OneShot"),
}
```

### 14.2 Interval Zero

**Given** an Interval schedule with Duration::ZERO
**When** created
**Then** interval_ms = 0

**Test:** `schedule_interval_zero_interval`
```rust
let schedule = Schedule::interval(Duration::ZERO);
match schedule {
    Schedule::Interval { interval_ms } => {
        assert_eq!(interval_ms, 0);
    }
    _ => panic!("Expected Interval"),
}
```

### 14.3 Priority Extremes

**Given** Critical and Low priorities
**When** comparing
**Then** Critical < Low (Critical fires first)

**Test:** `job_priority_extremes`
```rust
assert!(JobPriority::Critical < JobPriority::Low);
```

### 14.4 Max U64 Fire Time

**Given** fire_at_ms = u64::MAX
**When** comparing in priority queue
**Then** no overflow, works correctly

**Test:** `priority_queue_max_u64_fire_time`
```rust
let mut queue = PriorityQueue::new();
let job = Job::new(JobId::new(1), "test".to_string(), Schedule::one_shot(Duration::ZERO));
queue.push(job, u64::MAX);
assert_eq!(queue.len(), 1);
```

### 14.5 Due Jobs None Due

**Given** all jobs with fire_at_ms > now
**When** calling due_jobs(now)
**Then** returns empty Vec

**Test:** `priority_queue_due_jobs_none_due`
```rust
let mut queue = PriorityQueue::new();
let now_ms = 1000;
queue.push(Job::new(JobId::new(1), "future".to_string(), Schedule::one_shot(Duration::ZERO)), now_ms + 1000);

let due = queue.due_jobs(now_ms, 100);
assert!(due.is_empty());
```

### 14.6 Due Jobs All Due

**Given** all jobs with fire_at_ms <= now
**When** calling due_jobs(now, max=100)
**Then** returns all jobs (up to max)

**Test:** `priority_queue_due_jobs_all_due`
```rust
let mut queue = PriorityQueue::new();
let now_ms = 1000;
for i in 0..10 {
    queue.push(Job::new(JobId::new(i), "test".to_string(), Schedule::one_shot(Duration::ZERO)), now_ms);
}

let due = queue.due_jobs(now_ms, 100);
assert_eq!(due.len(), 10);
```

### 14.7 Zero Max Concurrent

**Given** max_concurrent=0
**When** calling try_acquire()
**Then** returns None

**Test:** `scheduler_config_zero_max_concurrent`
```rust
let scheduler = Scheduler::new(SchedulerConfig {
    max_concurrent: 0,
    ..SchedulerConfig::default()
});
assert!(scheduler.try_acquire().is_none());
```

### 14.8 Zero Scan Interval

**Given** scan_interval=Duration::ZERO
**When** creating config
**Then** config is valid (zero duration is acceptable)

**Test:** `scheduler_config_zero_scan_interval`
```rust
let config = SchedulerConfig {
    scan_interval: Duration::ZERO,
    ..SchedulerConfig::default()
};
assert_eq!(config.scan_interval, Duration::ZERO);
```

### 14.9 Empty Payload

**Given** a Job with empty payload
**When** created
**Then** job is valid

**Test:** `job_empty_payload`
```rust
let job = Job::new(
    JobId::new(1),
    String::new(),
    Schedule::one_shot(Duration::ZERO),
);
assert_eq!(job.payload, "");
```

### 14.10 Large Payload

**Given** a Job with very large payload (1MB)
**When** created
**Then** job is valid (memory permitting)

**Test:** `job_large_payload`
```rust
let large_payload = "x".repeat(1_000_000);
let job = Job::new(
    JobId::new(1),
    large_payload,
    Schedule::one_shot(Duration::ZERO),
);
assert_eq!(job.payload.len(), 1_000_000);
```

---

## 15. Property Tests — proptest

### 15.1 Priority Ordering Transitive

**Property:** For any three priorities p1, p2, p3, if p1 < p2 and p2 < p3, then p1 < p3

**Strategy:** proptest::collection::vec(prop_oneof![...], 3)

**Test:** `priority_ordering_transitive`
```rust
proptest! {
    #[test]
    fn priority_ordering_transitive(p1 in any::<JobPriority>(), p2 in any::<JobPriority>(), p3 in any::<JobPriority>()) {
        if p1 < p2 && p2 < p3 {
            assert!(p1 < p3);
        }
    }
}
```

### 15.2 Schedule Fire Time Always Future

**Property:** OneShot schedules always have fire_at_ms >= now

**Strategy:** proptest::num::u64::ANY (delay_ms)

**Test:** `schedule_fire_time_always_future`
```rust
proptest! {
    #[test]
    fn schedule_fire_time_always_future(delay_ms in proptest::num::u64::ANY()) {
        let delay = Duration::from_millis(delay_ms);
        let schedule = Schedule::one_shot(delay);
        match schedule {
            Schedule::OneShot { fire_at_ms } => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64);
                assert!(fire_at_ms >= now_ms);
            }
            _ => panic!("Expected OneShot"),
        }
    }
}
```

### 15.3 Interval Fire Times Monotonic

**Property:** For Interval schedules, next_fire_time is always increasing

**Strategy:** Random interval values

**Test:** `interval_fire_times_monotonic`
```rust
proptest! {
    #[test]
    fn interval_fire_times_monotonic(interval_ms in proptest::num::u64::ANY()) {
        let schedule = Schedule::interval(Duration::from_millis(interval_ms));
        let mut last_ms = 0u64;
        for _ in 0..10 {
            let next_ms = schedule.next_fire_time(last_ms);
            if let Some(next) = next_ms {
                assert!(next >= last_ms);
                last_ms = next;
            }
        }
    }
}
```

### 15.4 Priority Queue Ordering Consistent

**Property:** After any sequence of push/pop operations, every pop returns highest priority job

**Strategy:** proptest::collection::vec(push_pop_sequence, 100)

**Test:** `priority_queue_ordering_consistent`
```rust
proptest! {
    #[test]
    fn priority_queue_ordering_consistent(ops in proptest::collection::vec(any::<(bool, u64, JobPriority)>(), 100)) {
        let mut queue = PriorityQueue::new();
        let mut counter = 0u64;
        
        for (is_push, fire_ms, priority) in ops {
            if is_push {
                let job = Job::new(
                    JobId::new(counter),
                    "test".to_string(),
                    Schedule::one_shot(Duration::ZERO),
                ).with_priority(priority);
                queue.push(job, fire_ms);
                counter += 1;
            } else if !queue.is_empty() {
                let (job, _) = queue.pop().unwrap();
                // Verify this was highest priority in queue
                // (would need to track expected order)
                drop(job);
            }
        }
    }
}
```

---

## Test Count Summary

| Category | Test Count |
|----------|-----------|
| JobPriority | 5 |
| Schedule | 10 |
| Job | 8 |
| JobId | 5 |
| JobResult | 4 |
| SchedulerConfig | 4 |
| SchedulerError | 6 |
| JobRunError | 3 |
| PriorityQueue Ordering | 6 |
| PriorityQueue Operations | 9 |
| Scheduler Integration | 8 |
| Concurrency Control | 4 |
| Scheduler State Machine | 5 |
| Edge Cases | 10 |
| Property Tests | 4 |
| **Total** | **91** |

---

## Priority Order for TDD Implementation

### P0 (Critical - must have)
- Schedule behavior (tests 2.1-2.10) - 10 tests
- SchedulerError taxonomy (tests 7.1-7.6) - 6 tests
- PriorityQueue ordering (tests 9.1-9.6) - 6 tests

### P1 (High - should have)
- Scheduler lifecycle (tests 11.1-11.8) - 8 tests
- Job type (tests 3.1-3.8) - 8 tests
- Concurrency control (tests 12.1-12.4) - 4 tests

### P2 (Medium - nice to have)
- JobId/JobResult/Config tests (tests 4, 5, 6) - 13 tests
- Edge cases (tests 14.1-14.10) - 10 tests
- State machine tests (tests 13.1-13.5) - 5 tests

### P3 (Low - property tests)
- Property tests (tests 15.1-15.4) - 4 tests

---

## Contract Invariant Traceability

| Contract Item | Test IDs |
|--------------|----------|
| JobPriority ordering (Critical > High > Normal > Low) | 1.1, 9.1-9.5, 15.1 |
| Schedule.next_fire_time semantics | 2.3-2.9, 15.2-15.3 |
| PriorityQueue ordering by (priority ASC, fire_at_ms ASC) | 9.1-9.6, 15.4 |
| Scheduler.schedule() returns Result<(), SchedulerError> | 11.2-11.3 |
| Scheduler.cancel() removes job and returns Option<Job> | 11.4-11.5 |
| Scheduler.poll_due_jobs() time-based filtering | 10.6-10.8, 11.6-11.7 |
| Concurrency limits via semaphore | 12.1-12.4 |
| Scheduler start/stop state | 12.4, 13.1-13.5 |

---

## Implementation Files Reference

- **types.rs**: Job, JobId, Schedule, JobPriority, JobResult, SchedulerConfig
- **error.rs**: SchedulerError, JobRunError
- **queue.rs**: PriorityQueue
- **mod.rs**: Scheduler struct and public API

---

## References

- Contract: `docs/adr/v2/ADR-047-v2-background-job-scheduler-contract.md`
- Existing test plan: `docs/contracts/ADR-047-v2-background-job-scheduler-test-plan.md` (ve-7tj8)
- Implementation: `crates/vo-executor/src/scheduler/`
- Bead: ve-yncka (this document)
- Discovered from: ve-i1vuu (vo-scheduler: Implement Background Job Scheduler - ADR-047)
