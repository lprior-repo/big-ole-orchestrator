# ADR 047: Background Job Scheduler Contract

## Status

Proposed

## Context

The `vo-scheduler` crate manages background job scheduling for workflow instances. It provides delayed execution, retry semantics, and priority-based queue management. Without a formal contract:

1. Job state transitions are not formally specified
2. Error handling semantics are ambiguous
3. Retry behavior under failures is undefined
4. Relationship between job scheduler and lifecycle state is unclear

This ADR defines the canonical runtime contract for the background job scheduler.

## Decision

### 1. Core Types

#### 1.1 JobId

```rust
pub struct JobId(pub Ulid);
```

A unique identifier for a scheduled job. Uses ULID for sortable uniqueness.

#### 1.2 JobState (7 variants)

```rust
pub enum JobState {
    /// Job is scheduled but not yet due
    Scheduled,
    /// Job is due and waiting to be picked up
    Pending,
    /// Job is currently executing
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed after all retries exhausted
    Failed,
    /// Job was cancelled explicitly
    Cancelled,
    /// Job is waiting for scheduled retry
    Retrying,
}
```

#### 1.3 JobKind (3 variants)

```rust
pub enum JobKind {
    /// One-time job that executes once and completes
    OneShot,
    /// Recurring job that reschedules itself after completion
    Recurring,
    /// Delayed job that executes once after a delay
    Delayed,
}
```

#### 1.4 SchedulePolicy

```rust
pub enum SchedulePolicy {
    /// Execute at a specific timestamp
    At(DateTime<Utc>),
    /// Execute after a duration from now
    After(Duration),
    /// Execute with a cron-like expression
    Cron(String),
    /// Execute immediately when due (default)
    Immediate,
}
```

#### 1.5 RetryPolicy

```rust
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}
```

#### 1.6 JobPriority

```rust
#[repr(u8)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}
```

### 2. State Machine

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

#### 2.1 Transition Events

| Event | From State | To State | Condition |
|-------|------------|----------|-----------|
| `schedule` | - | Scheduled | Job created with future due time |
| `due` | Scheduled | Pending | Current time >= due time |
| `start` | Pending | Running | Worker picks up job |
| `complete` | Running | Completed | Job executed successfully |
| `fail` | Running | Failed | Job failed with exhausted retries |
| `retry` | Failed | Retrying | Retry policy allows retry |
| `cancel` | Scheduled, Pending, Running, Retrying | Cancelled | Explicit cancellation |
| `reschedule` | Completed (Recurring) | Scheduled | Job reschedules itself |

#### 2.2 State Invariants

1. **Phase Atomicity**: A `ScheduledJob` is in exactly one state at any time.
2. **ID Persistence**: `JobId` is immutable once assigned.
3. **Attempt Monotonicity**: `attempt_count` is monotonically increasing.
4. **Due Time Validity**: `due_at` is always in the future when state is `Scheduled`.

### 3. Data Structures

#### 3.1 ScheduledJob (Persisted)

```rust
pub struct ScheduledJob {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    pub priority: JobPriority,
    pub schedule_policy: SchedulePolicy,
    pub retry_policy: RetryPolicy,
    pub attempt_count: u32,
    pub due_at: DateTime<Utc>,
    pub payload: SerializedPayload,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

#### 3.2 SchedulerQueue (In-Memory)

```rust
pub struct SchedulerQueue {
    jobs: PriorityQueue<JobId, (JobPriority, DateTime<Utc>)>,
    by_id: HashMap<JobId, JobState>,
}
```

The queue uses a priority queue ordered by `(JobPriority, due_at)` where lower priority number comes first, and earlier due times come first for equal priorities.

### 4. Error Taxonomy

#### 4.1 SchedulerError

Errors in scheduler configuration or operations:

| Error | Description |
|-------|-------------|
| `QueueFull` | Scheduler queue has reached capacity |
| `InvalidSchedule` | Schedule policy is malformed |
| `JobNotFound` | Requested job does not exist |
| `InvalidTransition` | State transition is not allowed |
| `SerializationError` | Failed to serialize/deserialize job payload |

#### 4.2 ExecutionError

Errors during job execution:

| Error | Description |
|-------|-------------|
| `Panicked` | Job task panicked during execution |
| `TimedOut` | Job exceeded its time limit |
| `Cancelled` | Job was cancelled during execution |
| `ResourceExhausted` | Job exhausted available resources |

#### 4.3 RetryExhaustedError

Retry policy errors:

| Error | Description |
|-------|-------------|
| `MaxAttemptsReached` | All retry attempts exhausted |
| `BackoffOverflow` | Backoff calculation overflowed |
| `RetryNotAllowed` | Job kind does not support retries |

#### 4.4 Error Classification

| Category | Errors | Retry Behavior |
|----------|--------|----------------|
| Transient | `SerializationError`, `ResourceExhausted` | Retryable |
| Permanent | `InvalidSchedule`, `InvalidTransition` | Non-retryable |
| Execution | `Panicked`, `TimedOut`, `Cancelled` | Depends on retry policy |
| Exhaustion | `MaxAttemptsReached`, `BackoffOverflow` | Terminal |

### 5. API Operations

#### 5.1 ScheduleJob

```rust
async fn schedule_job(
    queue: &SchedulerQueue,
    job: ScheduledJob,
) -> Result<JobId, SchedulerError>
```

**Contract:**
- Returns `Ok(JobId)` with the assigned job ID
- Job starts in `Scheduled` state
- Job is persisted to storage before returning
- If `schedule_policy` is `At(past_time)` or `After(0)`, job transitions immediately to `Pending`

#### 5.2 CancelJob

```rust
async fn cancel_job(
    queue: &SchedulerQueue,
    job_id: JobId,
) -> Result<(), SchedulerError>
```

**Contract:**
- Returns `Ok(())` if job was cancelled
- Returns `Err(SchedulerError::JobNotFound)` if job doesn't exist
- Returns `Err(SchedulerError::InvalidTransition)` if job is in terminal state (`Completed`, `Failed`, `Cancelled`)
- Job transitions to `Cancelled` if found and not terminal

#### 5.3 GetJobStatus

```rust
async fn get_job_status(
    job_id: JobId,
) -> Result<JobState, SchedulerError>
```

**Contract:**
- Returns `Ok(JobState)` for existing job
- Returns `Err(SchedulerError::JobNotFound)` if job doesn't exist
- Does not modify job state

#### 5.4 UpdateJobSchedule

```rust
async fn update_job_schedule(
    queue: &SchedulerQueue,
    job_id: JobId,
    new_schedule: SchedulePolicy,
) -> Result<(), SchedulerError>
```

**Contract:**
- Returns `Ok(())` if schedule was updated
- Only allowed in `Scheduled` or `Pending` states
- Returns `Err(SchedulerError::InvalidTransition)` if job is running, completed, or failed
- Updates `due_at` based on new `SchedulePolicy`

### 6. Integration with LifecycleState

Jobs integrate with `LifecycleState` from `vo-types` as follows:

| JobState | LifecycleState | Notes |
|----------|----------------|-------|
| Scheduled | `Pending` | Job queued, not yet due |
| Pending | `StepScheduled` | Job is due and waiting |
| Running | `StepExecuting` | Job actively executing |
| Completed | `Completed` | Terminal success state |
| Failed | `Failed` | Terminal failure state |
| Cancelled | `Cancelled` | Terminal cancellation state |
| Retrying | `WaitingForTimer` | Waiting to retry |

#### 6.1 State Synchronization

- When job enters `Running` state, corresponding `LifecycleState` MUST be `StepExecuting`
- When job completes or fails, corresponding `LifecycleState` MUST transition to `Completed` or `Failed`
- The scheduler MUST update lifecycle state within the same transaction as job state

### 7. Invariants

#### 7.1 Type Invariants

1. `JobId` is always valid ULID
2. `JobPriority` is in range 0-4
3. `RetryPolicy.max_attempts` is > 0
4. `RetryPolicy.backoff_multiplier` is >= 1.0
5. `RetryPolicy.initial_delay` is > 0

#### 7.2 State Invariants

1. Non-terminal states: `Scheduled`, `Pending`, `Running`, `Retrying`
2. Terminal states: `Completed`, `Failed`, `Cancelled`
3. Only `Recurring` jobs can transition `Completed` -> `Scheduled`
4. `Retrying` jobs always have `attempt_count < retry_policy.max_attempts`

#### 7.3 Consistency Invariants

1. `due_at` is monotonically increasing across retries
2. `attempt_count` is incremented atomically with state transition
3. Job is always persisted before state transition completes
4. `last_error` is `Some` if and only if previous state was `Failed` or `Retrying`

### 8. Observability

The scheduler MUST emit telemetry for:

| Metric | Description |
|--------|-------------|
| `jobs_scheduled_total` | Counter of jobs scheduled |
| `jobs_completed_total` | Counter of jobs completed |
| `jobs_failed_total` | Counter of jobs failed |
| `jobs_cancelled_total` | Counter of jobs cancelled |
| `jobs_retried_total` | Counter of job retries |
| `queue_depth` | Current number of jobs in queue |
| `job_execution_duration_seconds` | Histogram of job execution times |
| `job_retry_delay_seconds` | Histogram of retry delays |

### 9. Cancellation Safety

The scheduler loop is cancellation-safe:

- `cancel_job()` waits for in-flight job to complete cancellation before returning
- If job is `Running` when cancelled, cancellation is requested but job may complete
- No state is lost if scheduler task is cancelled

### 10. Send+Sync Requirements

All scheduler state (`SchedulerQueue`, `ScheduledJob`) MUST be `Send + Sync` to support multi-threaded tokio runtime.

## Consequences

### Positive

- Job state transitions become deterministic and testable
- Retry behavior is formally specified
- Error handling becomes exhaustive and categorized
- Integration with lifecycle state is clearly defined

### Negative

- Contract may restrict scheduling optimization opportunities
- Additional ceremony for implementing new scheduler backend

## References

- Implementation: `crates/vo-scheduler/`
- Related ADR: ADR-039 (Lifecycle Superstate)
- Related ADR: ADR-046 (Async Process Supervisor Contract)