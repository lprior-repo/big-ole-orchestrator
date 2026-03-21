# ADR-014: NATS KV as Materialized View (CQRS Query Side)

## Status

Accepted

## Context

The NATS JetStream event log (ADR-013) is the source of truth but is not designed for O(1) point queries. To answer "what is the current state of instance X?" the engine would need to replay the full log — expensive for the UI polling constantly.

We need a fast, queryable representation of current workflow state, timer definitions, and workflow definitions. This is a classic CQRS problem: the command side (JetStream) handles durability; the query side needs a fast read path.

### Requirements

1. **O(1) instance state lookup** — for Dioxus dashboard rendering
2. **Push-on-change** — Dioxus subscribes to changes rather than polling
3. **Reconstructable** — if corrupted, must be rebuildable from JetStream
4. **Separate from source of truth** — losing KV data is survivable
5. **Same operational surface as JetStream** — no additional infrastructure

NATS KV is built on top of JetStream. It is a first-class primitive in the NATS ecosystem, available in the same `async-nats` client, on the same cluster. It satisfies all five requirements.

## Decision

NATS KV is the **materialized view** (query side) of workflow state. It is written by the engine *strictly after* the corresponding JetStream event is acknowledged. It is never the ground truth. It is always reconstructable.

### KV Buckets

```
Bucket: wtf-instances
  Key:   <namespace>/<instance_id>
  Value: InstanceView { status, current_state, last_event_seq, updated_at }
  TTL:   none (engine removes on terminal state after archive)

Bucket: wtf-snapshots
  Key:   <namespace>/<instance_id>
  Value: SnapshotRecord { seq, state_bytes, checksum }
  TTL:   none
  Note:  Written by engine when snapshotting (ADR-019). Also readable by sled fallback.

Bucket: wtf-timers
  Key:   <timer_id>
  Value: TimerRecord { instance_id, fire_at, event_to_inject }
  TTL:   auto-expires at fire_at + 5min buffer

Bucket: wtf-definitions
  Key:   <namespace>/<workflow_type>
  Value: WorkflowDefinition (DAG or FSM or Procedural spec)
  TTL:   none

Bucket: wtf-heartbeats
  Key:   hb/<instance_id>
  Value: engine node ID
  TTL:   10 seconds (refreshed by actor's heartbeat task)
  Note:  Expiry triggers recovery on any watching engine node
```

### Write Ordering (Critical)

The engine always writes KV **after** the JetStream `PublishAck`:

```
1. Construct WorkflowEvent
2. Publish to JetStream → await PublishAck        ← durable
3. Execute side effect
4. Update in-memory state
5. Write to NATS KV                               ← derived view (best-effort)
```

Step 5 failing does not compromise durability. The KV entry can be reconstructed by replaying the log.

### Dioxus Real-Time Updates

The Dioxus dashboard subscribes to the `wtf-instances` bucket via a watch:

```
Engine → JetStream (authoritative)
       → NATS KV wtf-instances (write on transition)
         ↑
         NATS KV watch (push on change)
         ↓
       Axum SSE endpoint (/api/v1/watch/<namespace>)
         ↓
       Dioxus frontend (live state overlays on graph)
```

Zero polling. The instant an actor advances, the KV update propagates to the frontend.

### Reconstruction from JetStream

If the KV buckets are wiped:

```bash
wtf admin rebuild-views --namespace <ns>
```

The admin tool streams all JetStream events for all instances, replays them, and rewrites the KV buckets. This is an offline operation and takes seconds for typical deployments.

## Consequences

### Positive

- O(1) current state lookup for all dashboard queries
- Push-based updates to Dioxus via SSE — no polling overhead
- Heartbeat TTL-based recovery without a reconciler (the expiry is the trigger)
- Single operational surface: same NATS cluster for both JetStream and KV

### Negative

- KV may lag JetStream by one event if the engine crashes between steps 4 and 5
- Reconstruction required if KV is corrupted (offline operation)
- Additional write per transition (negligible overhead)

### Invariant

The engine guarantees: **KV state ≤ JetStream state** (never ahead). KV can be behind; it can never claim a transition occurred that is not in the log.
