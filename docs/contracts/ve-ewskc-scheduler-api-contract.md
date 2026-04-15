# Design-by-Contract: Scheduler API Operations

**Bead:** ve-ewskc
**Contract:** ADR-047-v2 Background Job Scheduler
**Implementation:** `crates/vo-executor/src/scheduler/`
**Design State:** Go State 1 - Design-by-contract

---

## Overview

This document specifies the design-by-contract for scheduler API operations. Each operation has preconditions, postconditions, invariants, and error handling semantics. The contract follows the ADR-047-v2 specification.

---

## 1. Core Types (Go State 1.1)

### 1.1 JobId

```rust
pub struct JobId(pub u64);
```

**Invariants:**
- `JobId` is always a valid u64 (no special validation needed)
- `JobId` is immutable once assigned
- `JobId` implements `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

**Constructor:**
```rust
impl JobId {
    pub fn new(id: u64) -> Self { Self(id) }
}
```

**Invariants:**
- **INV-JOBID-1**: Every `JobId` is created via `JobId::new(u64)`
- **INV-JOBID-2**: `JobId` values are never mutated after creation

---

### 1.2 JobPriority

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
}
```

**Invariants:**
- **INV-PRIORITY-1**: Lower value = higher priority (Critical < High < Normal < Low)
- **INV-PRIORITY-2**: Default priority is `Normal`
- **INV-PRIORITY-3**: Ordering is total (all variants are comparable)

**Preconditions:**
- None (constructible from enum literals)

**Postconditions:**
- **POST-PRIORITY-1**: `JobPriority::Critical < JobPriority::High`
- **POST-PRIORITY-2**: `JobPriority::High < JobPriority::Normal`
- **POST-PRIORITY-3**: `JobPriority::Normal < JobPriority::Low`

---

### 1.3 Schedule

```rust
pub enum Schedule {
    Cron(String),
    OneShot { fire_at_ms: u64 },
    Interval { interval_ms: u64 },
}
```

**Invariants:**
- **INV-SCHEDULE-1**: `Cron` variant stores cron expression string
- **INV-SCHEDULE-2**: `OneShot.fire_at_ms` is always in the future (>= now)
- **INV-SCHEDULE-3**: `Interval.interval_ms` > 0

**Constructor Invariants:**
```rust
impl Schedule {
    pub fn cron(expr: &str) -> Self { Self::Cron(expr.to_string()) }
    
    pub fn one_shot(delay: Duration) -> Self {
        let fire_at_ms = now_ms() + delay.as_millis() as u64;
        Self::OneShot { fire_at_ms }
    }
    
    pub fn interval(interval: Duration) -> Self {
        Self::Interval {
            interval_ms: interval.as_millis() as u64,
        }
    }
}
```

**Preconditions:**
- **PRE-ONE-SHOT-1**: `delay` must be > 0 (zero delay allowed but fire_at_ms = now)
- **PRE-INTERVAL-1**: `interval` must be > 0

**Postconditions:**
- **POST-ONE-SHOT-1**: `fire_at_ms >= now_ms()`
- **POST-INTERVAL-1**: `interval_ms > 0`

**Method Contracts:**

#### `Schedule::next_fire_time(&self, last_fire_ms: u64) -> Option<u64>`

**Preconditions:**
- **PRE-NEXT-FIRE-1**: `last_fire_ms == 0` for initial call
- **PRE-NEXT-FIRE-2**: `last_fire_ms > 0` for subsequent calls

**Postconditions:**
- **POST-NEXT-FIRE-CRON**: Returns `None` (cron not implemented)
- **POST-NEXT-FIRE-ONESHOT-FIRST**: If `last_fire_ms == 0`, returns `Some(fire_at_ms)`
- **POST-NEXT-FIRE-ONESHOT-SECOND**: If `last_fire_ms != 0`, returns `None` (one-shot semantics)
- **POST-NEXT-FIRE-INTERVAL**: Returns `Some(last_fire_ms + interval_ms)` (monotonically increasing)

**Invariants:**
- **INV-NEXT-FIRE-1**: For `OneShot`, only first call with `last_fire_ms == 0` returns value
- **INV-NEXT-FIRE-2**: For `Interval`, all calls return monotonically increasing values
- **INV-NEXT-FIRE-3**: No overflow (uses `saturating_add`)

---

### 1.4 Job

```rust
pub struct Job {
    pub id: JobId,
    pub payload: String,
    pub schedule: Schedule,
    pub priority: JobPriority,
    pub max_retries: u32,
    pub backoff_ms: u64,
}
```

**Invariants:**
- **INV-JOB-1**: `id` is never mutated
- **INV-JOB-2**: `payload` is always a valid UTF-8 string
- **INV-JOB-3**: `priority` defaults to `JobPriority::Normal`
- **INV-JOB-4**: `max_retries` defaults to 3
- **INV-JOB-5**: `backoff_ms` defaults to 1000 (1 second)

**Constructor Contract:**
```rust
impl Job {
    pub fn new(id: JobId, payload: String, schedule: Schedule) -> Self {
        Self {
            id,
            payload,
            schedule,
            priority: JobPriority::Normal,
            max_retries: 3,
            backoff_ms: 1000,
        }
    }
}
```

**Preconditions:**
- **PRE-JOB-NEW-1**: `id` must be valid `JobId` (always true via constructor)
- **PRE-JOB-NEW-2**: `payload` must be valid UTF-8 (always true for `String`)
- **PRE-JOB-NEW-3**: `schedule` must be constructible (always true)

**Postconditions:**
- **POST-JOB-NEW-1**: `priority == JobPriority::Normal`
- **POST-JOB-NEW-2**: `max_retries == 3`
- **POST-JOB-NEW-3**: `backoff_ms == 1000`

**Builder Methods:**

#### `Job::with_priority(self, priority: JobPriority) -> Self`

**Preconditions:**
- None (always valid)

**Postconditions:**
- **POST-WITH-PRIORITY-1**: Returns new `Job` with `priority` set to argument

#### `Job::with_retries(self, max_retries: u32, backoff_ms: u64) -> Self`

**Preconditions:**
- None (always valid)

**Postconditions:**
- **POST-WITH-RETRIES-1**: Returns new `Job` with `max_retries` and `backoff_ms` set

---

### 1.5 JobResult

```rust
pub struct JobResult {
    pub job_id: JobId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub attempt: u32,
}
```

**Invariants:**
- **INV-JOBRESULT-1**: If `success == true`, then `error == None`
- **INV-JOBRESULT-2**: If `success == false`, then `error == Some(_)`
- **INV-JOBRESULT-3**: `attempt` is always >= 1

**Constructor Contract:**
```rust
pub fn success(job_id: JobId, output: String, attempt: u32) -> Self {
    Self {
        job_id,
        success: true,
        output: Some(output),
        error: None,
        attempt,
    }
}

pub fn failure(job_id: JobId, error: String, attempt: u32) -> Self {
    Self {
        job_id,
        success: false,
        output: None,
        error: Some(error),
        attempt,
    }
}
```

**Preconditions:**
- **PRE-JOBRESULT-SUCCESS-1**: `attempt >= 1`
- **PRE-JOBRESULT-FAILURE-1**: `attempt >= 1`

**Postconditions:**
- **POST-JOBRESULT-SUCCESS-1**: `success == true`, `error == None`
- **POST-JOBRESULT-FAILURE-1**: `success == false`, `error == Some(_)`

---

### 1.6 SchedulerConfig

```rust
pub struct SchedulerConfig {
    pub max_concurrent: usize,
    pub scan_interval: Duration,
    pub max_jobs_per_scan: u32,
}
```

**Invariants:**
- **INV-CONFIG-1**: `max_concurrent >= 0`
- **INV-CONFIG-2**: `scan_interval > 0`
- **INV-CONFIG-3**: `max_jobs_per_scan > 0`

**Default Contract:**
```rust
impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(100),
            max_jobs_per_scan: 100,
        }
    }
}
```

**Preconditions:**
- **PRE-CONFIG-DEFAULT-1**: None (always constructible)

**Postconditions:**
- **POST-CONFIG-DEFAULT-1**: `max_concurrent == 10`
- **POST-CONFIG-DEFAULT-2**: `scan_interval == 100ms`
- **POST-CONFIG-DEFAULT-3**: `max_jobs_per_scan == 100`

---

## 2. Error Taxonomy (Go State 1.2)

### 2.1 SchedulerError

```rust
pub enum SchedulerError {
    QueueFull,
    InvalidSchedule(String),
    JobNotFound(JobId),
    InvalidTransition,
    SchedulerStopped,
    ConcurrencyLimitReached,
    StorageError(String),
}
```

**Classification:**
| Error | Category | Retryable | Description |
|-------|----------|-----------|-------------|
| `QueueFull` | Resource | No | Scheduler queue at capacity |
| `InvalidSchedule` | Permanent | No | Schedule policy is malformed |
| `JobNotFound` | Permanent | No | Requested job doesn't exist |
| `InvalidTransition` | Permanent | No | State transition not allowed |
| `SchedulerStopped` | Resource | Yes | Scheduler not running (retryable) |
| `ConcurrencyLimitReached` | Resource | Yes | No available permits (retryable) |
| `StorageError` | Transient | Yes | Storage failure (retryable) |

**Preconditions:**
- **PRE-SCHEDULERERROR-1**: Each error variant is constructed with correct fields

**Postconditions:**
- **POST-SCHEDULERERROR-1**: Errors implement `Display` and `std::error::Error`
- **POST-SCHEDULERERROR-2**: Errors are serializable via serde

---

### 2.2 JobRunError

```rust
pub enum JobRunError {
    Failed { job_id: JobId, reason: String },
    ExceededRetries { job_id: JobId, attempts: u32 },
    Cancelled { job_id: JobId },
}
```

**Classification:**
| Error | Category | Retryable | Description |
|-------|----------|-----------|-------------|
| `Failed` | Execution | Depends on retry policy | Job execution failed |
| `ExceededRetries` | Exhaustion | No | All retries exhausted |
| `Cancelled` | Cancellation | No | Job was cancelled |

**Preconditions:**
- **PRE-JOBRUNERROR-1**: `job_id` is always provided
- **PRE-JOBRUNERROR-2**: `reason` (if present) describes failure cause

**Postconditions:**
- **POST-JOBRUNERROR-1**: Errors are serializable via serde
- **POST-JOBRUNERROR-2**: Errors implement `Display` and `std::error::Error`

---

## 3. API Operations (Go State 1.3)

### 3.1 schedule_job

```rust
async fn schedule_job(
    queue: &mut SchedulerQueue,
    job: Job,
) -> Result<JobId, SchedulerError>
```

**Purpose:** Add a new job to the scheduler queue.

**Preconditions:**
- **PRE-SCHEDULE-1**: `job.id` is valid `JobId`
- **PRE-SCHEDULE-2**: `job.schedule` is constructible (not `Cron`)
- **PRE-SCHEDULE-3**: `queue` is not in `Stopped` state
- **PRE-SCHEDULE-4**: `queue` has capacity (not `QueueFull`)

**Postconditions:**
- **POST-SCHEDULE-1**: Job is added to priority queue
- **POST-SCHEDULE-2**: Returns `Ok(job.id)` with assigned job ID
- **POST-SCHEDULE-3**: Job state is `Scheduled` (not yet due)
- **POST-SCHEDULE-4**: Job will be returned by `poll_due_jobs` when `fire_at_ms <= now_ms`

**Error Conditions:**
- **ERR-SCHEDULE-1**: `SchedulerError::InvalidSchedule` if schedule policy is malformed
- **ERR-SCHEDULE-2**: `SchedulerError::SchedulerStopped` if scheduler is stopped
- **ERR-SCHEDULE-3**: `SchedulerError::QueueFull` if queue reached capacity

**Invariants:**
- **INV-SCHEDULE-1**: Job is persisted before returning `Ok`
- **INV-SCHEDULE-2**: `fire_at_ms` is computed and stored with job
- **INV-SCHEDULE-3**: Job priority and schedule determine queue ordering

**Given-When-Then Scenarios:**

| # | Given | When | Then |
|---|-------|------|------|
| 3.1.1 | Empty queue, valid job | `schedule_job(job)` | `Ok(job.id)`, job in queue |
| 3.1.2 | Queue at max capacity | `schedule_job(job)` | `Err(SchedulerError::QueueFull)` |
| 3.1.3 | Cron schedule | `schedule_job(job)` | `Err(SchedulerError::InvalidSchedule)` |
| 3.1.4 | Scheduler stopped | `schedule_job(job)` | `Err(SchedulerError::SchedulerStopped)` |
| 3.1.5 | One-shot schedule | `schedule_job(job)` | Job returns `None` on second poll |
| 3.1.6 | Interval schedule | `schedule_job(job)` | Job returns repeatedly on polls |

---

### 3.2 cancel_job

```rust
fn cancel_job(
    queue: &mut SchedulerQueue,
    job_id: JobId,
) -> Result<(), SchedulerError>
```

**Purpose:** Remove a scheduled job from the queue.

**Preconditions:**
- **PRE-CANCEL-1**: `job_id` identifies a job in the queue
- **PRE-CANCEL-2**: Job is not in terminal state (`Completed`, `Failed`, `Cancelled`)
- **PRE-CANCEL-3**: Job is not currently executing (no running workers)

**Postconditions:**
- **POST-CANCEL-1**: Job is removed from priority queue
- **POST-CANCEL-2**: Returns `Ok(())` if cancellation successful
- **POST-CANCEL-3**: Job state transitions to `Cancelled`

**Error Conditions:**
- **ERR-CANCEL-1**: `SchedulerError::JobNotFound` if job doesn't exist in queue
- **ERR-CANCEL-2**: `SchedulerError::InvalidTransition` if job already terminal

**Invariants:**
- **INV-CANCEL-1**: Cancellation is idempotent (multiple cancels safe)
- **INV-CANCEL-2**: No jobs are lost during cancellation
- **INV-CANCEL-3**: Queue ordering is preserved for remaining jobs

**Given-When-Then Scenarios:**

| # | Given | When | Then |
|---|-------|------|------|
| 3.2.1 | Job in queue | `cancel_job(job_id)` | `Ok(())`, job removed |
| 3.2.2 | Job not in queue | `cancel_job(job_id)` | `Err(SchedulerError::JobNotFound)` |
| 3.2.3 | Job already cancelled | `cancel_job(job_id)` | `Err(SchedulerError::InvalidTransition)` |
| 3.2.4 | Job due but not polled | `cancel_job(job_id)` | `Ok(())`, not in poll result |

---

### 3.3 get_job_status

```rust
fn get_job_status(
    queue: &SchedulerQueue,
    job_id: JobId,
) -> Result<JobState, SchedulerError>
```

**Purpose:** Query the current state of a job without modifying it.

**Preconditions:**
- **PRE-STATUS-1**: `job_id` may or may not exist in queue
- **PRE-STATUS-2**: No modification to queue state

**Postconditions:**
- **POST-STATUS-1**: Returns `Ok(JobState)` for existing job
- **POST-STATUS-2**: Returns `Err(SchedulerError::JobNotFound)` if job doesn't exist
- **POST-STATUS-3**: Does not modify job or queue

**Error Conditions:**
- **ERR-STATUS-1**: `SchedulerError::JobNotFound` if job not in queue

**Invariants:**
- **INV-STATUS-1**: Read-only operation (no state changes)
- **INV-STATUS-2**: Status reflects current queue state
- **INV-STATUS-3**: Thread-safe read access

**Given-When-Then Scenarios:**

| # | Given | When | Then |
|---|-------|------|------|
| 3.3.1 | Job in queue | `get_job_status(job_id)` | `Ok(Scheduled)` or `Ok(Pending)` |
| 3.3.2 | Job not in queue | `get_job_status(job_id)` | `Err(SchedulerError::JobNotFound)` |
| 3.3.3 | Multiple jobs | `get_job_status(id1)`, `get_job_status(id2)` | Each returns correct status |

---

### 3.4 update_job_schedule

```rust
fn update_job_schedule(
    queue: &mut SchedulerQueue,
    job_id: JobId,
    new_schedule: Schedule,
) -> Result<(), SchedulerError>
```

**Purpose:** Change the schedule policy of an existing job.

**Preconditions:**
- **PRE-UPDATE-1**: `job_id` identifies an existing job
- **PRE-UPDATE-2**: Job is in non-terminal state (`Scheduled` or `Pending`)
- **PRE-UPDATE-3**: `new_schedule` is valid (not `Cron`)
- **PRE-UPDATE-4**: Job is not currently executing

**Postconditions:**
- **POST-UPDATE-1**: Job's schedule is updated to `new_schedule`
- **POST-UPDATE-2**: Returns `Ok(())` if update successful
- **POST-UPDATE-3**: `fire_at_ms` is recomputed from new schedule
- **POST-UPDATE-4**: Job is re-ordered in priority queue if needed

**Error Conditions:**
- **ERR-UPDATE-1**: `SchedulerError::JobNotFound` if job doesn't exist
- **ERR-UPDATE-2**: `SchedulerError::InvalidTransition` if job is terminal
- **ERR-UPDATE-3**: `SchedulerError::InvalidSchedule` if new schedule is malformed

**Invariants:**
- **INV-UPDATE-1**: Only `Scheduled` or `Pending` jobs can be rescheduled
- **INV-UPDATE-2**: Rescheduling doesn't affect job execution state
- **INV-UPDATE-3**: Queue ordering is updated if fire time changed

**Given-When-Then Scenarios:**

| # | Given | When | Then |
|---|-------|------|------|
| 3.4.1 | Job in Scheduled state | `update_job_schedule(job_id, new_schedule)` | `Ok(())`, new fire time computed |
| 3.4.2 | Job in Running state | `update_job_schedule(job_id, new_schedule)` | `Err(SchedulerError::InvalidTransition)` |
| 3.4.3 | Job not in queue | `update_job_schedule(job_id, new_schedule)` | `Err(SchedulerError::JobNotFound)` |
| 3.4.4 | Cron schedule | `update_job_schedule(job_id, Cron(expr))` | `Err(SchedulerError::InvalidSchedule)` |

---

## 4. SchedulerQueue Data Structure (Go State 1.4)

### 4.1 PriorityQueue

```rust
pub struct PriorityQueue {
    jobs: BinaryHeap<JobEntry>,
}

struct JobEntry {
    job: Job,
    fire_at_ms: u64,
}

impl PartialOrd for JobEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Order by (priority ASC, fire_at_ms ASC)
        // Lower priority number = higher priority
        // Earlier fire time = higher priority
        Some(self.cmp(other))
    }
}
```

**Invariants:**
- **INV-PQ-1**: Queue is always ordered by `(priority ASC, fire_at_ms ASC)`
- **INV-PQ-2**: `BinaryHeap` ordering ensures highest priority job is at top
- **INV-PQ-3**: All operations are O(log n)

**Operations:**

#### `push(&mut self, job: Job, fire_at_ms: u64)`

**Preconditions:**
- **PRE-PUSH-1**: `job.id` is unique in queue
- **PRE-PUSH-2**: `fire_at_ms > 0`

**Postconditions:**
- **POST-PUSH-1**: Job is added to heap
- **POST-PUSH-2**: Heap ordering is maintained
- **POST-PUSH-3**: `len()` increases by 1

#### `pop(&mut self) -> Option<(Job, u64)>`

**Preconditions:**
- **PRE-POP-1**: Queue is not empty

**Postconditions:**
- **POST-POP-1**: Returns `Some((job, fire_at_ms))` with highest priority job
- **POST-POP-2**: Heap ordering is maintained
- **POST-POP-3**: `len()` decreases by 1

#### `remove(&mut self, job_id: &JobId) -> Option<Job>`

**Preconditions:**
- **PRE-REMOVE-1**: `job_id` exists in queue

**Postconditions:**
- **POST-REMOVE-1**: Returns `Some(job)` if found
- **POST-REMOVE-2**: Returns `None` if not found
- **POST-REMOVE-3**: Heap ordering is maintained

#### `due_jobs(&self, now_ms: u64, max: u32) -> Vec<(Job, u64)>`

**Preconditions:**
- **PRE-DUE-1**: `now_ms > 0`
- **PRE-DUE-2**: `max > 0`

**Postconditions:**
- **POST-DUE-1**: Returns jobs with `fire_at_ms <= now_ms`
- **POST-DUE-2**: Returns at most `max` jobs
- **POST-DUE-3**: Results are ordered by priority

#### `len(&self) -> usize`

**Preconditions:**
- None

**Postconditions:**
- **POST-LEN-1**: Returns current number of jobs in queue

#### `is_empty(&self) -> bool`

**Preconditions:**
- None

**Postconditions:**
- **POST-IS-EMPTY-1**: Returns `true` if `len() == 0`

**Given-When-Then Scenarios:**

| # | Given | When | Then |
|---|-------|------|------|
| 4.1.1 | Empty queue | `pop()` | `None` |
| 4.1.2 | Multiple priorities | `pop()` repeatedly | Critical before High before Normal before Low |
| 4.1.3 | Same priority, different times | `pop()` repeatedly | Earlier fire time first |
| 4.1.4 | All jobs due | `due_jobs(now, 100)` | All jobs returned (up to 100) |
| 4.1.5 | Some jobs due | `due_jobs(now, 100)` | Only due jobs returned |

---

## 5. State Machine Transitions (Go State 1.5)

### 5.1 JobState Variants

```rust
pub enum JobState {
    Scheduled,  // Job queued, not yet due
    Pending,    // Job due, waiting to be picked up
    Running,    // Job currently executing
    Completed,  // Job executed successfully (terminal)
    Failed,     // Job failed after all retries (terminal)
    Cancelled,  // Job was cancelled (terminal)
    Retrying,   // Job waiting for retry backoff
}
```

**State Machine:**
```
    ┌───────────┐   schedule    ┌───────────┐   due         ┌─────────┐   start      ┌─────────┐
    │ Scheduled │ ────────────► │  Pending  │ ────────────► │ Running │ ───────────► │Completed│
    └───────────┘               └───────────┘               └─────────┘              └─────────┘
         │                           │                          │                          │
         │ cancel                   │ cancel                   │ cancel                    │
         ▼                           ▼                          ▼                          │
    ┌───────────┐               ┌───────────┐              ┌─────────┐                   │
    │ Cancelled │               │ Cancelled │              │Failed/  │                   │
    └───────────┘               └───────────┘              │Retrying │                   │
                                                          └─────────┘                   │
                                                               │                         │
                                                               │ retry                    │
                                                               ▼                         │
                                                          ┌───────────┐                 │
                                                          │  Pending  │ ─────────────────┘
                                                          └───────────┘   (if recurring)
```

**Valid Transitions:**

| From | To | Trigger | Preconditions |
|------|-----|---------|---------------|
| - | Scheduled | `schedule_job()` | Job created |
| Scheduled | Pending | `due` | `now_ms >= fire_at_ms` |
| Pending | Running | `start` | Worker acquires permit |
| Running | Completed | `complete` | Job executed successfully |
| Running | Failed | `fail` | All retries exhausted |
| Running | Retrying | `retry` | Retry policy allows retry |
| Running | Cancelled | `cancel` | Cancellation requested |
| Scheduled | Cancelled | `cancel` | Cancellation requested |
| Pending | Cancelled | `cancel` | Cancellation requested |
| Retrying | Pending | `retry` | Backoff elapsed |
| Completed | Scheduled | `reschedule` | Job is recurring |

**State Invariants:**
- **INV-STATE-1**: Job is always in exactly one state
- **INV-STATE-2**: Terminal states (`Completed`, `Failed`, `Cancelled`) have no outgoing transitions
- **INV-STATE-3**: `Retrying` always has `attempt_count < max_retries`
- **INV-STATE-4**: `Running` implies worker has acquired permit

---

## 6. Contract Summary

### 6.1 Type Invariants Summary

| Type | Invariants |
|------|------------|
| `JobId` | Immutable, unique, serializable |
| `JobPriority` | Total ordering, default is Normal |
| `Schedule` | Valid fire times, cron returns None |
| `Job` | Required fields, builder pattern |
| `JobResult` | success/error consistency |
| `SchedulerConfig` | Valid defaults, positive values |

### 6.2 Error Classification Summary

| Category | Errors | Retryable |
|----------|--------|-----------|
| Resource | `QueueFull`, `SchedulerStopped`, `ConcurrencyLimitReached` | Partial |
| Permanent | `InvalidSchedule`, `JobNotFound`, `InvalidTransition` | No |
| Transient | `StorageError` | Yes |
| Execution | `Failed`, `ExceededRetries`, `Cancelled` | Depends |

### 6.3 API Operation Summary

| Operation | Precondition | Postcondition | Errors |
|-----------|--------------|---------------|--------|
| `schedule_job()` | Valid job, not stopped | Job added, returns Ok(id) | QueueFull, InvalidSchedule, SchedulerStopped |
| `cancel_job()` | Job exists, not terminal | Job removed | JobNotFound, InvalidTransition |
| `get_job_status()` | Job may exist | Returns state or NotFound | JobNotFound |
| `update_job_schedule()` | Job in Scheduled/Pending | Schedule updated | JobNotFound, InvalidTransition, InvalidSchedule |

---

## 7. References

- **Contract:** `docs/adr/v2/ADR-047-v2-background-job-scheduler-contract.md`
- **Implementation:** `crates/vo-executor/src/scheduler/`
  - `types.rs` - Job, JobId, Schedule, JobPriority
  - `error.rs` - SchedulerError, JobRunError
  - `queue.rs` - PriorityQueue
  - `mod.rs` - Scheduler API
- **Bead:** ve-ewskc (this document)
- **Discovered from:** ve-i1vuu (vo-scheduler: Implement Background Job Scheduler - ADR-047)
