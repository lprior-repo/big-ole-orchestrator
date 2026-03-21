# ADR-015: Write-Ahead Guarantee (Event Before Side Effect)

## Status

Accepted

## Context

The central durability property of wtf-engine is: **no committed transition is ever lost**. A transition is "committed" when its event is durably appended to JetStream with a majority `PublishAck`. Once committed, the transition must eventually produce its side effects, even if the engine crashes immediately after the append.

This requires a strict ordering invariant that every code path in the engine must obey. It cannot be enforced only at the architectural level — it must be enforced at the code level, in every actor message handler.

The prior architecture (ADR-005) wrote to sled *after* the side effect, which created a crash window where the effect executed but was never recorded, and on recovery it would be re-executed (if the engine detected the crash at all via reconciliation). This is the opposite of what we want.

## Decision

Every actor message handler that executes a side effect **must** follow the write-ahead sequence without exception:

```
1. Construct the WorkflowEvent that describes the intended operation
2. Publish event to JetStream subject wtf.log.<ns>.<id>
3. Await PublishAck (confirms majority replication)
4. Execute the side effect
5. Update in-memory actor state
6. Write derived state to NATS KV (best-effort)
```

Steps 1–3 are atomic from the engine's perspective. The engine will not proceed to step 4 until step 3 succeeds.

### Crash Scenarios and Their Outcomes

| Crash point | Event in log? | Side effect executed? | Recovery action |
|-------------|--------------|----------------------|-----------------|
| Before step 2 | No | No | Caller retries; no event in log; correct |
| Between 2 and 3 (in flight) | Maybe | No | NATS retransmit; PublishAck eventually received or timeout; retry |
| Between 3 and 4 | Yes | No | Replay sees event; re-executes side effect |
| Between 4 and 5 | Yes | Yes | Replay sees event; skips side effect (idempotency, ADR-016) |
| After 5 | Yes | Yes | No recovery needed |

In every case, the outcome is correct. The only recovery primitive needed is log replay.

### Idempotency Requirement

Because the engine may replay an event whose side effect was already partially or fully executed before a crash, all side effects must be idempotent with respect to their activity ID or timer ID. The engine tracks a set of `applied_seq` numbers in the actor's in-memory state. During replay, any event whose JetStream sequence number is already in `applied_seq` is skipped.

For activity dispatch specifically: before dispatching to the work queue, the engine checks whether the activity ID is already registered as in-flight. If yes, the dispatch is skipped. The worker's response will still arrive and complete the activity normally.

### SDK Enforcement

The write-ahead sequence is encapsulated in a single function in the engine SDK:

```rust
/// Append an event to the instance log and await durability confirmation.
/// This is the ONLY way to record a transition. Direct JetStream publishes
/// outside this function are forbidden in engine code.
async fn append_event(
    &self,
    js: &JetStreamContext,
    event: WorkflowEvent,
) -> Result<u64, AppendError> {
    let payload = rmp_serde::to_vec(&event)?;
    let ack = js
        .publish(self.subject.clone(), payload.into())
        .await?
        .await?;  // double-await: first for send, second for Raft ACK
    Ok(ack.sequence)
}
```

All actor handlers call `append_event` before any side effect. A linter rule (via `clippy` custom lint or CI check) verifies no `async-nats` publish calls exist outside `append_event`.

## Consequences

### Positive

- The crash window between "recorded" and "executed" is eliminated in the dangerous direction
- Recovery is a single mechanism: log replay
- No reconciler, no heartbeat polling loop, no manual recovery tooling
- Every possible crash scenario has a deterministic, correct outcome

### Negative

- Every transition now has a NATS round-trip before it can proceed
- Network latency to NATS adds to per-transition latency (~1ms local, ~5ms cross-AZ)
- Developers writing engine internals must understand the sequence; linter enforcement is critical

### Mitigations

- NATS latency is bounded and predictable; for most workflows this is acceptable
- The embedded NATS mode (ADR-008) reduces latency to sub-millisecond for dev
- Linter enforcement makes violations visible at CI time, not in production
