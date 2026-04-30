# ADR 051: Circuit Breaker Reset and Auto-Recovery

## Status

Accepted

## Context

ADR-026 established a dual-layered circuit breaker for AI deployment protection:
- **Layer 1**: Rate limiting (60s cooldown between registrations)
- **Layer 2**: Failure loop detection (5 unique binary failures in 10 minutes → automatic quarantine)

However, the current implementation requires **manual unquarantine** via operator CLI. This creates an operational burden: when a workflow is legitimately stuck in a bad state, the AI cannot self-heal — a human must intervene. This impacts availability and increases MTTR.

ADR-026's own "Unwanted" row correctly identifies the problem:
> "IF breaker stuck in OPEN with no auto-reset, THE SYSTEM SHALL NOT require only manual intervention"

## Decision

We extend the circuit breaker with an **automatic reset and recovery state machine** that provides self-healing capability:

### 1. New State: HalfOpen

We introduce a `HalfOpen` intermediate state in the workflow registration lifecycle:

```
  ┌──────────┐    fail N times    ┌─────────────┐
  │  Active  │───────────────────>│   Open       │
  │          │                    │ (Quarantined)│
  └──────────┘                    └──────┬───────┘
       ^                                │
       │   unquarantine() or        auto-reset
       │   probe succeeds              timeout
       │                                │
       │                                ▼
       │                        ┌─────────────┐
       │                        │  HalfOpen   │
       │                        │  (Probe)    │
       │                        └──────┬──────┘
       │                               │
       │          ┌────────────────────┼────────────────────┐
       │          │ probe succeeds      │  probe fails       │
       │          ▼                     ▼                    │
       │    ┌──────────┐        ┌─────────────┐             │
       │    │  Active  │        │    Open     │─────────────┘
       │    └──────────┘        │(Quarantined)│  (reset timer)
       │                         └─────────────┘
       │
       │   operator
       └────────────────────────────────────────
```

### 2. Auto-Reset Timeout

When a workflow transitions to `Open` (Quarantined), a **configurable auto-reset timer** starts:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `auto_reset_timeout` | 5 minutes | Time in Open before attempting HalfOpen transition |

After the timeout expires:
1. The breaker automatically transitions `Open → HalfOpen`
2. A **probe request** is initiated to test if the workflow can accept registrations
3. If the probe succeeds, transition `HalfOpen → Closed (Active)`
4. If the probe fails, transition `HalfOpen → Open` and restart the auto-reset timer

### 3. Hysteresis Thresholds

To prevent rapid oscillation between `HalfOpen` and `Open`, we introduce **hysteresis** via separate success/failure thresholds in `HalfOpen`:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `half_open_success_threshold` | 1 | Successful probe requests needed to close |
| `half_open_failure_threshold` | 2 | Failed probe requests before reopening |

This means:
- **1 successful probe** → `HalfOpen → Active` (circuit closes)
- **2 failed probes** → `HalfOpen → Open` (circuit reopens, timer restarts)

### 4. Probe Mechanism

The **probe** is a lightweight health check that tests whether the workflow can accept new registrations. The probe:

1. Attempts a **dry-run registration** (validates hash, checks rate limit but does NOT persist)
2. Returns `Ok` if the workflow appears healthy
3. Returns `Err` if the workflow is still failing

```rust
pub enum ProbeResult {
    /// Workflow appears healthy, can accept registrations
    Healthy,
    /// Workflow is still failing
    Unhealthy,
}
```

### 5. State Exposure via Metrics

The circuit breaker exposes the following metrics for observability:

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `circuit_breaker_state` | Gauge | `workflow_name` | Current state (0=Active, 1=HalfOpen, 2=Open) |
| `circuit_breaker_transitions_total` | Counter | `workflow_name`, `from_state`, `to_state` | State transitions |
| `circuit_breaker_auto_resets_total` | Counter | `workflow_name` | Automatic resets from Open→HalfOpen |
| `circuit_breaker_probe_results_total` | Counter | `workflow_name`, `result` | Probe results (healthy/unhealthy) |
| `circuit_breaker_open_duration_seconds` | Histogram | `workflow_name` | Time spent in Open state |

### 6. Configuration

The `CircuitBreakerConfig` is extended with:

```rust
pub struct CircuitBreakerConfig {
    // ... existing fields ...

    /// Timeout in Open before attempting HalfOpen transition.
    /// Default: 5 minutes.
    pub auto_reset_timeout: Duration,

    /// Number of successful probes needed to close from HalfOpen.
    /// Default: 1.
    pub half_open_success_threshold: u8,

    /// Number of failed probes before reopening from HalfOpen.
    /// Default: 2.
    pub half_open_failure_threshold: u8,
}
```

### 7. Invariants

| ID | Description |
|----|-------------|
| INV-011 | A workflow NEVER transitions directly from Open to Active without passing through HalfOpen |
| INV-012 | The auto-reset timer is restartable — each trip to Open starts a fresh timer |
| INV-013 | Probe results do NOT count toward the original failure threshold |
| INV-014 | Manual unquarantine bypasses HalfOpen and goes directly to Active |
| INV-015 | Hysteresis prevents single-probe-fail from immediately reopening |

## Consequences

- **Positive:** Workflows can self-heal after transient failures, reducing operational burden
- **Positive:** Hysteresis prevents flapping between HalfOpen and Open
- **Positive:** Metrics provide observability into circuit breaker behavior
- **Negative:** Auto-reset may mask underlying issues that need human attention
- **Negative:** Additional complexity in state machine implementation

## Implementation Notes

1. The `RegistrationStatus` enum in `vo-types` will be extended with a `HalfOpen` variant
2. The `CircuitBreakerState` struct will track `half_open_timestamps` per workflow for timer management
3. A background task (or tokio timer) will handle the auto-reset transitions
4. Metrics will be exported via the existing `vo-metrics` infrastructure

## References

- [ADR-026](ADR-026-v2-ai-loop-poisoning-circuit-breakers.md) — Original circuit breaker design
- [ADR-039](ADR-039-v2-hierarchical-lifecycle-state-machine.md) — State machine patterns