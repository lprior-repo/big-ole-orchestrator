# Test Plan: Background Job Scheduler

**Bead:** ve-7tj8
**Contract:** ADR-047-v2 (Background job scheduler)
**Implementation:** `crates/vo-executor/src/scheduler/`

## Testing Trophy Distribution

- **Unit Tests (75%):** Types, Schedule behavior, PriorityQueue ordering, error taxonomy
- **Integration Tests (20%):** Scheduler API, concurrency limits, job lifecycle
- **Property Tests (5%):** Fire time monotonicity, priority ordering invariants

---

## 1. Unit Tests: JobPriority Enum (5 tests)

| # | Test Name | What | Invariant |
|---|-----------|------|-----------|
| 1.1 | `job_priority_ordering` | Critical < High < Normal < Low | Lower enum = higher priority |
| 1.2 | `job_priority_default_is_normal` | Default priority is Normal | Consistent initialization |
| 1.3 | `job_priority_serialization_roundtrip` | Serialize + deserialize = original | Serde compatibility |
| 1.4 | `job_priority_all_variants_present` | All 4 variants constructable | Exhaustive match |
| 1.5 | `job_priority_debug_format` | format!("{:?}", priority) works | Debug output |

## 2. Unit Tests: Schedule Enum (10 tests)

| # | Test Name | What | Contract |
|---|-----------|------|----------|
| 2.1 | `schedule_cron_creation` | Cron(String) stores expression | Expression preserved |
| 2.2 | `schedule_cron_next_fire_returns_none` | Cron.next_fire_time always None | Not yet implemented |
| 2.3 | `schedule_one_shot_creation` | one_shot(delay) sets fire_at_ms | Future timestamp |
| 2.4 | `schedule_one_shot_next_fire_first_call` | first call with last_fire_ms=0 returns Some | Initial fire |
| 2.5 | `schedule_one_shot_next_fire_second_call` | second call with last_fire_ms!=0 returns None | One-shot semantics |
| 2.6 | `schedule_interval_creation` | interval(delay) stores interval_ms | Interval preserved |
| 2.7 | `schedule_interval_next_fire_first` | first call returns now + interval | Initial fire |
| 2.8 | `schedule_interval_next_fire_subsequent` | subsequent calls return last + interval | Monotonic |
| 2.9 | `schedule_interval_no_overflow` | saturating_add prevents overflow at u64::MAX | Safety |
| 2.10 | `schedule_serialization_roundtrip` | All variants survive serde | Persistence |

## 3. Unit Tests: Job Type (8 tests)

| # | Test Name | What | Contract |
|---|-----------|------|----------|
| 3.1 | `job_new_sets_all_fields` | Job::new sets id, payload, schedule | Required fields |
| 3.2 | `job_default_priority_is_normal` | Default priority = JobPriority::Normal | Default value |
| 3.3 | `job_default_retries_is_3` | Default max_retries = 3 | Default retry budget |
| 3.4 | `job_default_backoff_is_1000ms` | Default backoff_ms = 1000 | Default backoff |
| 3.5 | `job_with_priority` | with_priority() builder sets priority | Builder pattern |
| 3.6 | `job_with_retries` | with_retries() sets max_retries and backoff_ms | Builder pattern |
| 3.7 | `job_serialization_roundtrip` | Serialize + deserialize = original | Serde compatibility |
| 3.8 | `job_payload_is_string` | payload field is String type | ADR-047 vs earlier version |

## 4. Unit Tests: JobId Type (5 tests)

| # | Test Name | What | Invariant |
|---|-----------|------|-----------|
| 4.1 | `job_id_new_constructs` | JobId::new(u64) creates instance | Constructor works |
| 4.2 | `job_id_equality` | Two JobIds with same u64 are equal | Eq implement |
| 4.3 | `job_id_hash` | JobId in HashMap works | Hashable |
| 4.4 | `job_id_display` | format!("{}", job_id) outputs "job-{n}" | Display impl |
| 4.5 | `job_id_debug` | format!("{:?}", job_id) works | Debug impl |

## 5. Unit Tests: JobResult Type (4 tests)

| # | Test Name | What | Fields |
|---|-----------|------|--------|
| 5.1 | `job_result_has_all_fields` | job_id, success, output, error, attempt | All fields present |
| 5.2 | `job_result_success_true` | success=true, error=None | Success case |
| 5.3 | `job_result_failure_false` | success=false, error=Some | Failure case |
| 5.4 | `job_result_serialization_roundtrip` | Roundtrip through serde | Persistence |

## 6. Unit Tests: SchedulerConfig (4 tests)

| # | Test Name | What | Contract |
|---|-----------|------|----------|
| 6.1 | `scheduler_config_default_values` | Default max_concurrent=10, scan_interval=100ms, max_jobs_per_scan=100 | ADR-047 defaults |
| 6.2 | `scheduler_config_custom_values` | Custom config accepted | User configuration |
| 6.3 | `scheduler_config_serialization_roundtrip` | Roundtrip through serde | Persistence |
| 6.4 | `scheduler_config_debug` | format!("{:?}", config) works | Debug |

## 7. Unit Tests: SchedulerError Taxonomy (6 tests)

| # | Test Name | Error Variant | Fields |
|---|-----------|---------------|--------|
| 7.1 | `scheduler_error_job_not_found` | JobNotFound(JobId) | Has JobId |
| 7.2 | `scheduler_error_queue_full` | QueueFull | Unit variant |
| 7.3 | `scheduler_error_scheduler_stopped` | SchedulerStopped | Unit variant |
| 7.4 | `scheduler_error_invalid_schedule` | InvalidSchedule(String) | Has reason string |
| 7.5 | `scheduler_error_concurrency_limit_reached` | ConcurrencyLimitReached | Unit variant |
| 7.6 | `scheduler_error_storage_error` | StorageError(String) | Has message |

## 8. Unit Tests: JobRunError Taxonomy (3 tests)

| # | Test Name | Error Variant | Fields |
|---|-----------|---------------|--------|
| 8.1 | `job_run_error_failed` | Failed{job_id, reason} | Both fields |
| 8.2 | `job_run_error_exceeded_retries` | ExceededRetries{job_id, attempts} | Both fields |
| 8.3 | `job_run_error_cancelled` | Cancelled{job_id} | Has job_id |

## 9. Unit Tests: PriorityQueue Ordering (6 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 9.1 | `priority_queue_critical_before_high` | Critical job at same fire time as High | Critical first |
| 9.2 | `priority_queue_high_before_normal` | High before Normal at same time | High first |
| 9.3 | `priority_queue_normal_before_low` | Normal before Low at same time | Normal first |
| 9.4 | `priority_queue_same_priority_earlier_first` | Same priority, earlier fire time | Earlier first |
| 9.5 | `priority_queue_pop_ordering` | Jobs with mixed priority and times | Correct ordering |
| 9.6 | `priority_queue_peek_does_not_remove` | peek() leaves job in queue | Non-destructive |

## 10. Unit Tests: PriorityQueue Operations (6 tests)

| # | Test Name | What | Expected |
|---|-----------|------|----------|
| 10.1 | `priority_queue_push_increases_len` | After push, len() = old_len + 1 | Size tracking |
| 10.2 | `priority_queue_pop_returns_job` | Pop returns (Job, fire_at_ms) | Correct return type |
| 10.3 | `priority_queue_pop_empty_returns_none` | Pop on empty queue | None |
| 10.4 | `priority_queue_remove_existing` | Remove job by JobId | Returns job, queue size -1 |
| 10.5 | `priority_queue_remove_nonexistent` | Remove non-existent job | None, size unchanged |
| 10.6 | `priority_queue_due_jobs_filters_time` | due_jobs(now_ms, max) returns jobs with fire_at_ms <= now_ms | Time filtering |
| 10.7 | `priority_queue_due_jobs_respects_max` | due_jobs with max=2 returns at most 2 | Limit respected |
| 10.8 | `priority_queue_due_jobs_returns_fire_time` | due_jobs returns (Job, fire_at_ms) pairs | Correct return type |
| 10.9 | `priority_queue_into_vec` | into_vec() drains queue | All jobs returned |

## 11. Integration Tests: Scheduler Lifecycle (8 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 11.1 | `scheduler_new_sets_config` | Scheduler::new(config) stores config | Config preserved |
| 11.2 | `scheduler_schedule_one_shot` | Schedule OneShot job, poll when due | Job returned |
| 11.3 | `scheduler_schedule_multiple` | Schedule 5 jobs | All 5 scheduled |
| 11.4 | `scheduler_cancel_existing` | Cancel scheduled job | Job removed |
| 11.5 | `scheduler_cancel_nonexistent` | Cancel unknown job | None returned |
| 11.6 | `scheduler_poll_due_jobs_empty` | Poll when nothing due | Empty vec |
| 11.7 | `scheduler_poll_due_jobs_respects_max` | Poll with max=1 returns 1 | Limit respected |
| 11.8 | `scheduler_reschedule_job` | Reschedule a job after execution | Job re-queued |

## 12. Integration Tests: Concurrency Control (4 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 12.1 | `scheduler_concurrency_limit_respected` | max_concurrent=2, acquire 3 permits | Third acquire fails |
| 12.2 | `scheduler_try_acquire_success` | Under limit | Some(permit) |
| 12.3 | `scheduler_try_acquire_failure` | At limit | None |
| 12.4 | `scheduler_start_stop` | start() sets running=true, stop() sets false | State toggles |

## 13. Integration Tests: Scheduler State Machine (5 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|------|-----------|
| 13.1 | `scheduler_schedule_and_poll_complete` | Schedule → due → poll → complete | Full lifecycle |
| 13.2 | `scheduler_cancel_before_due` | Cancel before poll returns | Job not in poll result |
| 13.3 | `scheduler_reschedule_recurring` | Interval job polled, rescheduled | Continuous execution |
| 13.4 | `scheduler_reschedule_after_cancel` | Cancel removes job, reschedule re-adds | Independent operations |
| 13.5 | `scheduler_len_and_empty` | len() = 0 when empty, > 0 when jobs | Accurate count |

## 14. Edge Cases & Boundary Tests (10 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 14.1 | `schedule_one_shot_zero_delay` | one_shot(Duration::ZERO) | fire_at_ms is now |
| 14.2 | `schedule_interval_zero_interval` | interval(Duration::ZERO) | interval_ms = 0 |
| 14.3 | `job_priority_extremes` | Critical and Low ordering | Correct |
| 14.4 | `priority_queue_max_u64_fire_time` | fire_at_ms = u64::MAX | No overflow |
| 14.5 | `priority_queue_due_jobs_none_due` | All jobs in future | Empty vec |
| 14.6 | `priority_queue_due_jobs_all_due` | All jobs in past | All returned (up to max) |
| 14.7 | `scheduler_config_zero_max_concurrent` | max_concurrent=0 | Semaphore of 0 permits |
| 14.8 | `scheduler_config_zero_scan_interval` | scan_interval=0 | Zero duration |
| 14.9 | `job_empty_payload` | payload = String::new() | Valid |
| 14.10 | `job_large_payload` | payload = "x".repeat(1_000_000) | Valid |

## 15. Property Tests — proptest (4 tests)

| # | Property | Strategy | Invariant |
|---|----------|----------|-----------|
| 15.1 | `priority_ordering_transitive` | Random pairs of priorities | Ordering is transitive |
| 15.2 | `schedule_fire_time_always_future` | OneShot with random delay | fire_at_ms > now |
| 15.3 | `interval_fire_times_monotonic` | Random interval schedule | next_fire always increases |
| 15.4 | `priority_queue_ordering_consistent` | Push/pop sequence | Every pop is highest priority |

---

## Test Count Summary

| Category | Count |
|----------|-------|
| Unit: JobPriority | 5 |
| Unit: Schedule | 10 |
| Unit: Job | 8 |
| Unit: JobId | 5 |
| Unit: JobResult | 4 |
| Unit: SchedulerConfig | 4 |
| Unit: SchedulerError | 6 |
| Unit: JobRunError | 3 |
| Unit: PriorityQueue Ordering | 6 |
| Unit: PriorityQueue Operations | 9 |
| Integration: Scheduler Lifecycle | 8 |
| Integration: Concurrency Control | 4 |
| Integration: Scheduler State Machine | 5 |
| Edge Cases | 10 |
| Property Tests | 4 |
| **Total** | **91** |

---

## Priority Order for TDD Red Phase

1. **P0 (must have):** Schedule behavior (10), SchedulerError taxonomy (6), PriorityQueue ordering (6)
2. **P1 (should have):** Scheduler lifecycle (8), Job type (8), Concurrency control (4)
3. **P2 (nice to have):** JobId/JobResult/Config tests (13), Edge cases (10)
4. **P3 (property):** Property tests (4)

---

## Contract Invariant Traceability

| Contract Item | Test IDs |
|--------------|----------|
| JobPriority ordering | 1.1, 9.1-9.5, 15.1 |
| Schedule.next_fire_time | 2.3-2.9, 15.2-15.3 |
| PriorityQueue ordering by (priority, fire_at_ms) | 9.1-9.6, 15.4 |
| Scheduler.schedule() returns Result | 11.2-11.3 |
| Scheduler.cancel() removes job | 11.4-11.5 |
| Scheduler.poll_due_jobs() time-based filtering | 10.6-10.8, 11.6-11.7 |
| Concurrency limits via semaphore | 12.1-12.3 |
| Scheduler.run() state management | 12.4, 13.1-13.5 |

---

## Implementation Notes

The current implementation diverges from the full ADR-047 contract in several ways:

1. **No explicit JobState** - States are implicit in queue position
2. **No LifecycleState integration** - Not yet implemented
3. **No DAG dependencies** - Jobs are independent
4. **No lease/fence mechanism** - Concurrency via semaphore only
5. **No retry state machine** - Retry logic is external
6. **Cron not implemented** - Returns None for next_fire_time
7. **JobId is u64** - Not ULID as in contract

Future work should extend tests when these features are implemented.

---

## References

- Contract: `docs/adr/v2/ADR-047-v2-background-job-scheduler-contract.md`
- Implementation: `crates/vo-executor/src/scheduler/`
  - `mod.rs` - Scheduler struct and methods
  - `types.rs` - Job, JobId, Schedule, JobPriority, etc.
  - `error.rs` - SchedulerError, JobRunError
  - `queue.rs` - PriorityQueue implementation