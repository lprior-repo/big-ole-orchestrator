# ADR 056 (v2): Quota Exhaustion Fallback Behavior

## Status

Accepted

## Context

Veloxide enforces resource limits through multiple quota mechanisms: execution semaphores (ADR-006), reserved permit budgets per workload class (ADR-033), and retry budgets (ADR-053). Each mechanism handles exhaustion differently, but there is no unified specification defining what "quota exhaustion fallback behavior" means across the system.

Without a unified ADR, different quota types have inconsistent fallback behaviors:
- Execution semaphore: actors yield and wait in queue (ADR-006)
- Reserved permit budget: returns error, caller decides (ADR-033)
- Retry budget: terminal failure (ADR-053)

This ADR establishes a consistent framework for all quota exhaustion scenarios.

## Decision

### 1. Quota is Always Finite

All quotas in Veloxide are finite. There is no mechanism for infinite quotas.

**Invariant:** The system SHALL NOT allow infinite quotas. Quota limits are always concrete, finite values (u32, usize, or similar bounded types).

### 2. Quota Categories and Their Fallback Behavior

Veloxide has three categories of quotas, each with distinct fallback semantics:

#### Category A: Yielding Quotas (Backpressure)

Used for: execution permits, concurrent subprocess limits

**Fallback:** When quota is exhausted, requestors yield and wait in queue. This is zero-cost (actor consumes ~1KB RAM, 0% CPU while waiting).

```
Request arrives
       │
       ▼
 Is quota available?
       │
  ┌────┴────┐
  │         │
 Yes        No ──────────────────┐
  │         │                    │
  ▼         ▼                    │
 Acquire    Wait in queue        │
 permit     (zero-cost yield)    │
  │         │                    │
  └────┬────┘                    │
       │                         │
       ▼                         │
   Request                      │
   proceeds                     │
       │                         │
       ▼                         │
   Release                      │
   permit                       │
       │                         │
       └─────────────────────────┘
```

**Invariant:** `acquire()` followed by `release()` maintains system-wide quota limit.

#### Category B: Shedding Quotas (Load Shedding)

Used for: ingress HTTP requests, webhook processing

**Fallback:** When quota is exhausted, new requests are rejected immediately with HTTP 429 (Too Many Requests) or HTTP 503 (Service Unavailable) with a `Retry-After` header.

**Invariant:** System-wide quota limit is never exceeded because requests are rejected at the boundary before entering the system.

#### Category C: Terminal Quotas (Fail-Fast)

Used for: retry budgets, workflow execution budgets

**Fallback:** When quota is exhausted, the operation fails immediately with a typed error (`QuotaExhausted`). The caller receives `Err(QuotaExhausted)` and must handle it.

```
Operation requests quota
       │
       ▼
 Is quota available?
       │
  ┌────┴────┐
  │         │
 Yes        No
  │         │
  ▼         ▼
 Acquire  Return Err(QuotaExhausted)
 permit   immediately (no waiting)
  │
  ▼
 Operation
 proceeds
```

**Invariant:** `try_acquire()` returns `Err` when quota unavailable - it never blocks or waits.

### 3. Fallback Mechanism Selection

The category of quota determines its fallback behavior:

| Quota Type | Category | Fallback | Blocks Caller? |
|------------|----------|----------|----------------|
| Execution semaphore | A: Yielding | Waits in queue | Yes (async yield) |
| Ingress load shedding | B: Shedding | HTTP 429/503 | No (rejected at boundary) |
| Reserved permit budget | C: Terminal | Returns error | No (fail-fast) |
| Retry budget | C: Terminal | Returns error | No (fail-fast) |

### 4. Unified Error Type

All Category C (fail-fast) quotas use a common error type:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuotaExhaustionError {
    #[error("Quota exhausted for {resource}: requested {requested}, available {available}")]
    Exhausted {
        resource: String,
        requested: u32,
        available: u32,
    },
}
```

This provides consistent error reporting across all quota types.

### 5. Observability

All quota exhaustion events are observable:

- **Metrics:** `quota_exhaustion_total{resource, category}` counter
- **Traces:** Span events with `quota.exhausted=true` and resource/category attributes
- **Logs:** Structured log with `quota.exhausted` field

### 6. Graceful Degradation

When quota is exhausted, the system prioritizes degradation in this order:

1. **Reject new work** (Category B) before queueing
2. **Queue existing work** (Category A) before dropping
3. **Fail fast** (Category C) before corrupting state

## Contracts

### Preconditions
- Quota is configured with finite `max` value
- Quota tokens may be consumed over time via acquire/release

### Postconditions
- On quota exhaustion (Category A): caller awaits in queue until permit available
- On quota exhaustion (Category B): caller receives HTTP 429/503 immediately
- On quota exhaustion (Category C): caller receives `Err(QuotaExhausted)` immediately

### Invariants
- `max` is always finite (system-defined maximum, not configurable to infinity)
- No mechanism exists for infinite quotas
- System-wide quota limit is never exceeded (enforced by acquire semantics)

## Consequences

- **Positive:** Consistent fallback behavior across all quota types
- **Positive:** Clear decision tree for quota category selection
- **Positive:** Observable quota exhaustion events enable alerting
- **Positive:** Graceful degradation prevents resource exhaustion attacks
- **Negative:** Different quota categories require different handling patterns
- **Negative:** Category selection requires upfront design decision

## References

- ADR-006: Backpressure and Load Shedding (Category A: Yielding)
- ADR-033: Fairness and Workload Classes (Category C: Terminal)
- ADR-053: Retry Budget Exhaustion Behavior (Category C: Terminal)
- vo-actor:start_budget::StartError::BudgetExhaustion
- vo-actor:start_budget::ReservedPermitBudget