# Martin Fowler Test Plan: capacity_check method (vo-47n)

## Overview

Test plan for `MasterOrchestrator::capacity_check(&self, state: &OrchestratorState) -> bool`.

## Test Strategy

- **Unit tests** covering the boolean comparison logic
- **Boundary tests** at limit edges (0, exactly-at-limit, over-limit)
- **Contract verification** ensuring pre/post conditions

---

## Happy Path Tests

### test_returns_true_when_running_count_zero_and_max_concurrent_three

**Given:** `OrchestratorState` with `running_count = 0` and `MasterOrchestrator` with `max_concurrent = 3`
**When:** `capacity_check` is called
**Then:** returns `true`

### test_returns_true_when_running_count_below_max_concurrent

**Given:** `OrchestratorState` with `running_count = 2` and `MasterOrchestrator` with `max_concurrent = 5`
**When:** `capacity_check` is called
**Then:** returns `true`

---

## Error / Rejection Path Tests

### test_returns_false_when_running_count_equals_max_concurrent

**Given:** `OrchestratorState` with `running_count = 3` and `MasterOrchestrator` with `max_concurrent = 3`
**When:** `capacity_check` is called
**Then:** returns `false`

### test_returns_false_when_running_count_exceeds_max_concurrent

**Given:** `OrchestratorState` with `running_count = 7` and `MasterOrchestrator` with `max_concurrent = 3`
**When:** `capacity_check` is called
**Then:** returns `false`

---

## Edge Case Tests

### test_returns_true_with_max_concurrent_one_and_empty_state

**Given:** `OrchestratorState` with `running_count = 0` and `MasterOrchestrator` with `max_concurrent = 1`
**When:** `capacity_check` is called
**Then:** returns `true`

### test_returns_false_with_max_concurrent_one_and_one_running

**Given:** `OrchestratorState` with `running_count = 1` and `MasterOrchestrator` with `max_concurrent = 1`
**When:** `capacity_check` is called
**Then:** returns `false`

### test_returns_true_with_very_large_max_concurrent

**Given:** `OrchestratorState` with `running_count = 999` and `MasterOrchestrator` with `max_concurrent = 100_000`
**When:** `capacity_check` is called
**Then:** returns `true`

---

## Contract Verification Tests

### test_invariant_max_concurrent_always_positive

**Given:** Any valid `MasterOrchestrator`
**Then:** `self.max_concurrent > 0`

> **Safety Note:** The `max_concurrent > 0` invariant is enforced at `MasterOrchestrator` construction time. It is impossible to construct a `MasterOrchestrator` with `max_concurrent = 0` through the public API.

### test_invariant_count_never_exceeds_limit_in_consistent_state

**Given:** A consistent `OrchestratorState` (no bugs in register/deregister)
**Then:** `state.running_count <= max_concurrent`

### test_invariant_running_count_never_negative

**Given:** Any valid `OrchestratorState`
**Then:** `state.running_count >= 0`

### test_postcondition_returns_exclusive_bound

**Given:** `OrchestratorState` with `running_count = N` and `MasterOrchestrator` with `max_concurrent = M`
**When:** `capacity_check` returns `true`
**Then:** `N < M` holds
**And:** `capacity_check` returns `false` implies `N >= M`

### test_boundary_at_equality_transitions_to_false

**Given:** `OrchestratorState` with `running_count = N` and `MasterOrchestrator` with `max_concurrent = N`
**When:** `capacity_check` is called
**Then:** returns `false`
**Note:** This verifies the critical threshold where `running_count == max_concurrent` transitions from `true` to `false`.

---

## Given-When-Then Scenarios

### Scenario 1: Spawn workflow when capacity available

**Given:** An orchestrator with `max_concurrent = 10` and `running_count = 3`
**When:** A client requests to start a new workflow
**And:** The system calls `capacity_check` before spawning
**Then:** `capacity_check` returns `true`
**And:** The workflow spawn request proceeds

### Scenario 2: Reject workflow when at capacity

**Given:** An orchestrator with `max_concurrent = 5` and `running_count = 5`
**When:** A client requests to start a new workflow
**And:** The system calls `capacity_check` before spawning
**Then:** `capacity_check` returns `false`
**And:** The workflow spawn is rejected with `StartError::AtCapacity`

### Scenario 3: Capacity restored after workflow completion

**Given:** An orchestrator with `max_concurrent = 2`, `running_count = 2`, and one workflow about to complete
**When:** The workflow deregisters (calls `deregister`)
**And:** `running_count` becomes `1`
**Then:** `capacity_check` returns `true`
**And:** A new workflow may be spawned

---

## Property-Based Tests (proptest)

### prop_capacity_check_result_consistent_with_comparison

**Given:** Arbitrary valid `running_count` and `max_concurrent`
**When:** `capacity_check` is evaluated
**Then:** Result equals `(running_count < max_concurrent)`

### prop_false_at_boundary_always

**Given:** Arbitrary `max_concurrent = M`
**When:** `running_count = M`
**Then:** `capacity_check` returns `false`

### prop_true_when_far_below_limit

**Given:** Arbitrary `max_concurrent = M` and `running_count = R`
**When:** `R <= M - 1`
**Then:** `capacity_check` returns `true`

(End of file - total 153 lines)
