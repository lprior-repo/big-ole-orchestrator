# ADR 047: Background Job Scheduler Contract

## Status

Accepted

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

#### 6.1 State Synchronization Protocol

The scheduler MUST update lifecycle state within the same transaction as job state (atomically).

**Transition Sequence for Running Jobs:**

1. Worker picks up job from queue
2. Scheduler atomically:
   - Updates job state: `Pending` → `Running`
   - Updates lifecycle state: `StepScheduled` → `StepExecuting`
   - Persists both state changes
3. If persistence fails, both states MUST roll back to previous values

**Transition Sequence for Completed/Failed Jobs:**

1. Job execution completes (success or failure)
2. Scheduler atomically:
   - Updates job state to `Completed` or `Failed`
   - Updates lifecycle state to `Completed` or `Failed`
   - Records `last_error` if applicable
   - Persists all changes
3. If persistence fails, the job remains in `Running` state and MUST be recovered via reconciliation

**Cancellation Synchronization:**

- When `cancel_job()` is called on a non-terminal job:
  - Job state transitions to `Cancelled`
  - Lifecycle state transitions to `Cancelled` within same transaction
- For `Running` jobs: cancellation is requested asynchronously; lifecycle state transitions to `Cancelled` only after job execution stops

#### 6.2 Transactional Requirements

All state transitions affecting both `JobState` and `LifecycleState` MUST be:

1. **Atomic**: Both states update or neither does
2. **Consistent**: Invariants hold before and after
3. **Isolated**: Concurrent transitions do not interfere
4. **Durable**: Persisted before acknowledgment

**Implementation note**: Use a single transaction spanning both job persistence and lifecycle state updates. If the underlying storage does not support multi-document transactions, use the Saga pattern with compensating transactions.

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

The scheduler MUST emit telemetry for all operations using the OpenTelemetry metrics API.

#### 8.1 Metric Specifications

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `jobs_scheduled_total` | Counter | `job_kind`, `priority` | Jobs added to scheduler |
| `jobs_completed_total` | Counter | `job_kind`, `priority` | Jobs completed successfully |
| `jobs_failed_total` | Counter | `job_kind`, `priority`, `error_type` | Jobs failed after retries |
| `jobs_cancelled_total` | Counter | `job_kind`, `priority` | Jobs cancelled explicitly |
| `jobs_retried_total` | Counter | `job_kind`, `priority`, `attempt` | Retry attempts triggered |
| `queue_depth` | Gauge | `state` | Current jobs per state |
| `job_execution_duration_seconds` | Histogram | `job_kind`, `priority` | Job execution time (see buckets) |
| `job_retry_delay_seconds` | Histogram | `job_kind`, `priority` | Delay between retry attempts |

#### 8.2 Histogram Buckets

**`job_execution_duration_seconds`:**
```
[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
```

**`job_retry_delay_seconds`:**
```
[0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]
```

#### 8.3 Error Classification Labels

`error_type` label values for `jobs_failed_total`:

| Error | Label Value |
|-------|-------------|
| `Panicked` | `panicked` |
| `TimedOut` | `timed_out` |
| `ResourceExhausted` | `resource_exhausted` |
| `MaxAttemptsReached` | `max_attempts_reached` |
| `BackoffOverflow` | `backoff_overflow` |
| `SerializationError` | `serialization_error` |

#### 8.4 Trace Spans

For each job lifecycle transition, emit a trace span with:

- **Span name**: `job.{transition}` (e.g., `job.schedule`, `job.complete`, `job.fail`)
- **Attributes**:
  - `job.id`: ULID of the job
  - `job.kind`: One of `one_shot`, `recurring`, `delayed`
  - `job.priority`: One of `critical`, `high`, `normal`, `low`, `background`
  - `job.state`: State before transition
  - `job.attempt`: Current attempt count

#### 8.5 Queue Depth Gauge Labels

`queue_depth` gauge MUST report with `state` label:

| JobState | State Label |
|----------|-------------|
| Scheduled | `scheduled` |
| Pending | `pending` |
| Running | `running` |
| Completed | `completed` |
| Failed | `failed` |
| Cancelled | `cancelled` |
| Retrying | `retrying` |

#### 8.6 Metric Emission Points

| Operation | Metrics to Emit |
|-----------|-----------------|
| `schedule_job()` | `jobs_scheduled_total` + trace span |
| Job starts executing | `queue_depth` (pending → running) |
| Job completes | `jobs_completed_total`, `job_execution_duration_seconds` + trace span |
| Job fails | `jobs_failed_total` + trace span |
| Retry triggered | `jobs_retried_total`, `job_retry_delay_seconds` |
| `cancel_job()` | `jobs_cancelled_total` |
| Queue poll | `queue_depth` (periodic gauge update) |

### 9. Cancellation Safety

The scheduler implements a multi-phase cancellation protocol that ensures no state loss.

#### 9.1 Cancellation Protocol

**Phase 1: Cancellation Request**

When `cancel_job()` is called:

1. Validate job exists and is not in terminal state (`Completed`, `Failed`, `Cancelled`)
2. Record cancellation intent in job metadata
3. Transition job state to `Cancelled` atomically with lifecycle state
4. Return `Ok(())` to caller

**Phase 2: In-Flight Job Handling (for Running jobs)**

If job is `Running` when cancelled:

1. Set cancellation flag in job context (checked by worker)
2. Worker MAY complete execution before checking cancellation flag
3. If worker completes first: job ends in `Completed`, lifecycle state updated
4. If cancellation checked first: job ends in `Cancelled`, lifecycle state updated
5. The scheduler loop MUST NOT lose track of the job during this window

**Phase 3: Draining (optional blocking mode)**

`cancel_job()` supports an optional `drain: bool` parameter:

- `drain: false` (default): Returns immediately after Phase 1
- `drain: true`: Waits until job execution fully stops (either completed or cancellation took effect)

#### 9.2 Scheduler Task Cancellation Safety

If the scheduler task itself is cancelled (e.g., tokio task cancellation):

1. All in-memory state MUST be flushed to persistence before cancellation completes
2. The scheduler MUST use structured concurrency (e.g., `JoinSet`) to track spawned workers
3. On restart, scheduler MUST reconcile in-memory state with persisted state
4. No job state is lost - any job not fully persisted is recovered from last known state

#### 9.3 Cancellation Invariants

1. **No orphaned jobs**: Every non-terminal job is either running or queued
2. **Idempotent cancellation**: Calling `cancel_job()` multiple times on same job returns `Ok(())` on first call, `Err(SchedulerError::InvalidTransition)` on subsequent calls
3. **Terminal states are final**: Cancellation MUST NOT transition `Completed`, `Failed`, or already `Cancelled` jobs
4. **Cancellation is cooperative**: Running jobs check cancellation flag and yield control if set

#### 9.4 Implementation Requirements

- Cancellation flag MUST be stored in job context accessible by worker
- Worker MUST check cancellation flag at:
  - Loop boundaries
  - Await points
  - Before long operations
- Scheduler MUST provide `cancel_with_drain()` async function that awaits job termination
- On scheduler restart, reconciliation MUST:
  1. Load all jobs from persistence
  2. For `Running` jobs without active workers: transition to `Pending` for retry or `Failed` based on retry policy

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