# wtf-engine Architecture

**Version:** 3.0
**Model:** Deterministic Event-Sourced Replay
**Language:** Rust (end-to-end)

---

## 1. What This System Is

wtf-engine is a durable execution runtime. It runs long-lived workflows — payment flows, data pipelines, approval chains, ETL jobs — with a guarantee that **no transition is ever lost**, even if the process crashes mid-execution.

It is not a database. It is not a message broker. It is an execution engine that uses a message broker (NATS JetStream) as its source of truth.

The central invariant:

> Every state transition is appended to the NATS JetStream event log before any side effect executes. If the process crashes, the new process replays the log and arrives at exactly the correct state. There is no other recovery mechanism.

---

## 2. Three Layers

```
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 1: Control Plane & Compiler (Dioxus)                     │
│  • Design Mode: visual graph editor → generates Rust source     │
│  • Simulate Mode: local execution preview, no engine needed     │
│  • Monitor Mode: live event log timeline, time-travel scrubber  │
└───────────────────────────┬─────────────────────────────────────┘
                            │ generated Rust code + HTTP/WebSocket
┌───────────────────────────▼─────────────────────────────────────┐
│  LAYER 2: Execution Engine (Ractor + axum)                      │
│  • MasterOrchestrator: root supervisor, capacity enforcement    │
│  • WorkflowInstance actors: per-instance, stateless (replay)   │
│  • Ingress Router: axum HTTP, appends events, returns 200 OK   │
│  • Scheduler: timer polling loop (NATS KV wtf-timers)          │
└───────────────────────────┬─────────────────────────────────────┘
                            │ async-nats
┌───────────────────────────▼─────────────────────────────────────┐
│  LAYER 3: Data Plane (NATS)                                     │
│  • JetStream: immutable event log (source of truth)            │
│  • KV: materialized view (instances, timers, definitions)      │
│  • Object Store: large payload storage (>512 KiB)             │
└─────────────────────────────────────────────────────────────────┘

                     + sled (embedded, local)
                     • Snapshot store (derived state at known seq)
                     • Bounded replay optimization — not source of truth
```

---

## 3. The Event Log

Every operation is an append to the NATS JetStream stream `wtf-events`, subject `wtf.log.<namespace>.<instance_id>`.

**Write-ahead guarantee (ADR-015):**
1. Construct `WorkflowEvent`
2. Publish → await `PublishAck` (majority replication confirmed)
3. Execute side effect
4. Update in-memory state
5. Write NATS KV materialized view (best-effort)

Crash between steps 2 and 3: event is in log → side effect re-executed on recovery. Correct.
Crash between steps 3 and 4: event is in log → idempotency check skips re-execution. Correct.
Crash between steps 1 and 2: event never appended → side effect never happened. Correct.

**`WorkflowEvent` is a closed enum.** Nothing else is written to the log. See ADR-013 for the full definition.

---

## 4. Recovery: Replay, Not Reconciliation

There is no reconciler. There is no background polling loop hunting for stalled instances.

When an actor crashes, the heartbeat TTL in `wtf-heartbeats` KV bucket expires (10 seconds). Any engine node watching that bucket spawns a new actor for the instance. The new actor:

1. Finds the most recent `SnapshotTaken` event in the log
2. Loads the sled snapshot for that sequence number
3. Replays JetStream events from `snapshot.seq + 1` to the stream tail
4. Enters Live Phase: dispatches any in-flight activities that need re-dispatch, re-arms timers, processes new events

The replay is deterministic (ADR-016). The same log always produces the same state. No race conditions. No "what was the reconciler going to do?" reasoning.

---

## 5. Actor Hierarchy

```
MasterOrchestrator (Ractor supervisor, root)
│  • Enforces max_concurrent capacity
│  • Spawns WorkflowInstance actors
│  • Handles crash notifications → triggers recovery spawn
│
├── WorkflowInstance:checkout:01ARZ... (FsmActor)
│   • Owns: current FSM state, applied_seq set, in-flight activities
│   • On startup: loads snapshot, replays log, enters Live Phase
│
├── WorkflowInstance:pipeline:01BQA... (DagActor)
│   • Owns: completed node set, in-flight node set
│
└── WorkflowInstance:onboarding:01CRB... (ProceduralActor)
    • Owns: checkpoint map (operation_id → result), op_counter
```

Each `WorkflowInstance` is stateless between crashes: all durable state lives in NATS JetStream. The in-memory state is always a cache of the replayed log.

---

## 6. Three Execution Paradigms

All three share the same event log, the same replay model, and the same durability guarantees. See ADR-017.

| Paradigm | Best for | Event pattern | Replay mechanism |
|----------|----------|--------------|-----------------|
| **FSM** | Payment flows, order state | `TransitionApplied { from, event, to, effects }` | Apply transitions, skip effect re-execution |
| **DAG** | Pipelines, parallel fan-out | `ActivityCompleted { node_id }` | Rebuild completed set, re-check readiness |
| **Procedural** | Conditional logic, human loops | `ActivityCompleted { operation_id }` | Checkpoint map lookup replaces I/O on re-entry |

---

## 7. CQRS Split

The system uses Command Query Responsibility Segregation (CQRS) at the persistence layer.

**Command side (JetStream):** All writes. Handles ingest, durability, replay. Designed for high write throughput and ordered reads.

**Query side (NATS KV):** All reads. Materialized view maintained by engine actors after each transition. O(1) point lookups. Push-on-change via KV watch.

```
Dioxus Dashboard
  ↑ push (KV watch → axum SSE)
NATS KV wtf-instances
  ↑ write (after each JetStream ACK)
Engine Actor
  ↑ reads + appends
NATS JetStream wtf-events
```

The KV view can lag JetStream by at most one event (the one being committed when the engine crashes). It is always reconstructable from JetStream. See ADR-014.

---

## 8. Persistence Summary

| Store | Role | Contents | Source of truth? |
|-------|------|----------|-----------------|
| NATS JetStream | Primary event log | All `WorkflowEvent` records | **Yes** |
| NATS KV | Materialized view | Instance status, timers, definitions, heartbeats | No (derived) |
| sled (embedded) | Local snapshot cache | `SnapshotRecord { seq, state_bytes, checksum }` | No (derived) |

If JetStream and KV disagree, JetStream wins.
If JetStream and sled disagree, JetStream wins.
Neither KV nor sled can ever be ahead of JetStream.

---

## 9. Crate Structure

```
crates/
├── wtf-common/     # Shared types: WorkflowEvent, InstanceId, RetryPolicy
├── wtf-core/       # Actor state types, DAG (petgraph), context API
├── wtf-actor/      # Ractor actors: MasterOrchestrator, WorkflowInstance variants
├── wtf-storage/    # NATS JetStream + KV wrappers, sled snapshot store
├── wtf-worker/     # Activity worker SDK, gRPC server
├── wtf-api/        # axum HTTP server, SSE endpoint, ingestion pipeline
├── wtf-frontend/   # Dioxus application (Design, Simulate, Monitor)
└── wtf-cli/        # `wtf serve`, `wtf lint`, `wtf admin rebuild-views`
```

---

## 10. Decision Log

| ADR | Decision | Status |
|-----|----------|--------|
| [ADR-001](adr/ADR-001-embedded-database-sled.md) | sled as embedded DB | Amended — sled is snapshot store only |
| [ADR-002](adr/ADR-002-dag-petgraph.md) | petgraph for DAG | Accepted |
| [ADR-003](adr/ADR-003-actor-model-ractor.md) | Ractor for actors | Accepted |
| [ADR-004](adr/ADR-004-step-functions-parity.md) | Step Functions parity | Accepted |
| [ADR-005](adr/ADR-005-journal-replay.md) | Journal-based replay | Superseded by ADR-013, ADR-016 |
| [ADR-006](adr/ADR-006-master-orchestrator-hierarchy.md) | Orchestrator hierarchy | Accepted |
| [ADR-007](adr/ADR-007-sled-schema.md) | sled schema | Amended — single snapshots tree |
| [ADR-008](adr/ADR-008-single-binary-deployment.md) | Single binary | Amended — NATS is external dependency |
| [ADR-009](adr/ADR-009-3x-parallelism.md) | 3x parallelism | Accepted |
| [ADR-010](adr/ADR-010-error-handling-taxonomy.md) | Error taxonomy | Accepted |
| [ADR-011](adr/ADR-011-frontend-architecture.md) | Oya fork frontend | Superseded by ADR-018 |
| [ADR-012](adr/ADR-012-api-design.md) | HTTP REST API | Accepted (extended with SSE, time-travel) |
| [ADR-013](adr/ADR-013-nats-jetstream-event-log.md) | NATS JetStream event log | **Accepted** |
| [ADR-014](adr/ADR-014-nats-kv-materialized-view.md) | NATS KV materialized view | **Accepted** |
| [ADR-015](adr/ADR-015-write-ahead-guarantee.md) | Write-ahead guarantee | **Accepted** |
| [ADR-016](adr/ADR-016-deterministic-replay-model.md) | Deterministic replay model | **Accepted** |
| [ADR-017](adr/ADR-017-three-execution-paradigms.md) | FSM + DAG + Procedural | **Accepted** |
| [ADR-018](adr/ADR-018-dioxus-compiler-control-plane.md) | Dioxus as compiler | **Accepted** |
| [ADR-019](adr/ADR-019-snapshot-recovery-optimization.md) | Snapshot recovery via sled | **Accepted** |
| [ADR-020](adr/ADR-020-procedural-workflow-linter.md) | Procedural workflow linter | **Accepted** |
