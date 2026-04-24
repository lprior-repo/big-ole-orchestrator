# Test Plan: Concurrent Task Scheduler

**Source Contract:** ve-qg90 (Contract: Concurrent task scheduler)
**Bead:** ve-au8k
**Total Tests:** 163

## Testing Trophy Distribution

- **Unit Tests (70%):** Types, state transitions, invariants, error taxonomy, lease logic
- **Integration Tests (20%):** ConcurrentScheduler trait implementations, async flows, storage interaction
- **Property Tests (8%):** Proptest for state machine, fence monotonicity, workload class fairness
- **Mutation Tests (2%):** Kill-specific mutants for critical logic branches

---

## 1. Unit Tests: JobState Enum (8 tests)

| # | Test Name | What | Invariant/Contract |
|---|-----------|------|--------------------|
| 1.1 | `job_state_pending_is_default` | Verify Pending is the default state | State machine starts correctly |
| 1.2 | `job_state_scheduled_contains_fence` | Scheduled { fence: u64 } stores fence token | Fence token present in Scheduled |
| 1.3 | `job_state_running_contains_attempt` | Running { attempt: u32 } stores attempt count | Attempt tracking |
| 1.4 | `job_state_failed_contains_error_and_attempt` | Failed { error, attempt } has both fields | Error + attempt co-exist |
| 1.5 | `job_state_deadlettered_has_final_error_and_attempts` | Deadlettered stores final error + total attempts | Invariant 7: retry budget exceeded |
| 1.6 | `job_state_waiting_for_timer_has_timer_id` | WaitingForTimer stores TimerId | Timer correlation |
| 1.7 | `job_state_completed_has_output` | Completed contains Vec\<u8\> output | Binary output (not String) |
| 1.8 | `job_state_cancelled_has_no_payload` | Cancelled is unit variant | Terminal state |

## 2. Unit Tests: JobState Transitions (18 tests)

| # | Test Name | From → To | Valid? | Invariant |
|---|-----------|-----------|--------|-----------|
| 2.1 | `pending_to_scheduled` | Pending → Scheduled{fence} | Yes | Lease acquired |
| 2.2 | `pending_to_cancelled` | Pending → Cancelled | Yes | Direct cancel |
| 2.3 | `scheduled_to_running` | Scheduled → Running{attempt} | Yes | Execution begins |
| 2.4 | `scheduled_to_failed` | Scheduled → Failed{error,attempt} | Yes | Execution error |
| 2.5 | `running_to_completed` | Running → Completed{output} | Yes | Success |
| 2.6 | `running_to_failed_retryable` | Running → Scheduled{new_fence} | Yes | Retryable, attempts < max |
| 2.7 | `running_to_deadlettered` | Running → Deadlettered{error,attempts} | Yes | Non-retryable or max exceeded |
| 2.8 | `running_to_waiting_for_timer` | Running → WaitingForTimer{timer_id} | Yes | Timer requested |
| 2.9 | `running_to_cancelled` | Running → Cancelled | Yes | Cancel during execution |
| 2.10 | `waiting_for_timer_to_scheduled` | WaitingForTimer → Scheduled | Yes | Timer fired |
| 2.11 | `completed_is_terminal` | Completed → any | No | Terminal state |
| 2.12 | `deadlettered_is_terminal` | Deadlettered → any | No | Terminal state |
| 2.13 | `cancelled_is_terminal` | Cancelled → any | No | Terminal state |
| 2.14 | `pending_to_running_rejected` | Pending → Running | No | Must go through Scheduled |
| 2.15 | `scheduled_to_completed_rejected` | Scheduled → Completed | No | Must go through Running |
| 2.16 | `failed_to_pending_rejected` | Failed → Pending | No | Must go to Scheduled or Deadlettered |
| 2.17 | `deadline_exceeded_goes_to_failed_not_deadletter` | Running → Failed (deadline) | Yes | Invariant 5: deadline → Failed |
| 2.18 | `max_retries_exceeded_goes_to_deadlettered` | Running → Deadlettered | Yes | Invariant 7: retry budget |

### Valid State Transition Diagram

```
Pending ──┬──→ Scheduled ──→ Running ──┬──→ Completed (terminal)
          │                  │         ├──→ Failed ──→ Scheduled (retry)
          │                  │         ├──→ WaitingForTimer ──→ Scheduled
          │                  │         ├──→ Cancelled (terminal)
          │                  │         └──→ Deadlettered (terminal)
          └──→ Cancelled (terminal)
```

## 3. Unit Tests: ConcurrentJob Type (7 tests)

| # | Test Name | What | Contract |
|---|-----------|------|----------|
| 3.1 | `concurrent_job_payload_is_binary` | payload: Vec\<u8\>, not String | Contract 2.2 |
| 3.2 | `concurrent_job_has_dependencies` | dependencies: Vec\<JobId\> | DAG edges |
| 3.3 | `concurrent_job_has_workload_class` | workload_class: WorkloadClass | ADR-033 |
| 3.4 | `concurrent_job_has_optional_deadline` | deadline_ms: Option\<u64\> | Hard deadline |
| 3.5 | `concurrent_job_builder_pattern` | Fluent API for construction | Ergonomic |
| 3.6 | `concurrent_job_default_priority` | Default priority is Normal | Consistent with existing Job |
| 3.7 | `concurrent_job_serialization_roundtrip` | Serialize + deserialize = original | Serde compatibility |

## 4. Unit Tests: WorkloadClass Enum (5 tests)

| # | Test Name | What | Invariant |
|---|-----------|------|-----------|
| 4.1 | `workload_class_variants` | ExactCritical, Standard, UnsafeBulk, Recovery | ADR-033 |
| 4.2 | `workload_class_ordering` | ExactCritical > Recovery > Standard > UnsafeBulk for permit priority | Fairness |
| 4.3 | `unsafe_bulk_cannot_starve_exact_critical` | UnsafeBulk cannot consume all permits | Invariant 6 |
| 4.4 | `recovery_always_gets_capacity` | Recovery class always has reserved capacity | ADR-033 |
| 4.5 | `workload_class_serialization_roundtrip` | All variants survive serde | Persistence |

## 5. Unit Tests: Lease Type (6 tests)

| # | Test Name | What | Invariant |
|---|-----------|------|-----------|
| 5.1 | `lease_has_monotonic_fence` | fence: u64, monotonically increasing | Invariant 2 |
| 5.2 | `lease_has_acquired_at_and_expires_at` | Timestamps present | Time-bounded |
| 5.3 | `lease_has_owner_node_id` | owner: NodeId identifies holder | Single-active-lease |
| 5.4 | `lease_is_expired_when_past_expires_at` | expires_at_ms < now = expired | Lease validity |
| 5.5 | `lease_is_valid_when_before_expires_at` | expires_at_ms >= now = valid | Lease validity |
| 5.6 | `lease_serialization_roundtrip` | Serde roundtrip | Persistence |

## 6. Unit Tests: SchedulerError Taxonomy (10 tests)

| # | Test Name | Error Variant | Fields |
|---|-----------|---------------|--------|
| 6.1 | `scheduler_error_job_not_found` | JobNotFound(JobId) | Has JobId |
| 6.2 | `scheduler_error_queue_full` | QueueFull{capacity, requested} | Has counts |
| 6.3 | `scheduler_error_scheduler_stopped` | SchedulerStopped | Unit |
| 6.4 | `scheduler_error_invalid_schedule` | InvalidSchedule(String) | Has reason |
| 6.5 | `scheduler_error_concurrency_limit_reached` | ConcurrencyLimitReached{class, permits} | Has class+permits |
| 6.6 | `scheduler_error_storage_error` | StorageError(String) | Has message |
| 6.7 | `scheduler_error_dependency_cycle` | DependencyCycleDetected{cycle} | Has cycle Vec |
| 6.8 | `scheduler_error_lease_expired` | LeaseExpired{job_id, fence} | Has both |
| 6.9 | `scheduler_error_stale_fence` | StaleFence{job_id, expected, actual} | Has all 3 |
| 6.10 | `all_scheduler_errors_are_display` | format!("{}", err) works | Debug/Display |

## 7. Unit Tests: JobRunError Taxonomy (6 tests)

| # | Test Name | Error Variant | Fields |
|---|-----------|---------------|--------|
| 7.1 | `job_run_error_failed` | Failed{job_id, reason, is_retryable} | All 3 fields |
| 7.2 | `job_run_error_exceeded_retries` | ExceededRetries{job_id, attempts, last_error} | All 3 fields |
| 7.3 | `job_run_error_cancelled` | Cancelled{job_id} | Has job_id |
| 7.4 | `job_run_error_deadline_exceeded` | DeadlineExceeded{job_id, deadline_ms} | Has both |
| 7.5 | `job_run_error_dependency_failed` | DependencyFailed{job_id, failed_dependency} | Both JobIds |
| 7.6 | `all_job_run_errors_are_display` | format! works for all variants | Debug/Display |

## 8. Unit Tests: Schedule — Extended (4 tests)

| # | Test Name | What | Contract |
|---|-----------|------|----------|
| 8.1 | `schedule_cron_next_fire_unimplemented` | Cron returns None until parser added | Graceful fallback |
| 8.2 | `schedule_one_shot_fires_once` | Second call to next_fire_time returns None | One-shot semantics |
| 8.3 | `schedule_interval_monotonic_fire_times` | Each next fire > previous | Monotonic |
| 8.4 | `schedule_interval_no_overflow` | saturating_add prevents overflow | Safety |

## 9. Unit Tests: ConcurrentScheduler Trait API (12 tests)

| # | Test Name | Method | Success Case |
|---|-----------|--------|--------------|
| 9.1 | `schedule_returns_job_id` | schedule(job) | Returns JobId |
| 9.2 | `cancel_pending_returns_job` | cancel(job_id) | Returns Some(ConcurrentJob) |
| 9.3 | `cancel_nonexistent_returns_job_not_found` | cancel(bogus_id) | Returns Err(JobNotFound) |
| 9.4 | `cancel_completed_returns_none` | cancel(completed_id) | Returns Ok(None) |
| 9.5 | `get_state_returns_pending_after_schedule` | get_state(job_id) | Returns Pending |
| 9.6 | `get_state_returns_completed_after_success` | get_state(job_id) | Returns Completed |
| 9.7 | `poll_ready_returns_empty_when_none_due` | poll_ready(10) | Returns empty Vec |
| 9.8 | `poll_ready_returns_due_jobs` | poll_ready(10) | Returns ready JobIds |
| 9.9 | `poll_ready_respects_max` | poll_ready(2) with 5 ready | Returns 2 |
| 9.10 | `acquire_lease_success` | acquire_lease(job_id) | Returns Lease |
| 9.11 | `release_lease_success` | release_lease(lease, result) | Returns Ok(()) |
| 9.12 | `move_to_deadletter_success` | move_to_deadletter(job_id, error) | Returns Ok(()) |

## 10. Integration Tests: Lease Lifecycle (14 tests)

| # | Test Name | Scenario | Invariant |
|---|-----------|----------|-----------|
| 10.1 | `lease_acquire_pending_job` | Pending → acquire → Scheduled | Lease created |
| 10.2 | `lease_acquire_blocked_by_dependency` | Job has unmet dep → acquire fails | Invariant 3 |
| 10.3 | `lease_acquire_after_dep_completed` | Dep completes → acquire succeeds | Dependency ordering |
| 10.4 | `lease_release_success_transitions_to_completed` | Release with Success | Completed |
| 10.5 | `lease_release_retryable_failure` | Release with retryable, attempts < max | Scheduled (new fence) |
| 10.6 | `lease_release_non_retryable_failure` | Release with non-retryable | Deadlettered |
| 10.7 | `lease_release_max_retries_exceeded` | Release with attempts >= max_retries | Deadlettered |
| 10.8 | `lease_expired_rejected` | Release with expired lease | Err(LeaseExpired) |
| 10.9 | `stale_fence_rejected` | Release with wrong fence | Err(StaleFence) |
| 10.10 | `single_active_lease_enforced` | Two acquires for same job | Invariant 1 |
| 10.11 | `fence_monotonicity_on_retry` | After retry, new fence > old fence | Invariant 2 |
| 10.12 | `lease_cannot_acquire_running_job` | Job already Running | Rejected |
| 10.13 | `lease_cannot_acquire_completed_job` | Job already Completed | Rejected |
| 10.14 | `lease_cannot_acquire_deadlettered` | Job already Deadlettered | Rejected |

## 11. Integration Tests: DAG Dependencies (10 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 11.1 | `linear_dag_a_then_b_then_c` | A→B→C, C depends on B depends on A | Execution order: A, B, C |
| 11.2 | `diamond_dag` | A→B, A→C, B→D, C→D | D waits for both B and C |
| 11.3 | `circular_dependency_rejected` | A→B→C→A | Err(DependencyCycleDetected) |
| 11.4 | `self_dependency_rejected` | A→A | Err(DependencyCycleDetected) |
| 11.5 | `missing_dependency_rejected` | A→B where B doesn't exist | Err(JobNotFound) |
| 11.6 | `dependency_cancelled_blocks` | A depends on B, B cancelled | A cannot run |
| 11.7 | `dependency_failed_blocks` | A depends on B, B failed | DependencyFailed propagated |
| 11.8 | `poll_ready_excludes_blocked_jobs` | Job with unmet deps not returned | Dependency ordering |
| 11.9 | `poll_ready_includes_unblocked_jobs` | After dep completes, job appears | Ready to run |
| 11.10 | `diamond_partial_completion` | A→B→D, A→C→D, only B done | D still blocked |

## 12. Integration Tests: Workload Classes & Fairness (6 tests)

| # | Test Name | Scenario | Invariant |
|---|-----------|----------|-----------|
| 12.1 | `exact_critical_gets_capacity_when_full` | Standard fills all, ExactCritical arrives | ExactCritical gets reserved slot |
| 12.2 | `recovery_always_has_capacity` | All classes saturated, Recovery arrives | Recovery gets slot |
| 12.3 | `unsafe_bulk_cannot_starve_exact_critical` | 1000 UnsafeBulk jobs queued | ExactCritical still gets permits |
| 12.4 | `standard_shares_capacity` | Standard jobs share non-reserved capacity | Fair sharing |
| 12.5 | `workload_class_semaphore_isolation` | Per-class semaphores | Isolation |
| 12.6 | `concurrency_limit_reports_class` | Limit reached error includes class | Observability |

## 13. Integration Tests: Deadletter Queue (7 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 13.1 | `max_retries_to_deadletter` | Retry budget exhausted | Deadlettered state |
| 13.2 | `non_retryable_immediate_deadletter` | Non-retryable error | Immediate deadletter |
| 13.3 | `deadline_exceeded_to_failed_not_deadletter` | Job past deadline | Failed, not Deadlettered |
| 13.4 | `deadletter_stores_final_error` | Deadletter entry has error+attempts | Error preserved |
| 13.5 | `deadletter_is_terminal` | Cannot transition from Deadlettered | Terminal state |
| 13.6 | `move_to_deadletter_api` | Explicit move_to_deadletter call | Ok(()) + state change |
| 13.7 | `move_to_deadletter_nonexistent` | Bad job_id | Err(JobNotFound) |

## 14. Integration Tests: Deadline Enforcement (5 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 14.1 | `deadline_none_means_no_deadline` | deadline_ms: None | No deadline check |
| 14.2 | `deadline_future_does_not_fire` | deadline_ms: now + 1hr | Job runs normally |
| 14.3 | `deadline_past_goes_to_failed` | deadline_ms: now - 1ms | Failed{DeadlineExceeded} |
| 14.4 | `deadline_exact_boundary` | deadline_ms: now exactly | Implementation-defined |
| 14.5 | `deadline_checked_before_lease` | Expired job cannot acquire lease | Rejected before lease |

## 15. Integration Tests: Concurrent Access — Send+Sync (6 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 15.1 | `concurrent_schedule` | 100 tasks schedule jobs | All 100 jobs scheduled |
| 15.2 | `concurrent_poll_ready` | Multiple tasks poll simultaneously | No duplicates |
| 15.3 | `concurrent_acquire_lease` | Two tasks acquire same job | Exactly one succeeds |
| 15.4 | `concurrent_cancel_and_execute` | Cancel + execute race | Deterministic outcome |
| 15.5 | `stress_test_schedule_and_poll` | 10k jobs, 100 concurrent pollers | No panics, no deadlocks |
| 15.6 | `scheduler_is_send_sync` | Static assertion: T: Send + Sync | Compiles |

## 16. Property Tests — proptest (7 tests)

| # | Property | Strategy | Invariant |
|---|----------|----------|-----------|
| 16.1 | `state_transitions_always_valid` | Random (from_state, to_state) pairs | Only valid transitions accepted |
| 16.2 | `fence_monotonicity` | Random sequence of lease acquire/release | Fence never decreases for same job |
| 16.3 | `retry_budget_never_exceeded` | Random failures with random retry settings | Deadlettered iff attempts >= max_retries |
| 16.4 | `dependency_ordering_preserved` | Random DAG of jobs | Never executes before all deps complete |
| 16.5 | `workload_class_capacity_bounds` | Random job arrival patterns | ExactCritical never starved |
| 16.6 | `serializable_types_roundtrip` | Random JobState, ConcurrentJob, etc. | serialize(deserialize(x)) == x |
| 16.7 | `schedule_fire_time_monotonic` | Random schedule configurations | next_fire_time never goes backwards |

## 17. Mutation Testing Targets (9 tests)

| # | Mutant Target | Kill Test | What it proves |
|---|---------------|-----------|----------------|
| 17.1 | `fence_increment_removed` | `fence_monotonicity_on_retry` | Fence increments on each lease |
| 17.2 | `dependency_check_removed` | `lease_acquire_blocked_by_dependency` | Deps checked before lease |
| 17.3 | `retry_count_check_removed` | `max_retries_to_deadletter` | Retry budget enforced |
| 17.4 | `workload_class_check_removed` | `unsafe_bulk_cannot_starve_exact_critical` | Fairness enforced |
| 17.5 | `deadline_check_removed` | `deadline_past_goes_to_failed` | Deadlines enforced |
| 17.6 | `terminal_state_guard_removed` | `completed_is_terminal` | Terminal states are immutable |
| 17.7 | `lease_expiry_check_removed` | `lease_expired_rejected` | Expired leases rejected |
| 17.8 | `stale_fence_check_removed` | `stale_fence_rejected` | Old fences rejected |
| 17.9 | `single_active_lease_check_removed` | `single_active_lease_enforced` | No duplicate leases |

## 18. Edge Cases & Boundary Tests (12 tests)

| # | Test Name | Scenario | Expected |
|---|-----------|----------|----------|
| 18.1 | `empty_dependencies` | ConcurrentJob with no deps | Runs immediately |
| 18.2 | `max_u64_job_id` | JobId(u64::MAX) | Works correctly |
| 18.3 | `zero_max_retries` | max_retries: 0 | First failure → Deadlettered |
| 18.4 | `empty_payload` | payload: vec![] | Valid job |
| 18.5 | `very_large_payload` | payload: vec![0u8; 10MB] | Valid (no unbounded alloc) |
| 18.6 | `schedule_after_stop` | schedule() after SchedulerStopped | Err(SchedulerStopped) |
| 18.7 | `poll_ready_empty_scheduler` | poll_ready on empty | Empty vec |
| 18.8 | `cancel_already_cancelled` | cancel on Cancelled job | Ok(None) |
| 18.9 | `get_state_nonexistent` | get_state on bad id | Err(JobNotFound) |
| 18.10 | `acquire_lease_nonexistent` | acquire_lease on bad id | Err(JobNotFound) |
| 18.11 | `release_lease_twice` | Double release | Second returns Err(StaleFence) |
| 18.12 | `deadletter_already_deadlettered` | move_to_deadletter on Deadlettered | Err or no-op |

## 19. Observability Tests (11 tests)

| # | Test Name | Event/Metric | Verified |
|---|-----------|-------------|----------|
| 19.1 | `job_scheduled_event_emitted` | JobScheduled{job_id, fence, timestamp_ms} | Emitted on schedule |
| 19.2 | `job_started_event_emitted` | JobStarted{job_id, attempt, timestamp_ms} | Emitted on Running |
| 19.3 | `job_completed_event_emitted` | JobCompleted{job_id, duration_ms, output_size} | Emitted on Completed |
| 19.4 | `job_failed_event_emitted` | JobFailed{job_id, error, attempt, timestamp_ms} | Emitted on Failed |
| 19.5 | `job_deadlettered_event_emitted` | JobDeadlettered{job_id, final_error, total_attempts} | Emitted on Deadlettered |
| 19.6 | `lease_acquired_event_emitted` | LeaseAcquired{job_id, fence, expires_at_ms} | Emitted on acquire |
| 19.7 | `lease_expired_event_emitted` | LeaseExpired{job_id, fence} | Emitted on expiry |
| 19.8 | `metrics_pending_gauge` | scheduler_jobs_pending{class, priority} | Correct count |
| 19.9 | `metrics_running_gauge` | scheduler_jobs_running{class} | Correct count |
| 19.10 | `metrics_completed_counter` | scheduler_jobs_completed_total{class} | Increments |
| 19.11 | `metrics_deadletter_gauge` | scheduler_deadletter_size{class} | Correct count |

---

## Test Count Summary

| Category | Count |
|----------|-------|
| Unit: JobState | 8 |
| Unit: State Transitions | 18 |
| Unit: ConcurrentJob | 7 |
| Unit: WorkloadClass | 5 |
| Unit: Lease | 6 |
| Unit: SchedulerError | 10 |
| Unit: JobRunError | 6 |
| Unit: Schedule (extended) | 4 |
| Trait API | 12 |
| Integration: Lease Lifecycle | 14 |
| Integration: DAG Dependencies | 10 |
| Integration: Workload Classes | 6 |
| Integration: Deadletter | 7 |
| Integration: Deadline | 5 |
| Integration: Concurrent Access | 6 |
| Property Tests | 7 |
| Mutation Targets | 9 |
| Edge Cases | 12 |
| Observability | 11 |
| **Total** | **163** |

## Priority Order for TDD Red Phase

1. **P0 (must have):** State transitions (18), SchedulerError taxonomy (10), Lease lifecycle (14)
2. **P1 (should have):** DAG dependencies (10), Concurrent access (6), Trait API (12)
3. **P2 (nice to have):** Workload classes (6), Property tests (7), Observability (11)
4. **P3 (mutation):** Mutation targets (9), Edge cases (12)

## Contract Invariant Traceability

| Invariant | Test IDs |
|-----------|----------|
| 1. Single-active-lease | 10.10, 15.3 |
| 2. Fence monotonicity | 5.1, 10.11, 16.2, 17.1 |
| 3. Dependency ordering | 10.2, 11.1-11.10, 16.4, 17.2 |
| 4. State transition validity | 2.1-2.18, 16.1 |
| 5. Deadline enforcement | 14.1-14.5, 17.5 |
| 6. Workload class isolation | 12.1-12.6, 16.5, 17.4 |
| 7. Retry budget | 13.1, 13.2, 16.3, 17.3 |
