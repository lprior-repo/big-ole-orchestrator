# Test Plan: Load Shedding Exhaustive Test Strategy

## Summary
- Behaviors identified: 12
- Trophy allocation: 8 unit / 4 integration / 0 e2e
- Proptest invariants: 4
- Fuzz targets: 1
- Kani harnesses: 0

## 1. Behavior Inventory

### ExecutionSemaphore Behaviors

1. "ExecutionSemaphore admits request when permits available"
2. "ExecutionSemaphore queues request when no permits but below wait threshold"
3. "ExecutionSemaphore rejects request with LoadShed when waiters exceed threshold"
4. "ExecutionSemaphore rejects request with Timeout when acquire times out"
5. "ExecutionSemaphore admits from reserved pool for recovery tasks"
6. "ExecutionSemaphore returns correct available permits count"
7. "ExecutionSemaphore returns correct waiting count"
8. "ExecutionSemaphore correctly reports load shedding status"

### WorkflowSemaphoreMap Behaviors

9. "WorkflowSemaphoreMap creates per-workflow semaphores on demand"
10. "WorkflowSemaphoreMap returns same semaphore for same workflow"
11. "WorkflowSemaphoreMap cleans up idle semaphores"

### InvariantEnforcer Behaviors

12. "InvariantEnforcer denies activation when instance already active"

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit (Calc) | 8 | Pure functions: `calculate_backpressure_status`, `estimate_wait_ms`, `is_workflow_saturated`; `try_acquire` variants |
| Integration | 4 | `acquire()` async with real semaphore; `WorkflowSemaphoreMap` with HashMap; `InvariantEnforcer` with registry |
| E2E | 0 | No CLI/API surface for semaphore directly |
| Static | 1 | Clippy on all modules |

## 3. BDD Scenarios

### Behavior 1: ExecutionSemaphore admits request when permits available

```
Given: ExecutionSemaphore with 500 permits, 0 waiting
When: try_acquire() is called
Then: Some(permit) is returned
And: available_permits() equals 499
```

**Test name**: `fn execution_semaphore_returns_permit_when_available`

### Behavior 2: ExecutionSemaphore queues request when no permits but below wait threshold

```
Given: ExecutionSemaphore with 0 available permits, 100 waiting, max_waiters=5000
When: acquire() is called
Then: AdmissionDecision::Queued { position, estimated_wait_ms } is returned
And: waiting_count() increases by 1
```

**Test name**: `fn execution_semaphore_queues_when_no_permits_below_threshold`

### Behavior 3: ExecutionSemaphore rejects with LoadShed when waiters exceed threshold

```
Given: ExecutionSemaphore with max_waiters_for_shed=5000, 5001 waiting tasks
When: acquire() is called
Then: AdmissionDecision::Rejected { reason: RejectionReason::LoadShed, retry_after_secs: 5 }
And: waiting_count() remains unchanged
```

**Test name**: `fn execution_semaphore_rejects_with_load_shed_when_waiters_exceed_threshold`

### Behavior 4: ExecutionSemaphore rejects with Timeout when acquire times out

```
Given: ExecutionSemaphore with 0 permits, no other waiters, acquire_timeout=10ms
When: acquire() is called and times out
Then: AdmissionDecision::Rejected { reason: RejectionReason::Timeout, retry_after_secs: 10 }
```

**Test name**: `fn execution_semaphore_returns_timeout_when_acquire_times_out`

### Behavior 5: ExecutionSemaphore admits from reserved pool for recovery tasks

```
Given: ExecutionSemaphore with 0 general permits, 1 reserved permit
When: try_acquire_recovery() is called
Then: Some(permit) is returned
And: reserved_available() decreases by 1
```

**Test name**: `fn execution_semaphore_acquires_from_reserved_pool_when_general_exhausted`

### Behavior 6: ExecutionSemaphore admits recovery when general pool is exhausted

```
Given: ExecutionSemaphore with max_concurrent_binaries=0, reserved_permits=1
When: try_acquire() is called
Then: None is returned
When: try_acquire_recovery() is called
Then: Some(permit) is returned
```

**Test name**: `fn execution_semaphore_recovery_independent_of_general_pool`

### Behavior 7: calculate_backpressure_status returns correct status

```
Given: available=400, total=500, waiting=50, max_waiters=5000
When: calculate_backpressure_status is called
Then: BackpressureStatus::Healthy

Given: available=100, total=500, waiting=300, max_waiters=5000
When: calculate_backpressure_status is called
Then: BackpressureStatus::Heavy

Given: available=0, total=500, waiting=5001, max_waiters=5000
When: calculate_backpressure_status is called
Then: BackpressureStatus::ShedLoad
```

**Test names**:
- `fn backpressure_status_is_healthy_when_under_threshold`
- `fn backpressure_status_is_heavy_when_waiters_high`
- `fn backpressure_status_is_shed_load_when_waiters_exceed_max`

### Behavior 8: estimate_wait_ms calculates correctly

```
Given: position=50, available_permits=10, avg_task_duration_ms=100
When: estimate_wait_ms(50, 10, 100) is called
Then: 600 (position/available + 1 * avg)

Given: position=5, available_permits=0, avg_task_duration_ms=100
When: estimate_wait_ms(5, 0, 100) is called
Then: 600 ((position+1) * avg when no permits)
```

**Test names**:
- `fn estimate_wait_ms_returns_correct_time_with_permits`
- `fn estimate_wait_ms_returns_correct_time_without_permits`

### Behavior 9: is_workflow_saturated detects saturation

```
Given: pending_count=5, max_per_workflow=10
When: is_workflow_saturated(5, 10) is called
Then: false

Given: pending_count=10, max_per_workflow=10
When: is_workflow_saturated(10, 10) is called
Then: true
```

**Test names**:
- `fn workflow_saturated_is_false_when_under_limit`
- `fn workflow_saturated_is_true_when_at_limit`

### Behavior 10: WorkflowSemaphoreMap creates per-workflow semaphores

```
Given: WorkflowSemaphoreMap with max_per_workflow=10
When: semaphore_for("workflow-a") is called
Then: returns Arc<Semaphore>
And: len() equals 1
```

**Test name**: `fn workflow_semaphore_map_creates_semaphore_on_demand`

### Behavior 11: WorkflowSemaphoreMap returns same semaphore for same workflow

```
Given: WorkflowSemaphoreMap with max_per_workflow=10
When: semaphore_for("workflow-a") is called twice
Then: same Arc<Semaphore> returned both times
And: len() equals 1 (not 2)
```

**Test name**: `fn workflow_semaphore_map_returns_same_semaphore_for_same_workflow`

### Behavior 12: InvariantEnforcer denies activation when instance already active

```
Given: InvariantEnforcer with instance already registered as active
When: check_activation(instance_id) is called
Then: InvariantCheck { allowed: false, error: Some(InstanceAlreadyActive) }
```

**Test name**: `fn invariant_enforcer_denies_when_instance_already_active`

## 4. Proptest Invariants

### Proptest: calculate_backpressure_status

**Invariant**: `calculate_backpressure_status` never panics and always returns a valid BackpressureStatus

**Strategies**:
- `available_permits`: 0..=1000 (any usize)
- `total_permits`: 1..=1000 (never 0 to avoid div by zero)
- `waiting_count`: 0..=10000
- `max_waiters_for_shed`: 1..=10000

**Anti-invariant**: `total_permits == 0` should be handled gracefully (returns ShedLoad if waiting > 0)

### Proptest: estimate_wait_ms

**Invariant**: Returns value >= avg_task_duration_ms when position > 0

**Strategies**:
- `position`: 0..=1000
- `available_permits`: 0..=100
- `avg_task_duration_ms`: 1..=60000

### Proptest: is_workflow_saturated

**Invariant**: Returns false when pending_count < max_per_workflow, true when >=

**Strategies**:
- `pending_count`: 0..=100
- `max_per_workflow`: 1..=100

### Proptest: ExecutionSemaphore permit accounting

**Invariant**: available_permits + waiting_count <= total_permits (after each acquire completes)

**Strategy**: Run concurrent acquire/timeout scenarios

## 5. Fuzz Targets

### Fuzz Target: SemaphoreConfig deserialization

**Input type**: YAML or JSON representing SemaphoreConfig
**Risk**: Panic on invalid config values (e.g., max_concurrent_binaries = 0)
**Corpus seeds**: Default config, zero values, max values

```
// Candidate fuzz target
fn fuzz_semaphore_config_parse(data: &[u8]) {
    if let Ok(config) = serde_json::from_slice::<SemaphoreConfig>(data) {
        let sem = ExecutionSemaphore::new(config);
        // Verify semaphore is functional
        assert!(sem.total_permits() > 0);
    }
}
```

## 6. Kani Harnesses

No Kani harnesses required - semaphore uses tokio primitives which Kani cannot verify.

## 7. Mutation Checkpoints

**Critical mutations to survive**:

| Mutation | Must be caught by |
|----------|------------------|
| `waiting_count >= max_waiters_for_shed` → `>` | `test_rejects_load_shed_when_waiters_exceed` |
| `usage_ratio > 0.8` → `>= 0.8` | `test_heavy_status_at_80_percent` |
| `estimate_wait_ms` division → off-by-one | `test_estimate_wait_ms_correct` |
| `is_workflow_saturated` ≥ → > | `test_workflow_saturated_at_limit` |

**Threshold**: 90% mutation kill rate minimum

## 8. Combinatorial Coverage Matrix

### BackpressureStatus Calculation

| Scenario | available | total | waiting | max_waiters | Expected Status |
|----------|-----------|-------|---------|-------------|-----------------|
| healthy-1 | 400 | 500 | 50 | 5000 | Healthy |
| healthy-2 | 250 | 500 | 100 | 5000 | Healthy |
| moderate-1 | 200 | 500 | 200 | 5000 | Moderate |
| moderate-2 | 250 | 500 | 150 | 5000 | Moderate |
| heavy-1 | 100 | 500 | 300 | 5000 | Heavy |
| heavy-2 | 50 | 500 | 250 | 5000 | Heavy |
| shed-1 | 0 | 500 | 5001 | 5000 | ShedLoad |
| shed-2 | 0 | 500 | 10000 | 5000 | ShedLoad |

### AdmissionDecision Variants

| Scenario | Permits | Waiters | Timeout | Expected Decision |
|----------|---------|---------|---------|-------------------|
| admit-immediate | 1 | 0 | no | Admitted |
| admit-after-wait | 0 | 0 | no | Admitted (permit released then acquired) |
| queue | 0 | 100 | no | Queued |
| reject-load | 0 | 5001 | no | Rejected(LoadShed) |
| reject-timeout | 0 | 1 | yes | Rejected(Timeout) |
| recover-pool | 0 (general) | - | - | None (general) |
| recover-pool | 0 (reserved) | - | - | Some (reserved) |

## Open Questions

1. Should `acquire_timeout` be configurable per-call or only at construction?
2. Is 5 seconds the correct `retry_after_secs` for LoadShed? Should it scale with waiters?
3. Should `WorkflowSemaphoreMap::cleanup_idle` be tested for eventual consistency or immediate cleanup?
