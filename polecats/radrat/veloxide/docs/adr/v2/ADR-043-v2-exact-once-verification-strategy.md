# ADR 043 (v2): Exact-Once Verification Strategy

## Status
Accepted

## Context
Veloxide now claims:
1. exactly-once admission,
2. exactly-once control-plane transitions,
3. exactly-once managed effects,
4. deterministic replay,
5. lineage rollover,
6. compensation.

These are too important to leave to unit tests and hope. The architecture needs a formal verification and fault-injection doctrine.

## Decision
We adopt a crash-oriented exact-once verification strategy.

### 1. Crash-Point Matrix
Every critical transition must define injectable crash points before and after:
1. dedupe write,
2. `StepScheduled`,
3. fence acquisition,
4. child start,
5. `EffectPrepared`,
6. connector commit,
7. `EffectCommitted`,
8. `StepCompleted`,
9. timer persistence,
10. signal acceptance,
11. lineage rollover,
12. compensation prepare/commit.

### 2. Required Properties
The test harness must prove at least:
1. duplicate ingress does not create duplicate logical work inside the retention window,
2. stale fence completions cannot win,
3. replay after any injected crash reaches the same legal state,
4. connector ambiguity always routes through reconciliation before retry,
5. projection rebuild reproduces the same operator state,
6. lineage rollover preserves correct signal routing,
7. compensation never runs for an effect that was never durably committed.

### 3. Test Layers
1. pure state-machine tests,
2. property tests for replay invariants,
3. connector contract tests,
4. storage crash-injection integration tests,
5. black-box product-owner scenarios over the real Engine.

### 4. Release Gate
Any change touching replay, dedupe, connectors, timers, signals, lineage, or compensation must run the exact-once verification suite before release.

## Consequences
- **Positive:** Exact-once becomes a tested property, not a marketing phrase.
- **Positive:** Architectural confidence stays high as the runtime grows more complex.
- **Negative:** CI and fault-injection infrastructure become more expensive and slower.
