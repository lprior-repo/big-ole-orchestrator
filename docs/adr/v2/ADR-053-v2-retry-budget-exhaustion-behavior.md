# ADR 053 (v2): Retry Budget Exhaustion Behavior

## Status
Accepted

## Context

When a workflow or managed effect retry budget is exhausted, the system must define clear, observable behavior. Without explicit definition, retry exhaustion can lead to undefined states, infinite retry loops, or silent failures.

ADR-034 covers saga compensation for committed managed effects, but does not specifically address retry budget exhaustion at the workflow level.

## Decision

### 1. Retry Budget is Always Finite

The `RetryBudget` type (vo-worker) enforces that retry budgets are always finite token counts. There is no mechanism for infinite retry budgets.

```rust
pub struct RetryBudget {
    available_tokens: AtomicU32,
    max_tokens: u32,        // Must be finite u32
    refill_rate: u32,       // Must be finite u32
    refill_interval: Duration,
}
```

**Invariant:** `max_tokens` is always a concrete, finite value. The system SHALL NOT allow infinite retry budgets.

### 2. Terminal Failure on Exhaustion

When a retry budget is exhausted during workflow execution:

1. The `RetryBudget::try_acquire()` returns `false`
2. The retry loop returns `RetryExhausted` error
3. The workflow transitions to `LifecycleState::Failed` (terminal state)

```
Workflow Exhausts Retry Budget
         │
         ▼
   ┌─────────────────┐
   │     Failed      │  ← Terminal State (LifecycleState::Failed)
   └─────────────────┘
```

The `LifecycleState::Failed` is observable via:
- Workflow status query API
- State change events
- Saga coordinator state transitions

### 3. No Automatic Indefinite Retry

The system SHALL NOT retry indefinitely when budget is exhausted. This is enforced by:

1. **Finite budget**: `RetryBudget` has fixed `max_tokens`
2. **Circuit breaker**: `RetryCircuitBreaker` trips after consecutive failures
3. **Max attempts**: `RetryConfig::max_attempts` bounds retry attempts

### 4. Compensation Semantics (ADR-034)

Per ADR-034, retry exhaustion does NOT automatically trigger compensation. Compensation is triggered by:

- **None policy**: Workflow enters terminal failure, requires operator intervention
- **Automatic policy**: Compensation is attempted, may succeed or fail
- **Manual policy**: Operator must explicitly approve compensation

Retry exhaustion is distinct from compensation - it's a signal that the workflow cannot make forward progress within its budget.

## Contracts

### Preconditions
- Retry budget is configured with finite `max_tokens`
- Retry budget tokens may be consumed over time via refill mechanism

### Postconditions
- On budget exhaustion: `LifecycleState::Failed` (terminal)
- Error chain includes `RetryExhausted` with attempt count and last error
- No retry occurs after budget exhaustion

### Invariants
- `RetryBudget::max_tokens` is always finite (u32::MAX maximum)
- No mechanism exists for infinite retry budget

## Consequences

- **Positive:** Retry exhaustion is deterministic and observable
- **Positive:** Finite budgets prevent resource exhaustion attacks
- **Positive:** Clear terminal state enables alerting and monitoring
- **Negative:** Workflows must handle terminal failure explicitly
- **Negative:** Requires operator intervention for `None` compensation policy workflows

## References

- ADR-034: Saga Compensation and Reversibility
- ADR-039: Hierarchical Lifecycle State Machine
- vo-worker:retry::RetryBudget
- vo-common:error::ExecutionError::RetryExhausted
- vo-types:state::lifecycle::LifecycleState