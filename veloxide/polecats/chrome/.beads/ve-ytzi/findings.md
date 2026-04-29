# BLACKHAT QA Audit: ve-ytzi ADR-047 Observability Metrics

## Issue
ADR-047 section 8 requires 8 specific metrics to be emitted by vo-scheduler. Issue claimed metrics were "completely absent".

## Findings

### What EXISTS (NOT completely absent as title claims)
- `crates/vo-scheduler/src/metrics.rs` - Defines all 8 required metrics per ADR-047 §8:
  - `jobs_scheduled_total` (Counter)
  - `jobs_completed_total` (Counter)
  - `jobs_failed_total` (Counter)
  - `jobs_cancelled_total` (Counter)
  - `jobs_retried_total` (Counter)
  - `queue_depth` (Gauge)
  - `job_execution_duration_seconds` (Histogram)
  - `job_retry_delay_seconds` (Histogram)

- `crates/vo-scheduler/src/api.rs` - API functions that accept metrics and record them:
  - `schedule_job()` - increments scheduled + sets queue_depth
  - `cancel_job()` - increments cancelled + sets queue_depth
  - `record_job_completed()` - increments completed
  - `record_job_failed()` - increments failed
  - `record_job_retried()` - increments retried
  - `record_execution_duration()` - records execution time
  - `record_retry_delay()` - records retry delay

### What WAS Missing (Actual bug)
The `SchedulerQueue` struct in `queue.rs` did NOT hold or automatically record metrics.

**Before:**
```rust
pub struct SchedulerQueue {
    heap: BinaryHeap<QueueEntry>,
    jobs: HashMap<JobId, ScheduledJob>,
    capacity: usize,
}
```

**Missing:**
- No `metrics: Option<Arc<SchedulerMetrics>>` field
- No `with_metrics()` builder method
- Queue operations didn't automatically record metrics
- Metrics had to be explicitly passed to API functions (leaky abstraction)

### Impact
ADR-047 §8 states scheduler "MUST emit telemetry". Original implementation:
1. Required callers to explicitly pass metrics to API functions
2. Direct `SchedulerQueue` users (without going through API) got zero observability
3. Metrics recording was opt-in rather than automatic

## FIX IMPLEMENTED

### Changes to `crates/vo-scheduler/src/queue.rs`:

1. Added imports:
```rust
use std::sync::Arc;
use crate::metrics::SchedulerMetrics;
```

2. Added metrics field and manual Debug impl:
```rust
pub struct SchedulerQueue {
    heap: BinaryHeap<QueueEntry>,
    jobs: HashMap<JobId, ScheduledJob>,
    capacity: usize,
    metrics: Option<Arc<SchedulerMetrics>>,
}
```

3. Added builder method:
```rust
pub fn with_metrics(mut self, metrics: Arc<SchedulerMetrics>) -> Self {
    self.metrics = Some(metrics);
    self
}
```

4. Modified `insert()` to record metrics:
```rust
if let Some(ref m) = self.metrics {
    m.increment_scheduled();
    m.set_queue_depth(self.jobs.len());
}
```

5. Modified `remove()` to update queue_depth:
```rust
if let Some(ref m) = self.metrics {
    m.set_queue_depth(self.jobs.len());
}
```

6. Modified `cancel()` to record metrics:
```rust
if let Some(ref m) = self.metrics {
    m.increment_cancelled();
    m.set_queue_depth(self.jobs.len());
}
```

### Metrics NOT Wired in Queue (handled by caller via api.rs)
- `jobs_completed_total` - caller calls `record_job_completed()`
- `jobs_failed_total` - caller calls `record_job_failed()`
- `jobs_retried_total` - caller calls `record_job_retried()`
- `job_execution_duration_seconds` - caller calls `record_execution_duration()`
- `job_retry_delay_seconds` - caller calls `record_retry_delay()`

These require execution context (duration, retry count) that the queue doesn't have.

## Verification
- `cargo build -p vo-scheduler` - COMPILES
- `cargo test -p vo-scheduler` - 66 PASSED (4 suites)
- Metrics wired up in core queue operations
- Builder pattern allows optional metrics injection

## Conclusion
**Status: FIXED** - Metrics infrastructure existed but was not integrated into core queue. Fix adds metrics field to SchedulerQueue with builder pattern and automatic recording on insert/remove/cancel operations. The issue title "completely absent" was inaccurate - metrics were present, just not wired up.
