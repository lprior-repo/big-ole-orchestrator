# BLACKHAT: ADR-047 Observability Metrics Implementation - ve-ytzi

## Issue
ADR-047 section 8 requires 8 specific metrics for vo-scheduler. The issue reported zero metric/telemetry references existed in vo-scheduler.

## ADR-047 Section 8 Required Metrics
| Metric | Type | Description |
|--------|------|-------------|
| `jobs_scheduled_total` | Counter | Jobs scheduled |
| `jobs_completed_total` | Counter | Jobs completed |
| `jobs_failed_total` | Counter | Jobs failed |
| `jobs_cancelled_total` | Counter | Jobs cancelled |
| `jobs_retried_total` | Counter | Job retries |
| `queue_depth` | Gauge | Current jobs in queue |
| `job_execution_duration_seconds` | Histogram | Job execution times |
| `job_retry_delay_seconds` | Histogram | Retry delays |

## Implementation

### Changes Made

1. **Added `metrics` dependency** to `crates/vo-scheduler/Cargo.toml`
   - Added `metrics.workspace = true`

2. **Created `crates/vo-scheduler/src/metrics.rs`** with 8 metric functions:
   - `jobs_scheduled_total()` - counter for scheduled jobs
   - `jobs_completed_total()` - counter for completed jobs
   - `jobs_failed_total()` - counter for failed jobs
   - `jobs_cancelled_total()` - counter for cancelled jobs
   - `jobs_retried_total()` - counter for job retries
   - `set_queue_depth(depth)` - gauge for queue depth
   - `record_job_execution_duration(duration_secs)` - histogram for execution time
   - `record_job_retry_delay(delay_secs)` - histogram for retry delay

3. **Updated `crates/vo-scheduler/src/lib.rs`**
   - Added `pub mod metrics;` export

4. **Updated `crates/vo-scheduler/src/queue.rs`** to emit metrics:
   - `insert()`: emits `jobs_scheduled_total` and `set_queue_depth`
   - `remove()`: emits `set_queue_depth`
   - `update_state()`: emits appropriate counter based on state transition

### Metric Naming Convention
All metrics use the `vo_scheduler.` prefix following the pattern observed in `vo_storage` crate:
- `vo_scheduler.jobs_scheduled_total`
- `vo_scheduler.jobs_completed_total`
- `vo_scheduler.jobs_failed_total`
- `vo_scheduler.jobs_cancelled_total`
- `vo_scheduler.jobs_retried_total`
- `vo_scheduler.queue_depth`
- `vo_scheduler.job_execution_duration_seconds`
- `vo_scheduler.job_retry_delay_seconds`

### Build Verification
- Library compiles without errors or warnings
- Clippy passes with no issues in vo-scheduler

## Notes
- The histogram functions (`record_job_execution_duration`, `record_job_retry_delay`) are implemented but require callers to pass the duration values. The actual timing tracking would need to be implemented at the runtime level that uses vo-scheduler, not in the data structure layer.
- Metrics are emitted using the `metrics` crate (v0.24) following the existing pattern in vo-storage.