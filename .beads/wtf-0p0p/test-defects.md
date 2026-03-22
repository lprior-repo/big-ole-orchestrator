# Test Defects Report: wtf-0p0p

## Status: REJECTED

## Review Framework
- Testing Trophy (Kent Beck): Integration-weighted testing
- Dan North BDD: Given-When-Then behavior-first
- ATDD: Acceptance criteria validation

---

## Critical Defects

### 1. Missing Error Path Tests (Contract-Test Misalignment)

**Contract specifies (contract.md:34-37):**
```
Error Taxonomy:
- `OrchestratorError::JetStreamUnavailable` - JetStream connection lost
- `OrchestratorError::WorkflowNotFound` - Unknown workflow ID
```

**Defect:** No test exercises `JetStreamUnavailable` or `WorkflowNotFound` error paths.

**Impact:** High - These are primary failure modes for a distributed system.

---

### 2. Invariant Verification Gap: Zero Durable State

**Contract specifies (contract.md:57):**
```
- No mutable state survives restart (derived from JetStream)
```

**Defect:** Test "Zero durable state in memory after restart" (martin-fowler-tests.md:173-177) does NOT actually restart the actor. It only describes expected behavior without verification.

**Impact:** Critical - This is the core guarantee of the architecture.

---

### 3. Missing Snapshot Failure Test

**Contract specifies (contract.md:110):**
```
Postconditions:
- Snapshot is written to JetStream
```

**Defect:** No test verifies behavior when snapshot write fails mid-operation.

**Impact:** Medium - Crash recovery could silently lose state.

---

### 4. Incomplete BDD "Then" Clauses

**Example (martin-fowler-tests.md:43):**
```
Then: state remains Running, `FsmEvent::TransitionRejected { reason: InvalidTransition }` is emitted, no state change
```

**Defect:** "no state change" is vague. Should verify:
- State unchanged (same reference)
- No event emitted to JetStream
- Command returns error

**Impact:** Low - Ambiguous acceptance criteria.

---

### 5. Missing Concurrent Command Edge Case

**Gap:** No test for multiple simultaneous commands to same actor (race condition).

**Impact:** Medium - Production systems face concurrent load.

---

## Testing Trophy Assessment

| Level | Coverage | Notes |
|---|---|---|
| E2E / Integration | Partial | JetStream replay covered, multi-actor E2E missing |
| Integration | Good | Actor-JetStream integration tested |
| Unit | Thin | Boundary conditions, edge cases missing |

**Verdict:** Skewed toward integration but missing critical integration paths.

---

## Recommendations

1. Add error path tests for all `OrchestratorError` variants
2. Implement actual restart verification (not just description)
3. Add snapshot-write-failure test
4. Clarify ambiguous "Then" outcomes
5. Add concurrent command test
