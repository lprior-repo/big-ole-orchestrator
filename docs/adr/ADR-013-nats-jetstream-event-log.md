# ADR-013: NATS JetStream as Primary Event Log

## Status

Accepted

## Context

wtf-engine v3 adopts event sourcing as its core durability model. Every state transition, activity dispatch, timer registration, signal receipt, and side-effect result must be recorded in an ordered, durable, replicated log *before* any side effect executes. The log is the source of truth. All in-memory actor state is derived by replaying it.

### Requirements for the Event Log

1. **Ordered** — Events within an instance must have a total, gap-free ordering
2. **Durable** — A committed event must survive a majority-node cluster failure
3. **Replicated** — No single point of failure
4. **Acknowledged** — Publisher must receive confirmation of replication before proceeding
5. **Subscribable** — Actors must be able to stream events for a specific instance (replay)
6. **Subject-scoped** — Per-instance isolation without a full table scan
7. **Rust-native client** — `async-nats` is a first-class async client

### Candidates Evaluated

| System | Ordered | Replicated | Rust Client | Embedded Option | Notes |
|--------|---------|------------|-------------|-----------------|-------|
| **NATS JetStream** | Yes (per subject) | Yes (Raft) | `async-nats` | Yes (single binary) | Chosen |
| Kafka | Yes (per partition) | Yes | `rdkafka` (C FFI) | No | Heavyweight, FFI |
| Redis Streams | Yes (per stream) | Yes (Sentinel) | `redis-rs` | No | Weaker durability guarantees |
| sled (prior) | Yes (per tree) | No | Native | Yes | Not replicated — single node only |

### Why NATS JetStream Wins

- **`async-nats`** is pure Rust, async-native, tokio-compatible — no FFI
- **Per-subject streams** map directly to per-instance event logs (`wtf.log.<ns>.<id>`)
- **Explicit publish acknowledgment** (`PublishAck`) confirms Raft replication to majority before the engine proceeds
- **Consumer API** allows replay from any sequence number — essential for crash recovery
- **Embedded mode** (`--nats-embedded`) allows single-binary dev experience (ADR-008)
- **JetStream KV** (ADR-014) is built on JetStream — same cluster, one operational surface

## Decision

NATS JetStream is the **immutable event log** and **primary source of truth** for all workflow instance state in wtf-engine.

### Stream Configuration

```
Stream name:  wtf-events
Subject:      wtf.log.<namespace>.<instance_id>
Storage:      File (replicated)
Replicas:     3 (production), 1 (dev embedded)
Retention:    Limits (max age: 90 days, max per subject: unlimited)
Max msg size: 1 MiB (large payloads reference NATS Object Store)
Ack policy:   Explicit
```

### Subject Convention

```
wtf.log.<namespace>.<instance_id>   # per-instance event log
wtf.work.<namespace>.<activity_type> # activity work queue
wtf.signals.<namespace>.<instance_id>.<signal_name> # signals
wtf.archive.<namespace>             # terminal instance log archive
```

### WorkflowEvent Enum (Closed, Typed)

Every append to the event log is a member of this enum. No other types are written.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEvent {
    // Lifecycle
    InstanceStarted { instance_id: String, workflow_type: String, input: Bytes },
    InstanceCompleted { output: Bytes },
    InstanceFailed { error: String },
    InstanceCancelled { reason: String },

    // FSM transitions
    TransitionApplied {
        from_state: String,
        event_name: String,
        to_state: String,
        effects: Vec<EffectDeclaration>,
    },

    // Activities
    ActivityDispatched { activity_id: String, activity_type: String, payload: Bytes, retry_policy: RetryPolicy, attempt: u32 },
    ActivityCompleted { activity_id: String, result: Bytes, duration_ms: u64 },
    ActivityFailed    { activity_id: String, error: String, retries_exhausted: bool },

    // Timers
    TimerScheduled { timer_id: String, fire_at: DateTime<Utc> },
    TimerFired     { timer_id: String },
    TimerCancelled { timer_id: String },

    // Signals
    SignalReceived { signal_name: String, payload: Bytes },

    // Child workflows
    ChildStarted    { child_id: String, workflow_type: String },
    ChildCompleted  { child_id: String, result: Bytes },
    ChildFailed     { child_id: String, error: String },

    // Snapshots
    SnapshotTaken { seq: u64, checksum: u32 },
}
```

### Write-Ahead Guarantee

The engine enforces this ordering for every operation:

1. Construct `WorkflowEvent`
2. Publish to `wtf.log.<ns>.<id>` and **await `PublishAck`**
3. Execute side effect (dispatch activity, fire timer, etc.)
4. Update in-memory actor state

If the process crashes between steps 2 and 3, the event is in the durable log. Recovery replays it and re-executes the side effect. This is correct. There is no other recovery mechanism.

### Replay Procedure

On actor startup (crash recovery or new node):

1. Create an ordered consumer from seq=1 (or from snapshot seq, see ADR-019)
2. Pull events in sequence order
3. Apply each event to in-memory state (skip side-effect execution)
4. When consumer reaches the stream tail, switch to live processing

## Consequences

### Positive

- Zero lost transitions: committed events survive majority-node failure
- Crash recovery is deterministic: replay always produces the same state
- Full audit trail is a free byproduct of normal operation
- Time-travel debugging: replay to any sequence number
- Per-subject ordering gives per-instance total order with no cross-instance lock contention

### Negative

- NATS is a new operational dependency (mitigated by embedded dev mode)
- Event log grows indefinitely (mitigated by snapshots, ADR-019, and archiving)
- Large payloads require Object Store indirection (1 MiB limit per message)

### Mitigations

- Embedded NATS for zero-config local development
- Snapshot mechanism (ADR-019) keeps replay fast for long-lived workflows
- Object Store reference pattern for payloads > 512 KiB
