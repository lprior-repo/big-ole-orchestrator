# Implementation Build Order

This document proposes the implementation order for the final ADR corpus.

The principle is simple:
- build the exact-once core before the nice UX,
- build the canonical workflow model before the drag-and-drop UI,
- build replay and verification before broad connector coverage.

## Phase 0: Type and State Foundations

Goal: make illegal states hard to represent.

Implement:
1. Hierarchical lifecycle model (`ADR-039`)
2. Command envelope metadata (`ADR-036`)
3. Core event schemas with versioning hooks (`ADR-035`)
4. Canonical key encoding helpers (`ADR-020`)

Exit criteria:
1. Engine core compiles around one lifecycle model.
2. Command metadata is present on all mutating surfaces.
3. Durable record types carry schema versions.

## Phase 1: Canonical Workflow Definition

Goal: one workflow language for SDK, Engine, and future UI.

Implement:
1. `WorkflowSpec` model (`ADR-031`)
2. SDK builder -> `WorkflowSpec` emission (`ADR-004`, `ADR-009`)
3. Discovery path validation and version pinning (`ADR-017`, `ADR-022`)
4. Node kind support: `Pure`, `ManagedEffect`, `Wait`, `Signal`, `Unsafe` (`ADR-003`)

Exit criteria:
1. `--graph` emits canonical `WorkflowSpec`.
2. Engine stores validated workflow versions.
3. Exact workflows reject unsupported node mixes.

## Phase 2: Storage and Atomic Control Plane

Goal: make the control plane durable before running real work.

Implement:
1. Fjall partition layout (`ADR-002`)
2. Atomic batch writer (`ADR-016`)
3. `events`, `instances`, `timers`, `leases`, `dedupe`, `effects`, `snapshots`, `workflow_versions`
4. Write-path QoS scaffolding (`ADR-032`)

Exit criteria:
1. Every control-plane transition uses one atomic path.
2. Writer metrics exist for queue depth and commit latency.

## Phase 3: Execution Boundary and Pure-Step Runtime

Goal: run pure work safely through the hardened subprocess boundary.

Implement:
1. FD3/FD4 contract (`ADR-012`, `ADR-014`, `ADR-018`)
2. Current-thread SDK runtime (`ADR-011`)
3. Signal handling / termination (`ADR-019`)
4. Stderr bounds (`ADR-023`)
5. Execution semaphores and actor invariants (`ADR-006`, `ADR-015`)

Exit criteria:
1. Pure steps can execute, fail, timeout, and replay safely.
2. Stale completions are rejected once fencing arrives.

## Phase 4: Exactly-Once Core

Goal: make exact-once true inside the Engine.

Implement:
1. Replay engine (`ADR-027`)
2. Ingress dedupe (`ADR-028`)
3. Execution leases and fencing (`ADR-029`)
4. Snapshotting and recovery throttle (`ADR-013`, `ADR-016`)
5. Verification harness skeleton (`ADR-043`)

Exit criteria:
1. Duplicate ingress does not create duplicate logical work inside retention.
2. Stale fence completions cannot win.
3. Recovery reaches legal deterministic state after injected crashes.

## Phase 5: Waiting, Timers, and Signals

Goal: support durable long-lived workflows.

Implement:
1. Hibernation and timer lifecycle (`ADR-005`)
2. Signal matching semantics (`ADR-042`)
3. Resume fairness integration (`ADR-033`)
4. Command identity integration for operator/API signals (`ADR-036`)

Exit criteria:
1. Waiting workflows hibernate cleanly.
2. Signals resume only the correct lineage/epoch/wait state.
3. Crash recovery preserves timer and signal correctness.

## Phase 6: Managed Effects

Goal: move exact-safe external effects behind the Engine.

Implement:
1. Managed effect journal (`ADR-030`)
2. Connector runtime contract (`ADR-041`)
3. One or two strong connectors first, not many
4. Compensation model (`ADR-034`)

Recommended first connector classes:
1. idempotency-key HTTP connector
2. SQL connector with unique constraint / compare-and-set semantics

Exit criteria:
1. Connector ambiguity routes through reconciliation, not blind retry.
2. Managed effects can commit exactly once under crash injection.

## Phase 7: Privacy and Blob Publication

Goal: preserve replay truth without turning storage into a liability.

Implement:
1. Canonical blob publication protocol (`ADR-040`)
2. Dual representation for canonical vs operator projection (`ADR-025`)
3. Encryption and DEK/KEK lifecycle
4. Blob garbage collection and retention policy

Exit criteria:
1. `output_ref` is never durable before blob durability.
2. Operator history remains useful while canonical state stays encrypted.

## Phase 8: Long-Lived Workflow Maturity

Goal: make the system stay correct over months, not minutes.

Implement:
1. Upcasters (`ADR-035`)
2. Rebuildable projections (`ADR-037`)
3. Continue-as-new / lineage (`ADR-038`)
4. Projection rebuild tooling

Exit criteria:
1. Old events still replay after schema change.
2. Lineage rollover preserves routing and signal behavior.

## Phase 9: UI, AI, and Operator Surfaces

Goal: expose the architecture cleanly without distorting it.

Implement:
1. Query APIs and best-effort SSE (`ADR-007`, `ADR-024`)
2. Guarantee-aware UI badges and views (`ADR-007`, `ADR-031`)
3. AI-facing redacted history and canonical privileged history (`ADR-008`, `ADR-025`)
4. Quarantine / circuit-breaker workflow controls (`ADR-026`)

Exit criteria:
1. UI reflects exact vs unsafe semantics truthfully.
2. Operator workflows do not bypass command identity / dedupe.

## Phase 10: Freeze Gate

Goal: stop architecture drift.

Implement:
1. Exact-once verification suite as release gate (`ADR-043`)
2. ADR freeze review checklist
3. Contradiction scan on each semantic change

## Recommended First Vertical Slice

Build this first:
1. One exact workflow
2. Pure node -> Wait node -> Signal node -> Managed effect node
3. One strong connector
4. Crash injection across every transition

If that vertical slice survives, the architecture is real.
