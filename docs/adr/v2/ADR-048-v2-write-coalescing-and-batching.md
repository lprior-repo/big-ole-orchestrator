# ADR 048 (v2): Write Coalescing and Batching Strategies

## Status
Accepted

## Context

Veloxide's write path serves three distinct QoS classes (ADR-032): critical control-plane events, operator projections, and bulk blobs. Each write currently flows through a single `fjall::OwnedWriteBatch` that is committed immediately. This one-write-one-commit pattern produces correct results but leaves throughput on the table when the engine is under moderate-to-high write pressure.

The existing infrastructure already provides the building blocks for coalescing:

1. **`BudgetQueues<T>`** (`append/queue.rs`) — three per-class bounded `VecDeque` queues that hold pending writes before they are drained.
2. **`Appender`** (`append/appender.rs`) — the enqueue facade that classifies and routes writes into the budget queues.
3. **`WriteBudget`** (`append/write_class.rs`) — byte-level budget tracking per class that gates enqueue capacity.
4. **`CommitLatencyTracker`** (`append/latency.rs`) — records commit latency and time-since-last-commit, enabling drain-on-latency decisions.
5. **`BackpressureSignal`** (`append/backpressure.rs`) — atomic booleans per class indicating queue fullness.
6. **`DurableBudgetSaga`** (`budget_saga/`) — two-phase stage/commit protocol with crash recovery for budget reservations.

The `BudgetQueues` already accumulate writes into class-separated queues. The missing piece is a **drain strategy** — a decision procedure that determines *when* to pop items from the queues and flush them as a single `fjall::OwnedWriteBatch`.

Without an explicit coalescing strategy, each enqueue-drain cycle produces at most one batch item, and the engine issues a commit per write. Under bursty workloads (e.g., fan-out DAG nodes completing simultaneously), this generates excessive I/O ops and SSD write amplification.

## Decision

We adopt a **time-windowed, count-gated drain strategy** called **Adaptive Coalescing** that sits between the `BudgetQueues` and the `fjall::OwnedWriteBatch` commit path.

### 1. Coalescing Policy

Writes are not flushed individually. Instead, the drain loop accumulates items from `BudgetQueues` and commits when *any* of the following conditions is met:

| Trigger | Threshold | Rationale |
|---------|-----------|-----------|
| **Max batch size** | 64 items per class | Caps per-class batch to prevent unbounded memory growth |
| **Max batch bytes** | 256 KiB per batch | Caps total batch byte size to keep commit latency bounded |
| **Max wait time** | 5 ms since first item enqueued | Ensures tail latency never exceeds 5 ms for any write class |
| **Critical priority drain** | Any critical item enqueued while non-critical batch is open | Critical control-plane writes bypass the timer and force an immediate drain of the open batch |
| **Idle drain** | Queue depth > 0 and no new items for 1 ms | Flushes residual items when the burst ends |

### 2. Batch Construction

When the drain fires, the coalescer:

1. Drains all available items from `BudgetQueues.dequeue_prioritized()` until the batch limits are hit. Prioritization order: `CriticalControlPlane > OperatorProjection > BulkBlob` (already implemented by `dequeue_prioritized`).
2. Groups drained items by target fjall keyspace partition (events, instances, dedupe, snapshots, timers, blobs).
3. Builds a single `fjall::OwnedWriteBatch` with all items across partitions.
4. Commits the batch atomically.
5. Releases budget for all committed items via `WriteBudget.release()`.

### 3. Write Ordering Invariant

Coalescing MUST preserve write ordering within a single instance ID. The constraint:

- Items for the same `instance_id` within the same batch are committed in their enqueue order (FIFO).
- Items for different `instance_id`s may be interleaved within the batch — fjall's sorted key space handles this.
- The `BudgetQueues` are already FIFO per class, so no additional ordering mechanism is needed.

### 4. Saga Integration

The `DurableBudgetSaga` stage/commit protocol remains unchanged. The coalescer operates *after* staging — items in the `BudgetQueues` are already staged. The coalescer drains and commits the batch; if the batch commit fails, the saga rollback path is invoked for all items in the failed batch.

```
enqueue → stage_write → BudgetQueues → [COALESCER] → fjall::OwnedWriteBatch → commit
                                         ↑
                                    drain policy
```

### 5. Admission Coupling (ADR-032 Integration)

The coalescer exposes metrics that feed the admission controller:

- `vo_storage.coalescer_batch_size` — histogram of items per committed batch
- `vo_storage.coalescer_batch_bytes` — histogram of bytes per committed batch
- `vo_storage.coalescer_wait_ms` — histogram of time items spent in the coalescing window
- `vo_storage.coalescer_forced_drain_total` — counter of critical-priority forced drains

These metrics allow the admission controller (ADR-032 Section 3) to make ingress-shedding decisions based on coalescer pressure, not just raw queue depth.

### 6. What Coalescing Does NOT Change

- **Atomic admission commit** (`admission_commit.rs`) — remains a single synchronous `fjall::OwnedWriteBatch`. Admission is a latency-critical path and does not go through the coalescer.
- **Atomic suspend commit** (`atomic_wait_commit.rs`) — remains a single synchronous batch. Timer registration is latency-critical.
- **Event + summary commit** (`event_summary_commit.rs`) — remains a single synchronous batch. This is the inner loop of the actor state machine.

The coalescer applies to the **bulk append path** — high-volume event writes, projection updates, and blob writes that flow through the `Appender` / `BudgetQueues`.

## Consequences

### Positive
- **Reduced I/O ops**: 64 writes that previously required 64 `fjall::OwnedWriteBatch` commits now require 1.
- **Lower SSD write amplification**: Larger batches align better with SSD page sizes (typically 4 KiB–16 KiB).
- **Bounded tail latency**: The 5 ms max wait ensures no write is held indefinitely.
- **Critical writes stay fast**: Forced drain on critical enqueue prevents control-plane latency degradation.

### Negative
- **Added complexity**: The coalescer introduces a timer-based drain loop that must be correctly integrated with the existing budget queue and saga infrastructure.
- **Memory pressure**: The coalescing window holds items in memory until the batch commits. The 256 KiB byte cap and 64-item count cap bound this.
- **Partial batch failure**: If a batch commit fails, all items in the batch must be rolled back via the saga. This is already handled by the existing atomic batch semantics of `fjall::OwnedWriteBatch`.

### Migration
- The coalescer is additive — existing commit paths remain unchanged.
- The `Appender` API gains a `drain_coalesced()` method alongside the existing per-class `dequeue_*` methods.
- No changes to `BudgetQueues`, `WriteBudget`, or `DurableBudgetSaga` are required.
