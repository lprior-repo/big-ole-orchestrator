# Test Suite Review: Concurrent Task Scheduler

**Module**: `crates/vo-executor/tests/scheduler_tests.rs`
**Reviewer**: veloxide/polecats/brahmin
**Date**: 2026-04-12
**Bead**: ve-mnwy
**Status**: AUDIT COMPLETE

---

## VERDICT: CRITICAL GAP — WRONG SCHEDULER BEING TESTED

---

## Executive Summary

The test plan (`docs/contracts/concurrent-task-scheduler-test-plan.md`) specifies 163 tests for a **ConcurrentScheduler** with sophisticated features:
- JobState enum with 8 states (Pending, Scheduled, Running, Failed, Deadlettered, WaitingForTimer, Completed, Cancelled)
- ConcurrentJob with binary payload (Vec<u8>), DAG dependencies, WorkloadClass
- WorkloadClass enum (ExactCritical, Standard, UnsafeBulk, Recovery)
- Lease type with monotonic fence tokens for optimistic concurrency
- ConcurrentScheduler trait (schedule, cancel, get_state, poll_ready, acquire_lease, release_lease, move_to_deadletter)
- DAG dependency ordering
- Workload class fairness guarantees

The actual test file (`scheduler_tests.rs`) tests a **basic priority queue Scheduler** with:
- Simple Job type with String payload
- No DAG dependencies
- No WorkloadClass
- No Lease/fence mechanism
- No ConcurrentScheduler trait

**These are fundamentally different schedulers. The test plan was written for an architecture that was never implemented.**

---

## Tier 0 — Static Analysis

[PASS] No `#[ignore]`, no sleep, no mocks in test file
[PASS] No shared mutable state in tests
[FAIL] Test plan specifies 163 tests; actual implementation has 60 tests
[FAIL] Test plan and implementation describe different systems
[FAIL] Missing all JobState tests (0/8)
[FAIL] Missing all WorkloadClass tests (0/5)
[FAIL] Missing all Lease tests (0/6)
[FAIL] Missing ConcurrentScheduler trait tests (0/12)
[FAIL] Missing all DAG dependency tests (0/10)
[FAIL] Missing all workload fairness tests (0/6)
[FAIL] Missing all deadletter queue tests (0/7)
[FAIL] Missing all deadline enforcement tests (0/5)
[FAIL] Missing all concurrent access tests (0/6)
[FAIL] Missing all property tests (0/7 proptest)
[FAIL] Missing all mutation tests (0/9)
[FAIL] Missing all observability tests (0/11)
[FAIL] Missing all state transition tests (0/18)

---

## Tier 1 — Execution

[PASS] cargo test: 60 passed, 0 failed
[PASS] Tests compile without errors
[PASS] All basic types have minimal coverage

---

## Test Coverage Analysis

### What the Test Plan Specifies (163 tests)

| Category | Planned | Covered | Gap |
|----------|---------|---------|-----|
| Unit: JobState | 8 | 0 | 8 |
| Unit: State Transitions | 18 | 0 | 18 |
| Unit: ConcurrentJob | 7 | 0 | 7 |
| Unit: WorkloadClass | 5 | 0 | 5 |
| Unit: Lease | 6 | 0 | 6 |
| Unit: SchedulerError | 10 | 6 | 4 |
| Unit: JobRunError | 6 | 3 | 3 |
| Unit: Schedule | 4 | 10 | +6 (over-covered) |
| Trait API | 12 | 0 | 12 |
| Integration: Lease Lifecycle | 14 | 0 | 14 |
| Integration: DAG Dependencies | 10 | 0 | 10 |
| Integration: Workload Classes | 6 | 0 | 6 |
| Integration: Deadletter Queue | 7 | 0 | 7 |
| Integration: Deadline | 5 | 0 | 5 |
| Integration: Concurrent Access | 6 | 3 | 3 |
| Property Tests (proptest) | 7 | 0 | 7 |
| Mutation Testing | 9 | 0 | 9 |
| Edge Cases | 12 | 10 | 2 |
| Observability | 11 | 0 | 11 |
| **TOTAL** | **163** | **~60** | **~103** |

### Actual Tests in scheduler_tests.rs (~60 tests)

The tests cover these categories:

**Unit Tests (~45 tests)**:
- JobPriority: 5 tests (ordering, debug, default, variants)
- Schedule: 10 tests (cron, one-shot, interval, boundaries)
- Job: 8 tests (construction, builder, payload, defaults)
- JobId: 5 tests (construction, equality, hash, display)
- JobResult: 4 tests (success/failure, fields)
- SchedulerConfig: 4 tests (defaults, custom, zero values)
- SchedulerError: 6 tests (Display for 6 variants)
- JobRunError: 3 tests (Failed, ExceededRetries, Cancelled)

**Integration Tests (~15 tests)**:
- scheduler_lifecycle: 6 tests (schedule, cancel, poll_due_jobs)
- scheduler_concurrency: 3 tests (try_acquire, start/stop)
- priority_queue: 6 tests (priority ordering, due job filtering)

---

## Critical Findings

### C1. ARCHITECTURAL MISMATCH — Test plan describes different system

The test plan (ve-au8k) was written for `ConcurrentScheduler` with:
- `JobState` enum (8 states)
- `ConcurrentJob` with `payload: Vec<u8>` (binary), `dependencies: Vec<JobId>`, `workload_class: WorkloadClass`
- `WorkloadClass` enum: `ExactCritical`, `Standard`, `UnsafeBulk`, `Recovery`
- `Lease` type with `fence: u64` (monotonic token)
- `ConcurrentScheduler` trait with `acquire_lease`, `release_lease`, `move_to_deadletter`
- DAG dependency graph support
- Workload class fairness via semaphore isolation

The actual `vo-executor` has:
- `Job` with `payload: String` (not binary)
- No dependencies field
- No WorkloadClass
- No Lease mechanism
- No ConcurrentScheduler trait
- Basic priority queue

**Impact**: The test plan cannot be implemented without implementing the ConcurrentScheduler architecture first.

### C2. Missing State Machine Tests (18 planned, 0 implemented)

The test plan requires 18 state transition tests verifying:
- All valid transitions (Pending→Scheduled, Scheduled→Running, etc.)
- All invalid transitions are rejected (e.g., Pending→Running rejected)
- Terminal states (Completed, Deadlettered, Cancelled) are immutable

The actual implementation has no `JobState` enum at all.

### C3. Missing Lease/Fence Tests (6 planned, 0 implemented)

The test plan requires 6 Lease tests:
- `lease_has_monotonic_fence` (fence: u64, monotonically increasing)
- `lease_has_acquired_at_and_expires_at`
- `lease_has_owner_node_id`
- `lease_is_expired_when_past_expires_at`
- `lease_is_valid_when_before_expires_at`
- `lease_serialization_roundtrip`

And 14 integration tests for lease lifecycle including fence monotonicity on retry.

### C4. Missing DAG Dependency Tests (10 planned, 0 implemented)

The test plan requires:
- `linear_dag_a_then_b_then_c`
- `diamond_dag`
- `circular_dependency_rejected`
- `self_dependency_rejected`
- `missing_dependency_rejected`
- `dependency_cancelled_blocks`
- `dependency_failed_blocks`
- `poll_ready_excludes_blocked_jobs`
- `poll_ready_includes_unblocked_jobs`
- `diamond_partial_completion`

The actual implementation has no dependency field on Job.

### C5. Missing WorkloadClass Tests (5 planned, 0 implemented)

The test plan requires tests for WorkloadClass enum:
- `ExactCritical`, `Standard`, `UnsafeBulk`, `Recovery` variants
- Ordering for permit priority (ExactCritical > Recovery > Standard > UnsafeBulk)
- `unsafe_bulk_cannot_starve_exact_critical` (fairness invariant)
- `recovery_always_gets_capacity` (reserved capacity)

The actual implementation has no WorkloadClass.

### C6. Missing Property Tests (7 planned, 0 implemented)

The test plan calls for proptest-based property tests:
- `state_transitions_always_valid`
- `fence_monotonicity`
- `retry_budget_never_exceeded`
- `dependency_ordering_preserved`
- `workload_class_capacity_bounds`
- `serializable_types_roundtrip`
- `schedule_fire_time_monotonic`

No proptest configuration exists in the test file.

### C7. Missing Mutation Tests (9 planned, 0 implemented)

The test plan specifies 9 mutation testing targets with specific kill tests (e.g., removing fence_increment should be killed by `fence_monotonicity_on_retry`).

### C8. Missing Observability Tests (11 planned, 0 implemented)

The test plan requires 11 event/metric tests:
- JobScheduled, JobStarted, JobCompleted, JobFailed, JobDeadlettered events
- LeaseAcquired, LeaseExpired events
- Metrics: pending gauge, running gauge, completed counter, deadletter gauge

---

## What IS Properly Tested

### Schedule Type (10 tests)
The Schedule enum and its `next_fire_time` logic is reasonably tested:
- Cron returns None (graceful fallback)
- One-shot fires once, not twice
- Interval produces monotonic fire times
- No overflow at u64::MAX (saturating_add)

### JobPriority Ordering (5 tests)
- Critical < High < Normal < Low ordering is tested
- Default is Normal

### Basic Scheduler Lifecycle (6 tests)
- schedule() and poll_due_jobs() work
- cancel() removes jobs
- max_jobs_per_scan is respected

---

## REQUIRED ACTIONS

### Immediate (Before any further test work)

1. **Resolve architectural ambiguity**: The test plan and implementation describe different schedulers. Either:
   - **Option A**: Implement the ConcurrentScheduler architecture to match the test plan
   - **Option B**: Rewrite the test plan to match the basic Scheduler that was implemented

2. **If Option B** (likely easier): Rewrite test plan to cover:
   - Current JobState is implicit (no explicit state enum)
   - Current Scheduler has no lease/fence mechanism
   - Current Job has String payload, not binary
   - No DAG dependencies in current implementation
   - No WorkloadClass in current implementation

### If Option A (implement ConcurrentScheduler)

The following must be built first:
1. `JobState` enum with all 8 states
2. `ConcurrentJob` type with binary payload, dependencies, workload_class
3. `WorkloadClass` enum with 4 variants
4. `Lease` type with monotonic fence tokens
5. `ConcurrentScheduler` trait
6. DAG dependency resolution
7. Workload class semaphore isolation
8. Observability events

Then the 163 tests can be implemented.

---

## FILES AUDITED

| File | Lines | Tests | Notes |
|------|-------|-------|-------|
| `crates/vo-executor/tests/scheduler_tests.rs` | 824 | ~60 | Main test file |
| `crates/vo-executor/src/scheduler/types.rs` | 193 | 4 (inline) | Job, JobId, Schedule, JobPriority |
| `crates/vo-executor/src/scheduler/error.rs` | 39 | 0 | SchedulerError, JobRunError |
| `docs/contracts/concurrent-task-scheduler-test-plan.md` | 329 | N/A | Test specification |

---

## RECOMMENDATION

This is a **test plan/implementation mismatch** issue, not a test quality issue. The tests that exist are reasonably written for what they test, but they test a different system than what the test plan specifies.

**Decision needed**: Should we:
1. Build the ConcurrentScheduler to match the test plan (large effort)?
2. Rewrite the test plan to match the existing Scheduler (moderate effort)?
3. Something else?

Until this is resolved, no meaningful test improvement can proceed.

---

*Review completed by brahmin (polecat) for bead ve-mnwy*
